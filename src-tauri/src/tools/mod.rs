pub mod definitions;
pub mod file_ops;
pub mod shell;
pub mod git;
pub mod web;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Per-session "what has the model SEEN" of each file — feeds the write
/// gate (never overwrite a partially-seen file) and the one-shot read dedup.
pub type SeenLedger = Arc<std::sync::Mutex<HashMap<String, HashMap<PathBuf, SeenFile>>>>;

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
            "patch_file" => self.execute_patch(input),
            "create_directory" => file_ops::create_directory(&self.working_dir, input),
            "delete_file" => file_ops::delete_file(&self.working_dir, input),
            "move_file" => file_ops::move_file(&self.working_dir, input),
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
            entry.insert(
                d.abs_path.clone(),
                SeenFile {
                    mtime: mtime.unwrap_or(std::time::UNIX_EPOCH),
                    size,
                    window: d.window,
                    total: d.total,
                    partial: d.partial,
                    dedup: if d.window.is_some() { Some((size, d.window)) } else { None },
                },
            );
        }
        Ok(d.text)
    }

    /// The partial-view gate shared by every content-rewriting tool (write_file,
    /// patch_file): never touch a file the model has only partly seen — name
    /// the recovery instead. Returns the full path when the gate passes.
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

    /// edit_file with the same gate.
    fn execute_edit(&self, input: &serde_json::Value) -> Result<String, String> {
        let (Some(_), Some(_)) = (&self.session_id, &self.ledger) else {
            return file_ops::edit_file(&self.working_dir, input);
        };
        let path = input["path"].as_str().ok_or("missing path")?;
        let full = match self.gate_seen_write(path) {
            Ok(f) => f,
            Err(denied) => return Ok(denied),
        };
        let r = file_ops::edit_file(&self.working_dir, input);
        if let Some(f) = full {
            self.clear_seen(&f);
        }
        r
    }

    /// patch_file with the same gate: a hunk applied over unseen content is
    /// as blind as an overwrite.
    fn execute_patch(&self, input: &serde_json::Value) -> Result<String, String> {
        let (Some(_), Some(_)) = (&self.session_id, &self.ledger) else {
            return file_ops::patch_file(&self.working_dir, input);
        };
        let path = input["path"].as_str().ok_or("missing path")?;
        let full = match self.gate_seen_write(path) {
            Ok(f) => f,
            Err(denied) => return Ok(denied),
        };
        let r = file_ops::patch_file(&self.working_dir, input);
        if let Some(f) = full {
            self.clear_seen(&f);
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
    fn test_patch_gate_denies_partial_overwrite() {
        let dir = tempdir().unwrap();
        let content = (1..=4000).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        fs::write(dir.path().join("big.rs"), &content).unwrap();
        let (ex, _) = ledger_executor(dir.path(), "s9");
        ex.execute("read_file", &serde_json::json!({"path":"big.rs"})).unwrap(); // head only
        let diff = "@@ -3000,1 +3000,1 @@\n-line 3000\n+CHANGED\n";
        let deny = ex.execute("patch_file", &serde_json::json!({"path":"big.rs","diff":diff})).unwrap();
        assert!(deny.contains("write denied") && deny.contains("offset=2001"), "patch gated: {deny}");
        assert_eq!(fs::read_to_string(dir.path().join("big.rs")).unwrap(), content, "file untouched");
        // Read the rest → the same patch applies.
        ex.execute("read_file", &serde_json::json!({"path":"big.rs","offset":2001,"limit":3000})).unwrap();
        let ok = ex.execute("patch_file", &serde_json::json!({"path":"big.rs","diff":diff})).unwrap();
        assert!(ok.contains("patched"), "patch after full read: {ok}");
        assert!(fs::read_to_string(dir.path().join("big.rs")).unwrap().contains("CHANGED"));
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
