pub mod definitions;
pub mod file_ops;
pub mod shell;
pub mod git;
pub mod web;
pub mod sidecar;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Per-session "what has the model SEEN" of each file — feeds the write
/// gate (never overwrite a partially-seen file) and the one-shot read dedup.
pub type SeenLedger = Arc<std::sync::Mutex<HashMap<String, HashMap<PathBuf, SeenFile>>>>;

/// Per-session cap on stale-anchor snapshot text kept for drift reporting.
const LEDGER_CONTENT_BUDGET: usize = 16 * 1024 * 1024;

/// Monotonic stamp for per-session snapshot eviction (oldest read first).
static _SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone)]
pub struct SeenFile {
    /// mtime + size at serve time (dedup key).
    pub mtime: std::time::SystemTime,
    pub size: u64,
    /// 1-based inclusive window served, when a window was returned.
    pub window: Option<(usize, usize)>,
    /// Total lines at read time (None when byte-capped and only the head fit).
    pub total: Option<usize>,
    /// True when MORE of the file exists beyond what was served.
    pub partial: bool,
    /// One-shot dedup record: exact (mtime, size, window) served last time.
    pub dedup: Option<(u64, Option<(usize, usize)>)>,
    /// Whole-file content tag from that read (stable across windows).
    pub tag: Option<String>,
    /// Canonical text the tag hashes — kept so a stale-edit mismatch can
    /// point at the drifted lines instead of "re-read the whole file".
    pub content: Option<String>,
    /// Insertion order stamp (per-session eviction by age under budget).
    pub seq: u64,
    /// True after the model edited this file — its layout (and the snapshot)
    /// are stale until the next read. Forces a re-read before the next edit.
    pub edited: bool,
}

pub struct ToolExecutor {
    pub working_dir: PathBuf,
    /// Session for ledger bookkeeping (None = tools run without a ledger).
    pub session_id: Option<String>,
    pub ledger: Option<SeenLedger>,
}

impl ToolExecutor {
    pub fn with_ledger(
        working_dir: impl AsRef<Path>,
        session_id: &str,
        ledger: SeenLedger,
    ) -> Self {
        Self {
            working_dir: working_dir.as_ref().to_path_buf(),
            session_id: Some(session_id.to_string()),
            ledger: Some(ledger),
        }
    }

    pub fn execute(&self, name: &str, input: &serde_json::Value) -> Result<String, String> {
        match name {
            // file ops
            "read_file" => self.execute_read(input),
            "write_file" => self.execute_write(input),
            "edit_file" => self.execute_edit(input),
            "delete_file" => file_ops::delete_file(&self.working_dir, input),
            "list_files" => file_ops::list_files(&self.working_dir, input),
            "search_files" => file_ops::search_files(&self.working_dir, input),
            // shell
            "run_command" => shell::run_command(&self.working_dir, input),
            // git
            "git_status" => git::git_status(&self.working_dir, input),
            "git_diff" => git::git_diff(&self.working_dir, input),
            "git_log" => git::git_log(&self.working_dir, input),
            "git_add" => git::git_add(&self.working_dir, input),
            "git_commit" => git::git_commit(&self.working_dir, input),
            // web
            "web_search" => web::web_search(input),
            "fetch_url" => web::fetch_url(input),
            other => Err(format!("unknown tool: {other}")),
        }
    }

    /// read_file with ledger bookkeeping: one-shot dedup for exact-window
    /// re-reads (self-expiring so a stale stub can never loop), then record
    /// what the model actually SAW for the write gate.
    fn execute_read(&self, input: &serde_json::Value) -> Result<String, String> {
        let d = file_ops::read_file_detailed(&self.working_dir, input)?;
        let (Some(session_id), Some(ledger)) = (&self.session_id, &self.ledger) else {
            return Ok(d.text);
        };
        let dedup_off = std::env::var("PRAGMA_NO_DEDUP").is_ok();
        let meta_now = std::fs::metadata(&d.abs_path).ok();
        let mtime = meta_now.as_ref().and_then(|m| m.modified().ok());
        let size = meta_now.map(|m| m.len()).unwrap_or(0);

        {
            let mut map = ledger.lock().unwrap();
            let entry = map.entry(session_id.clone()).or_default();
            let seen = entry.get(&d.abs_path);

            // Self-expiring dedup: exact (mtime, size, window) match → serve a
            // stub AND consume the record. A stale hit is a wasted turn at
            // worst; a miss is cheap. mtime guards the same-size case (file
            // rewritten without changing its length), and the window guard
            // keeps *facts* (not found / binary / empty → window=None) out of
            // dedup — re-reading a missing file must say "not found" again,
            // never "identical content" about nothing.
            if !dedup_off && d.window.is_some() {
                if let Some(s) = seen {
                    if let Some((s_size, s_win)) = s.dedup {
                        if mtime == Some(s.mtime) && s_size == size && s_win == d.window {
                            let name = d.abs_path
                                .strip_prefix(&self.working_dir)
                                .unwrap_or(&d.abs_path)
                                .display()
                                .to_string();
                            let win = match d.window {
                                Some((a, b)) if a != b => format!(" lines {a}-{b}"),
                                Some((a, _)) => format!(" line {a}"),
                                None => String::new(),
                            };
                            entry.get_mut(&d.abs_path).unwrap().dedup = None;
                            return Ok(format!("[{name}{win}]: unchanged since the last read of this window — identical content; see that read above"));
                        }
                    }
                }
            }

            // Record what was served (replaces any previous record). Only a
            // real served window arms dedup — facts re-read fresh every time.
            let next_seq = _SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let (content, tag) = (d.canonical, d.tag);
            entry.insert(
                d.abs_path.clone(),
                SeenFile {
                    mtime: mtime.unwrap_or(std::time::UNIX_EPOCH),
                    size,
                    window: d.window,
                    total: d.total,
                    partial: d.partial,
                    dedup: if d.window.is_some() { Some((size, d.window)) } else { None },
                    tag,
                    content,
                    seq: next_seq,
                    edited: false,
                },
            );
            // Bounded stale-anchor memory: keep at most LEDGER_CONTENT_BUDGET
            // of snapshot text per session; evict oldest by read order first.
            let budget: usize = entry
                .values()
                .map(|f| f.content.as_ref().map(|c| c.len()).unwrap_or(0))
                .sum();
            if budget > LEDGER_CONTENT_BUDGET {
                let mut aged: Vec<(PathBuf, u64)> = entry
                    .iter()
                    .filter(|(_, f)| f.content.is_some())
                    .map(|(p, f)| (p.clone(), f.seq))
                    .collect();
                aged.sort_by_key(|(_, s)| *s);
                let mut freed = 0usize;
                for (path, _) in aged {
                    let f = entry.get_mut(&path).unwrap();
                    if let Some(c) = f.content.take() {
                        freed += c.len();
                    }
                    if budget.saturating_sub(freed) <= LEDGER_CONTENT_BUDGET {
                        break;
                    }
                }
            }
        }
        Ok(d.text)
    }

    /// The partial-view gate shared by every content-rewriting tool
    /// (write_file, edit_file): never touch a file the model has only partly
    /// seen — name the recovery instead. Returns the full path when the gate
    /// passes.
    fn gate_seen_write(&self, path: &str) -> Result<Option<PathBuf>, String> {
        let (Some(session_id), Some(ledger)) = (&self.session_id, &self.ledger) else {
            return Ok(None); // no ledger → no gate
        };
        let full = file_ops::abs(&self.working_dir, path);
        {
            let map = ledger.lock().unwrap();
            if let Some(seen) = map.get(session_id).and_then(|m| m.get(&full)) {
                if seen.partial {
                    let hint = match (seen.window, seen.total) {
                        (Some((_, last)), Some(total)) => {
                            format!("you have only seen lines up to {last} of {total} — re-read the rest first (offset={}) before overwriting", last + 1)
                        }
                        (_, None) => {
                            "the file is larger than 128 KB and you have only seen its head — locate the parts you need with search_files, or read a smaller file, before overwriting".to_string()
                        }
                        _ => "you have only seen part of the file — re-read it fully (offset=1) before overwriting".to_string()
                    };
                    return Err(format!("[write denied]: {path} — {hint}"));
                }
            }
        }
        Ok(Some(full))
    }

    /// Invalidate any seen/dedup record after the file actually changed under
    /// the model's feet.
    fn clear_seen(&self, full: &PathBuf) {
        if let (Some(session_id), Some(ledger)) = (&self.session_id, &self.ledger) {
            ledger.lock().unwrap().get_mut(session_id).map(|m| m.remove(full));
        }
    }

    /// write_file with the partial-view gate: never blindly overwrite a file
    /// the model has only partly seen — name the recovery instead.
    fn execute_write(&self, input: &serde_json::Value) -> Result<String, String> {
        let (Some(_), Some(_)) = (&self.session_id, &self.ledger) else {
            return file_ops::write_file(&self.working_dir, input);
        };
        let path = input["path"].as_str().ok_or("missing path")?;
        let full = match self.gate_seen_write(path) {
            Ok(f) => f,
            Err(denied) => return Ok(denied), // gate denial is a fact, not an error
        };
        let r = file_ops::write_file(&self.working_dir, input);
        // The file changed under the model's feet — stale seen/dedup records
        // are now wrong.
        if let Some(f) = full {
            self.clear_seen(&f);
        }
        r
    }

    /// Edit_file with hash anchoring + the seen-window gate: edits must stay
    /// inside what the model actually read (hashline rule — touch only
    /// displayed lines); a stale anchor or an edit after your own last edit
    /// is denied and names the recovery. The whole-file hash carries the rest
    /// of the staleness check.
    fn execute_edit(&self, input: &serde_json::Value) -> Result<String, String> {
        let (Some(session_id), Some(ledger)) = (&self.session_id, &self.ledger) else {
            return file_ops::edit_file(&self.working_dir, input, None);
        };
        let path = input["path"].as_str().ok_or("missing path")?;
        let start = input["start_line"].as_u64().unwrap_or(0) as usize;
        let end = input["end_line"].as_u64().unwrap_or(start as u64) as usize;
        let full = file_ops::abs(&self.working_dir, path);

        let (denial, stale) = {
            let map = ledger.lock().unwrap();
            let seen = map.get(session_id).and_then(|m| m.get(&full));
            match seen {
                // Your own last edit moved the layout — line numbers are stale.
                Some(f) if f.edited => (
                    Some(format!(
                        "[edit denied]: {path} — the file changed under your last edit; re-read it (offset=1) to get fresh line numbers and the new hash, then retry"
                    )),
                    None,
                ),
                // Edited range must lie within the window the model saw.
                Some(f) if start < 1 || (f.partial && end > f.window.map(|(_, l)| l).unwrap_or(0)) => (
                    Some(
                        f.window
                            .map(|(first, last)| {
                                format!(
                                    "[edit denied]: {path} — lines {start}-{end} are outside the window you have seen ({first}-{last}); re-read the part you want to edit (offset={}) then retry",
                                    last + 1
                                )
                            })
                            .unwrap_or_else(|| format!("[edit denied]: {path} — the file extends beyond what you have seen; re-read the part you want to edit then retry")),
                    ),
                    None,
                ),
                _ => (None, seen.and_then(|f| f.content.clone())),
            }
        };
        if let Some(denied) = denial {
            return Ok(denied);
        }
        let r = file_ops::edit_file(&self.working_dir, input, stale.as_deref());
        if r.is_ok() {
            // The model's window/snapshot is now stale: mark the file edited so
            // the next edit forces a re-read (line numbers moved under it).
            if let Some(seen) = ledger.lock().unwrap().get_mut(session_id).and_then(|m| m.get_mut(&full)) {
                seen.edited = true;
                seen.content = None; // pre-edit snapshot no longer matches
            }
        }
        r
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    fn ledger_executor(wd: &std::path::Path, session: &str) -> (ToolExecutor, SeenLedger) {
        let ledger: SeenLedger = Arc::new(std::sync::Mutex::new(HashMap::new()));
        (ToolExecutor::with_ledger(wd, session, ledger.clone()), ledger)
    }

    #[test]
    fn test_write_gate_refuses_partial_overwrite_with_recovery() {
        let dir = tempdir().unwrap();
        let content = (1..=4000).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        fs::write(dir.path().join("big.txt"), &content).unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s1");

        // Read only the head (line window caps at 2000).
        let r1 = ex.execute("read_file", &serde_json::json!({"path": "big.txt"})).unwrap();
        assert!(r1.contains("offset=2001"));

        // Overwriting now must be denied with a named recovery.
        let deny = ex.execute("write_file", &serde_json::json!({"path":"big.txt","content":"clobbered"})).unwrap();
        assert!(deny.contains("write denied"), "gate fired: {deny}");
        assert!(deny.contains("offset=2001"), "recovery hint: {deny}");
        // The file must NOT have been modified.
        assert_eq!(fs::read_to_string(dir.path().join("big.txt")).unwrap(), content);

        // Read the rest, then write succeeds.
        ex.execute("read_file", &serde_json::json!({"path":"big.txt","offset":2001,"limit":3000})).unwrap();
        let ok = ex.execute("write_file", &serde_json::json!({"path":"big.txt","content":"ok now"})).unwrap();
        assert!(ok.contains("wrote"), "write after full read: {ok}");
        assert_eq!(fs::read_to_string(dir.path().join("big.txt")).unwrap(), "ok now");
    }

    #[test]
    fn test_write_gate_allows_full_read_and_new_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("small.txt"), "a\nb\n").unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s2");
        ex.execute("read_file", &serde_json::json!({"path":"small.txt"})).unwrap();
        // full window == whole file → gate passes
        let ok = ex.execute("write_file", &serde_json::json!({"path":"small.txt","content":"z"})).unwrap();
        assert!(ok.contains("wrote"));
        // brand-new files are never gated
        let n = ex.execute("write_file", &serde_json::json!({"path":"new.txt","content":"hi"})).unwrap();
        assert!(n.contains("wrote"));
    }

    #[test]
    fn test_dedup_serves_stub_once_then_content() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "one\ntwo\n").unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s3");

        let first = ex.execute("read_file", &serde_json::json!({"path":"f.txt","limit":1})).unwrap();
        assert!(first.contains("one"));
        let stub = ex.execute("read_file", &serde_json::json!({"path":"f.txt","limit":1})).unwrap();
        assert!(stub.contains("unchanged"), "dedup stub: {stub}");
        // Self-expiring: the next identical read returns real content again.
        let again = ex.execute("read_file", &serde_json::json!({"path":"f.txt","limit":1})).unwrap();
        assert!(again.contains("one") && !again.contains("unchanged"), "expired: {again}");
    }

    #[test]
    fn test_dedup_misses_on_different_window() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "a\nb\nc\nd\n").unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s4");
        ex.execute("read_file", &serde_json::json!({"path":"f.txt","limit":2})).unwrap();
        let r = ex.execute("read_file", &serde_json::json!({"path":"f.txt","offset":1,"limit":3})).unwrap();
        assert!(r.contains("3 | c"), "different window → real content: {r}");
    }

    #[test]
    fn test_write_clears_seen_records() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "hello\n").unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s5");
        ex.execute("read_file", &serde_json::json!({"path":"f.txt"})).unwrap();
        ex.execute("write_file", &serde_json::json!({"path":"f.txt","content":"changed\n"})).unwrap();
        // Post-write read must NOT dedup against the pre-write content.
        let r = ex.execute("read_file", &serde_json::json!({"path":"f.txt"})).unwrap();
        assert!(r.contains("changed") && !r.contains("unchanged"), "fresh read after write: {r}");
    }

    #[test]
    fn test_dedup_kill_switch_env() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "x\ny\n").unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s6");
        std::env::set_var("PRAGMA_NO_DEDUP", "1");
        let first = ex.execute("read_file", &serde_json::json!({"path":"f.txt"})).unwrap();
        let second = ex.execute("read_file", &serde_json::json!({"path":"f.txt"})).unwrap();
        std::env::remove_var("PRAGMA_NO_DEDUP");
        assert!(first.contains("x"));
        assert!(second.contains("x") && !second.contains("unchanged"), "dedup off: {second}");
    }

    #[test]
    fn test_dedup_no_stub_for_facts() {
        let dir = tempdir().unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s7");
        // A missing file's fact must re-read as "not found" every time — a
        // dedup stub claiming "identical content" about nothing is a lie.
        let first = ex.execute("read_file", &serde_json::json!({"path":"missing.txt"})).unwrap();
        let second = ex.execute("read_file", &serde_json::json!({"path":"missing.txt"})).unwrap();
        assert!(first.contains("not found"), "fact: {first}");
        assert!(second.contains("not found") && !second.contains("unchanged"), "no stub for facts: {second}");
        // Binary facts likewise never stub.
        fs::write(dir.path().join("img.png"), [0x89u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).unwrap();
        let b1 = ex.execute("read_file", &serde_json::json!({"path":"img.png"})).unwrap();
        let b2 = ex.execute("read_file", &serde_json::json!({"path":"img.png"})).unwrap();
        assert!(b1.contains("image/png") && b2.contains("image/png") && !b2.contains("unchanged"), "binary facts fresh: {b2}");
    }

    #[test]
    fn test_dedup_mtime_guard() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "same size\n").unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s8");
        let first = ex.execute("read_file", &serde_json::json!({"path":"f.txt"})).unwrap();
        assert!(first.contains("same size"));
        // Rewrite with identical byte length — a size-only dedup would stall.
        fs::write(dir.path().join("f.txt"), "CHANGED!!1\n").unwrap();
        let second = ex.execute("read_file", &serde_json::json!({"path":"f.txt"})).unwrap();
        assert!(second.contains("CHANGED") && !second.contains("unchanged"), "mtime guard: {second}");
    }

    #[test]
    fn test_edit_gate_denies_partial_overwrite() {
        let dir = tempdir().unwrap();
        let content = (1..=4000).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        fs::write(dir.path().join("big.rs"), &content).unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s9");
        ex.execute("read_file", &serde_json::json!({"path":"big.rs"})).unwrap(); // head only
        let deny = ex.execute("edit_file", &serde_json::json!({"path":"big.rs","hash":file_ops::file_tag(&content),"start_line":3000,"content":"CHANGED"})).unwrap();
        assert!(deny.contains("denied") && deny.contains("offset=2001"), "edit gated: {deny}");
        assert_eq!(fs::read_to_string(dir.path().join("big.rs")).unwrap(), content, "file untouched");
        // Read the rest → the same edit applies.
        ex.execute("read_file", &serde_json::json!({"path":"big.rs","offset":2001,"limit":3000})).unwrap();
        let ok = ex.execute("edit_file", &serde_json::json!({"path":"big.rs","hash":file_ops::file_tag(&content),"start_line":3000,"content":"CHANGED"})).unwrap();
        assert!(ok.contains("edited"), "edit after full read: {ok}");
        assert!(fs::read_to_string(dir.path().join("big.rs")).unwrap().contains("CHANGED"));
    }

    // Pull the hex after "hash: " out of a read header — tolerant of the
    // "; offset=…" hint that follows it on partial-window reads.
    fn hash_of(read: &str) -> String {
        read.split("hash: ")
            .nth(1)
            .unwrap()
            .split(|c: char| !c.is_ascii_hexdigit())
            .next()
            .unwrap()
            .to_string()
    }

    #[test]
    fn test_edit_stale_anchor_names_drifted_lines() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "a\nb\nc\nd\ne\n").unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s-drift");
        let read = ex.execute("read_file", &serde_json::json!({"path":"f.txt"})).unwrap();
        let hash = hash_of(&read);
        // External edit elsewhere in the file (line 1) — the anchor goes stale.
        fs::write(dir.path().join("f.txt"), "A\nb\nc\nd\ne\n").unwrap();
        let r = ex.execute("edit_file", &serde_json::json!({
            "path":"f.txt", "hash": hash, "start_line": 3, "content": "CC"
        })).unwrap_err();
        assert!(r.contains("hash mismatch"), "error: {r}");
        assert!(r.contains("lines 1-1"), "should name the drifted line: {r}");
        assert_eq!(fs::read_to_string(dir.path().join("f.txt")).unwrap(), "A\nb\nc\nd\ne\n", "file untouched");
    }

    #[test]
    fn test_edit_subwindow_whole_file_hash_succeeds() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "a\nb\nc\nd\ne\nf\n").unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s-sub");
        let read = ex.execute("read_file", &serde_json::json!({"path":"f.txt","limit":2})).unwrap();
        let hash = hash_of(&read);
        // Edit a line INSIDE the read window with the header's whole-file hash.
        let ok = ex.execute("edit_file", &serde_json::json!({
            "path":"f.txt", "hash": hash, "start_line": 2, "content": "BB"
        })).unwrap();
        assert!(ok.contains("edited"), "sub-window edit must succeed: {ok}");
        assert_eq!(fs::read_to_string(dir.path().join("f.txt")).unwrap(), "a\nBB\nc\nd\ne\nf\n");
    }

    #[test]
    fn test_edit_required_hash_and_endline_default() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s-req");
        // Missing hash → hard error telling the model to re-read.
        let miss = ex.execute("edit_file", &serde_json::json!({"path":"f.txt","start_line":1,"content":"x"})).unwrap_err();
        assert!(miss.contains("missing hash"), "error: {miss}");
        // Omitted end_line → single-line replace; drift text via ledger snapshot.
        let read = ex.execute("read_file", &serde_json::json!({"path":"f.txt"})).unwrap();
        let hash = hash_of(&read);
        let ok = ex.execute("edit_file", &serde_json::json!({
            "path":"f.txt", "hash": hash, "start_line": 2, "content": "B2"
        })).unwrap();
        assert!(ok.contains("replaced lines 2-2"), "single-line default: {ok}");
        // Insertion still works via explicit end_line = start - 1.
        let read2 = ex.execute("read_file", &serde_json::json!({"path":"f.txt"})).unwrap();
        let hash2 = read2.split("hash: ").nth(1).unwrap().split(']').next().unwrap().to_string();
        let ok2 = ex.execute("edit_file", &serde_json::json!({
            "path":"f.txt", "hash": hash2, "start_line": 3, "end_line": 2, "content": "INS"
        })).unwrap();
        assert!(ok2.contains("inserted") || ok2.contains("replaced lines 3-2"), "insertion: {ok2}");
    }

    #[test]
    fn test_read_limit_zero_repaired() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        let r = file_ops::read_file(dir.path(), &serde_json::json!({"path":"f.txt","limit":0})).unwrap();
        assert!(r.contains("1 | a"), "limit 0 repaired to 1 line: {r}");
        assert!(!r.contains("1-0"), "no inverted window: {r}");
    }
}
