use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;
use walkdir::WalkDir;

const MAX_LINES: usize = 2_000; // line window — bounds ordinary long files
const MAX_BYTES: u64 = 128 * 1024; // byte budget — bounds wide files / logs
const MAX_LINE_CHARS: usize = 2_000; // per-line clamp — bounds minified bundles

pub fn abs(working_dir: &Path, rel: &str) -> PathBuf {
    working_dir.join(rel)
}

/// Path relative to the working directory (for display in results).
fn rel_of(wd: &Path, path: &Path) -> String {
    path.strip_prefix(wd).unwrap_or(path).display().to_string()
}

fn glob_matches(pattern: &str, rel: &str) -> bool {
    glob::Pattern::new(pattern)
        .map(|p| p.matches(rel))
        .unwrap_or(false)
}

/// Thin wrapper used by tests; the executor goes through `read_file_detailed`
/// (it needs the window/partial info for the ledger).
#[cfg_attr(not(test), allow(dead_code))]
pub fn read_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    read_file_detailed(wd, input).map(|r| r.text)
}

/// Structured read result — the executor uses `window`/`partial` for the
/// seen-files ledger (write gate); the model sees only `text`.
pub struct ReadDetail {
    pub text: String,
    /// Absolute path that was read (for ledger bookkeeping).
    pub abs_path: PathBuf,
    /// 1-based inclusive window that was actually served, if any.
    pub window: Option<(usize, usize)>,
    /// Line count, when the whole file fit in the byte budget.
    pub total: Option<usize>,
    /// True when more of the file exists beyond what was served.
    pub partial: bool,
    /// Whole-file content tag (stable across windows of the same file) —
    /// the value edit_file verifies. None for fact reads (binary/empty/etc.).
    pub tag: Option<String>,
    /// The canonical text the tag hashes (full file or the 128 KB head) —
    /// kept in the seen-ledger so a stale-edit mismatch can point at the
    /// exact drifted lines instead of "re-read the whole file".
    pub canonical: Option<String>,
}

pub fn read_file_detailed(wd: &Path, input: &serde_json::Value) -> Result<ReadDetail, String> {
    let path = parse_read_path(input)?;
    let full = abs(wd, &path);

    // ── Device blocklist, BEFORE any I/O (a read of /dev/zero never EOFs).
    if let Some(reason) = device_blocklist(&full) {
        return Ok(ReadDetail {
            text: format!("[{path}]: {reason}"),
            abs_path: full,
            window: None,
            total: None,
            partial: false,
            tag: None,
            canonical: None,
        });
    }

    // ── Adversarial filenames: the path the model typed may be byte-identical
    // to the real name only in a terminal (narrow NBSP, NFD/NFC, curly
    // quotes). Retry candidate spellings before failing; a repair must never
    // become an escape hatch, so every candidate stays inside the workspace.
    let mut repair_note: Option<String> = None;
    let full = if full.exists() {
        full
    } else if let Some((fixed, how)) = repair_path(wd, &path) {
        repair_note = Some(how);
        fixed
    } else {
        let sugg = did_you_mean(wd, &path);
        let text = if sugg.is_empty() {
            format!("[{path}]: not found — the file (or a parent directory) is missing")
        } else {
            format!("[{path}]: not found — did you mean: {sugg}?")
        };
        return Ok(ReadDetail {
            text,
            abs_path: full,
            window: None,
            total: None,
            partial: false,
            tag: None,
            canonical: None,
        });
    };

    let meta = match std::fs::metadata(&full) {
        Ok(m) => m,
        Err(e) => {
            return Ok(ReadDetail {
                text: format!("[{path}]: not found — {e}"),
                abs_path: full,
                window: None,
                total: None,
                partial: false,
                tag: None,
                canonical: None,
            })
        }
    };
    if !meta.is_file() {
        return Ok(ReadDetail {
            text: format!("[{path}]: not a regular file"),
            abs_path: full,
            window: None,
            total: None,
            partial: false,
            tag: None,
            canonical: None,
        });
    }

    // ── Boring formats get one-line answers: sniff magic bytes (never the
    // extension) so real text under a misleading extension still reads, and
    // garbage bytes never reach the model.
    {
        let mut f = File::open(&full).map_err(|e| format!("[{path}]: cannot open — {e}"))?;
        let mut head = [0u8; 8192];
        let n = f
            .read(&mut head)
            .map_err(|e| format!("[{path}]: read failed — {e}"))?;
        if let Some((mime, pdf)) = sniff_binary(&head[..n]) {
            let extra = if pdf {
                " — extract text with: pdftotext <file> - | head -n 200"
            } else {
                " — not readable as text"
            };
            return Ok(ReadDetail {
                text: format!("[{path}]: binary file ({mime}){extra}"),
                abs_path: full,
                window: None,
                total: None,
                partial: false,
                tag: None,
                canonical: None,
            });
        }
        // NUL in the head (without a known signature) → not text. Real text
        // files — even UTF-16 — would be caught here; that's the honest
        // one-liner rather than feeding junk to the model.
        if head[..n].contains(&0) {
            return Ok(ReadDetail {
                text: format!(
                    "[{path}]: binary file (application/octet-stream) — not readable as text"
                ),
                abs_path: full,
                window: None,
                total: None,
                partial: false,
                tag: None,
                canonical: None,
            });
        }
    }

    // 1-based offset (default 1); limit = max lines (default: until ceilings hit).
    // Repair degenerate inputs: offset/limit 0 mean "from the start / at least
    // one line" — never an empty window (an inverted "lines a-b" header).
    let offset = parse_index(input, "offset", 1)?.max(1);
    let limit = parse_index(input, "limit", usize::MAX)?.max(1);

    let size = meta.len();
    let byte_capped = size > MAX_BYTES;
    // The head is streamed through `take()` (never loaded whole) so a 400 MB
    // line before the window never accumulates — bounded memory per read.
    let mut raw = if byte_capped {
        let mut f = File::open(&full).map_err(|e| format!("[{path}]: cannot open — {e}"))?;
        read_prefix(&mut f, MAX_BYTES)?
    } else {
        std::fs::read_to_string(&full).map_err(|e| format!("[{path}]: cannot read — {e}"))?
    };

    // Hygiene: strip a leading UTF-8 BOM; lines() already normalises CRLF.
    if let Some(rest) = raw.strip_prefix('\u{feff}') {
        raw = rest.to_string();
    }
    if raw.is_empty() {
        return Ok(ReadDetail {
            text: format!("[{path}]: is empty"),
            abs_path: full,
            window: None,
            total: None,
            partial: false,
            tag: None,
            canonical: None,
        });
    }

    let all_lines: Vec<&str> = raw.lines().collect();
    let total = if byte_capped {
        None
    } else {
        Some(all_lines.len())
    };

    // Byte-budget reads only ever contain the file head (we can't seek by
    // line) — an offset past the loaded chunk gets a named recovery.
    if byte_capped {
        let first_offset = 1;
        if offset > first_offset && offset > all_lines.len() {
            return Ok(ReadDetail {
                text: format!("[{path}]: offset {offset} is past the loaded 128 KB — the file is larger than that; only the first {} lines are within the budget. Use search_files to find content, or read a smaller file.]", all_lines.len()),
                abs_path: full, window: None, total: None, partial: true,
                tag: None, canonical: None,
            });
        }
    }

    let start_0 = offset.saturating_sub(1);
    let end_0 = match total {
        Some(n) if start_0 >= n => {
            // Past EOF — name the recovery instead of failing.
            return Ok(ReadDetail {
                text: format!("[{path}]: offset {offset} is past the end — the file has {n} lines; retry smaller"),
                abs_path: full, window: None, total, partial: false, tag: None, canonical: None,
            });
        }
        _ => usize::MAX,
    };

    // Serve at most `limit` lines, and never more than the line window.
    let want = std::cmp::min(limit, MAX_LINES);
    let slice_beg = start_0;
    let slice_end = match end_0 {
        usize::MAX => slice_beg.saturating_add(want),
        n => std::cmp::min(slice_beg.saturating_add(want), n),
    };

    let shown: &[&str] = &all_lines[slice_beg.min(all_lines.len())..slice_end.min(all_lines.len())];
    let mut out = String::new();
    let mut clamped_any = false;
    for (i, l) in shown.iter().enumerate() {
        let lineno = slice_beg + i + 1;
        let (line_text, clamped) = clamp_line(l, lineno);
        if clamped {
            clamped_any = true;
        }
        out.push_str(&format!("{lineno:>4} | {line_text}\n"));
    }

    // "Is there more?" — line window hit while more lines exist, and/or the
    // byte budget stopped us before EOF.
    let last_shown = slice_beg + shown.len(); // exclusive
    let partial = byte_capped || matches!(total, Some(n) if last_shown < n);

    let first = slice_beg + 1;
    let last = last_shown;
    let next = last + 1;

    // Whole-file content tag: stable for any window of this file, and the
    // same value edit_file verifies. Computed over the loaded text (full file
    // or the 128 KB head), not the served window — a sub-window edit with
    // this hash must succeed, and ANY drift in the viewed region fails it.
    let content_hash = file_tag(&raw);

    let header = match total {
        Some(n) if n == last && first == 1 => format!("[{path}: {n} lines | hash: {content_hash}]"),
        Some(n) if partial => format!("[{path}: lines {first}-{last} of {n} | hash: {content_hash}; offset={next} to read the rest]"),
        Some(n) => format!("[{path}: lines {first}-{last} of {n} | hash: {content_hash}]"),
        None if partial => {
            // The head is all we ever load, so there is NO resume offset that
            // reaches past it — an "offset=N" hint here would silently loop
            // (re-reading returns the same head). Say so plainly (#6: never
            // emit a recovery that lies). `ends_with('\n')` + the stat make
            // the "is there more?" decision exact — never a guess.
            if raw.ends_with('\n') {
                format!("[{path}: lines {first}-{last} of more — the file exceeds the 128 KB read budget and this window ends exactly at the boundary; use search_files to find content beyond it | hash: {content_hash}]")
            } else {
                format!("[{path}: lines {first}-{last} of more — the file exceeds the 128 KB read budget and the last line shown was cut by it; there is no offset to resume with (re-reading returns the same head); use search_files to find content beyond the budget | hash: {content_hash}]")
            }
        }
        None => format!("[{path}: lines {first}-{last} | hash: {content_hash}]"),
    };
    let clamp_note = if clamped_any {
        "\n[single line(s) truncated at 2000 chars — see per-line notes]"
    } else {
        ""
    };
    let repair_note = match &repair_note {
        Some(how) => format!("\n[note: {how} — use the corrected name in future calls (a terminal may render both spellings identically)]"),
        None => String::new(),
    };

    Ok(ReadDetail {
        text: format!("{header}\n{out}{clamp_note}{repair_note}"),
        abs_path: full,
        window: Some((first, last)),
        total,
        partial,
        tag: Some(content_hash),
        canonical: Some(raw.clone()),
    })
}

/// Read a file up to `cap` bytes, then trim to the longest valid UTF-8 prefix
/// — a byte cap must never split a codepoint.
fn read_prefix(file: &mut File, cap: u64) -> Result<String, String> {
    let mut buf = Vec::with_capacity(cap.min(1 << 20) as usize);
    file.take(cap)
        .read_to_end(&mut buf)
        .map_err(|e| format!("read failed: {e}"))?;
    match String::from_utf8(buf) {
        Ok(s) => Ok(s),
        Err(e) => {
            let valid = e.utf8_error().valid_up_to();
            Ok(String::from_utf8(e.into_bytes()[..valid].to_vec()).unwrap_or_default())
        }
    }
}

/// Whole-file content tag for stale-edit detection. Hashes the file's
/// readable text (BOM-stripped) so the tag is stable across read windows and
/// identical between read_file's header and edit_file's verification.
pub(crate) fn file_tag(raw: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    raw.strip_prefix('\u{feff}')
        .unwrap_or(raw)
        .hash(&mut hasher);
    format!("{:08x}", hasher.finish() as u32)
}

/// Read the same text read_file_detailed would load for tag purposes (full
/// file, or the 128 KB head when the file exceeds the byte budget). Keeps
/// read↔edit tags identical for large files.
fn tag_source(full: &Path) -> Result<String, String> {
    match std::fs::metadata(full).map(|m| m.len()).unwrap_or(0) > MAX_BYTES {
        false => std::fs::read_to_string(full).map_err(|e| format!("read failed: {e}")),
        true => {
            let mut f = std::fs::File::open(full).map_err(|e| format!("open failed: {e}"))?;
            read_prefix(&mut f, MAX_BYTES)
        }
    }
}

/// Coarse but honest "what changed" report between two canonical texts:
/// trim common prefix/suffix lines, then report the remaining middle span in
/// 1-based NEW-file line numbers. Points a re-read at the drifted region
/// without shipping a full diff engine.
pub(crate) fn changed_line_ranges(old: &str, new: &str) -> Option<(usize, usize)> {
    let o: Vec<&str> = old.lines().collect();
    let n: Vec<&str> = new.lines().collect();
    let (mut lo, mut hi_o) = (0usize, o.len());
    let (mut ln, mut hi_n) = (0usize, n.len());
    while lo < hi_o && ln < hi_n && o[lo] == n[ln] {
        lo += 1;
        ln += 1;
    }
    while hi_o > lo && hi_n > ln && o[hi_o - 1] == n[hi_n - 1] {
        hi_o -= 1;
        hi_n -= 1;
    }
    if ln >= hi_n {
        return None; // no drift
    }
    Some((ln + 1, hi_n))
}

/// Candidate spellings for a path the model saw only in a terminal: narrow
/// no-break space (U+202F) and non-breaking space (U+00A0) render as regular
/// spaces, NFD/NFC decompose differently, curly quotes look like straight
/// ones. Retry these before failing a read.
fn spelling_candidates(orig: &str) -> Vec<String> {
    let mut out = vec![orig.to_string()];
    let swap_space = |s: &str| s.replace(['\u{202f}', '\u{00a0}'], " ");
    let swap_quotes = |s: &str| {
        s.replace(['\u{2018}', '\u{2019}'], "'")
            .replace(['\u{201c}', '\u{201d}'], "\"")
    };
    let curlyize = |s: &str| s.replace('\'', "\u{2019}").replace('\"', "\u{201d}");
    // macOS puts a narrow no-break space before AM/PM (".04 PM") while the
    // model types a regular space — so try every space individually swapped,
    // plus all-of-them, in both directions (interchangeable per the brief).
    let space_variants = |s: &str| -> Vec<String> {
        let spaces = s.chars().filter(|&c| c == ' ').count();
        let mut v = vec![s.replace(' ', "\u{202f}"), s.replace(' ', "\u{00a0}")];
        if spaces <= 3 {
            let chars: Vec<char> = s.chars().collect();
            for (i, &c) in chars.iter().enumerate() {
                if c == ' ' {
                    let mut a = chars.clone();
                    a[i] = '\u{202f}';
                    v.push(a.into_iter().collect());
                    let mut b = chars.clone();
                    b[i] = '\u{00a0}';
                    v.push(b.into_iter().collect());
                }
            }
        }
        v
    };
    let nfd: String = orig.nfd().collect();
    let nfc: String = orig.nfc().collect();
    let add = |s: String, out: &mut Vec<String>| {
        if s != orig && !out.contains(&s) {
            out.push(s);
        }
    };
    for s in space_variants(orig) {
        add(s, &mut out);
    }
    add(swap_space(orig), &mut out);
    add(swap_quotes(orig), &mut out);
    add(curlyize(orig), &mut out);
    add(nfd.clone(), &mut out);
    add(nfc, &mut out);
    add(swap_quotes(&nfd), &mut out);
    add(swap_space(&nfd), &mut out);
    for s in space_variants(&nfd) {
        add(s, &mut out);
    }
    out
}

/// Retry candidate spellings against the workspace (never escaping it). A
/// repair is only accepted when it resolves to an existing file inside wd —
/// a repair must never quietly become an escape hatch.
fn repair_path(wd: &Path, rel: &str) -> Option<(PathBuf, String)> {
    let wd_canon = wd.canonicalize().unwrap_or_else(|_| wd.to_path_buf());
    for cand in spelling_candidates(rel).into_iter().skip(1) {
        let p = abs(wd, &cand);
        if !p.is_file() {
            continue;
        }
        let inside = if let Ok(c) = p.canonicalize() {
            c.starts_with(&wd_canon)
        } else {
            false
        };
        if !inside {
            continue;
        }
        return Some((
            p,
            format!("filename corrected from \"{rel}\" to \"{cand}\""),
        ));
    }
    None
}

/// Bounded Levenshtein distance (full DP; names are short, dirs are bounded).
fn lev(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        cur[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
}

/// Suggest up to 3 candidate names when a read misses: bounded scan of the
/// requested file's parent dir, ranked by Levenshtein (≤2) + substring.
fn did_you_mean(wd: &Path, rel: &str) -> String {
    let base = match Path::new(rel).file_name() {
        Some(b) => b.to_string_lossy().to_string(),
        None => return String::new(),
    };
    let parent = Path::new(rel)
        .parent()
        .filter(|p| !p.as_os_str().is_empty());
    let dir = wd.join(parent.unwrap_or_else(|| Path::new(".")));
    if !dir.is_dir() {
        return String::new();
    }
    let mut scored: Vec<(usize, String)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for (i, entry) in rd.flatten().enumerate() {
            if i > 500 {
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let d = lev(&base, &name);
            let sub = name.contains(&base) || base.contains(&name);
            if d <= 2 || (sub && d <= base.chars().count().max(1) * 2 + 2) {
                scored.push((d, name));
            }
        }
    }
    scored.sort_by_key(|(d, n)| (*d, n.to_lowercase()));
    scored.truncate(3);
    scored
        .into_iter()
        .map(|(_, n)| n)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Sniff magic bytes (never the extension — a text file renamed .png, or a
/// PNG renamed .svg, is judged by content, not name). Returns (mime, pdf).
fn sniff_binary(head: &[u8]) -> Option<(&'static str, bool)> {
    if head.len() >= 4 && &head[..4] == b"\x89PNG" {
        return Some(("image/png", false));
    }
    if head.len() >= 3 && head[..3] == [0xff, 0xd8, 0xff] {
        return Some(("image/jpeg", false));
    }
    if head.len() >= 6 && (&head[..6] == b"GIF87a" || &head[..6] == b"GIF89a") {
        return Some(("image/gif", false));
    }
    if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Some(("image/webp", false));
    }
    if head.len() >= 5 && &head[..5] == b"%PDF-" {
        return Some(("application/pdf", true));
    }
    if head.len() >= 3 && head[..3] == [0x1f, 0x8b, 0x08] {
        return Some(("application/gzip", false));
    }
    if head.len() >= 2 && &head[..2] == b"PK" {
        return Some(("application/zip", false));
    }
    if head.len() >= 4 && &head[..4] == b"\x7fELF" {
        return Some(("application/x-elf", false));
    }
    if head.len() >= 4 && &head[..4] == b"\x00asm" {
        return Some(("application/wasm", false));
    }
    if head.len() >= 16 && &head[..16] == b"SQLite format 3\x00" {
        return Some(("application/vnd.sqlite3", false));
    }
    None
}

/// Clamp a single line to `MAX_LINE_CHARS` with a named note (a minified
/// bundle must never hand back one unusable mega-string).
fn clamp_line(line: &str, lineno: usize) -> (String, bool) {
    if line.chars().count() <= MAX_LINE_CHARS {
        return (line.to_string(), false);
    }
    let cut: String = line.chars().take(MAX_LINE_CHARS).collect();
    let note = format!(
        "[line {lineno} truncated at {MAX_LINE_CHARS} chars — total {})]",
        line.chars().count()
    );
    (format!("{cut}{note}"), true)
}

/// Never open these files — a read of /dev/zero hangs forever (a DoS you
/// shipped yourself), and the workspace boundary won't save you when cwd is /.
fn device_blocklist(full: &Path) -> Option<String> {
    let canon = full.canonicalize().unwrap_or_else(|_| full.to_path_buf());
    let s = canon.to_string_lossy();
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty()).collect();
    if parts.first() != Some(&"dev") && parts.first() != Some(&"proc") {
        return None;
    }
    let name = parts.get(1).copied().unwrap_or("");
    match parts[0] {
        "dev" => {
            if matches!(
                name,
                "zero" | "random" | "urandom" | "stdin" | "stdout" | "stderr"
            ) {
                Some(format!(
                    "refused — /dev/{name} is a special device that never reads like a file"
                ))
            } else if name == "fd" {
                Some("refused — /dev/fd/* are process fds, not readable files".into())
            } else if name == "mem" {
                Some("refused — /dev/mem is physical memory".into())
            } else {
                None
            }
        }
        "proc" => {
            let pid_ok = parts.len() > 2 && parts[1].chars().all(|c| c.is_ascii_digit());
            if pid_ok && (parts.get(2) == Some(&"fd") || parts.get(2) == Some(&"mem")) {
                Some(format!(
                    "refused — /proc/{}/{} is process internals, not a workspace file",
                    parts[1], parts[2]
                ))
            } else if parts.get(1) == Some(&"kcore") {
                Some("refused — /proc/kcore is a kernel core dump".into())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Resolve the path argument, repairing common aliases instead of bouncing
/// the call (repair inputs, don't reject them).
fn parse_read_path(input: &serde_json::Value) -> Result<String, String> {
    for key in [
        "path",
        "filePath",
        "file_path",
        "absolutePath",
        "target",
        "filename",
    ] {
        if let Some(s) = input.get(key).and_then(|v| v.as_str()) {
            if !s.trim().is_empty() {
                return Ok(s.to_string());
            }
        }
    }
    Err(
        "missing path (accepted keys: path, filePath, file_path, absolutePath, target, filename)"
            .into(),
    )
}

/// Strict numeric coercion for offset/limit: integers or "2000"-style strings;
/// "2abc" and fractions are rejected, never silently read.
fn parse_index(input: &serde_json::Value, key: &str, default: usize) -> Result<usize, String> {
    let Some(v) = input.get(key) else {
        return Ok(default);
    };
    match v {
        serde_json::Value::Number(n) => n
            .as_u64()
            .map(|x| x as usize)
            .ok_or_else(|| format!("invalid {key}: {v} (must be a positive integer)")),
        serde_json::Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                return Ok(default);
            }
            if !t.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!(
                    "invalid {key}: \"{s}\" (must be an integer like 2000)"
                ));
            }
            t.parse::<usize>()
                .map_err(|_| format!("invalid {key}: \"{s}\" (too large)"))
        }
        _ => Err(format!("invalid {key}: must be an integer")),
    }
}

pub fn write_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    let content = input["content"].as_str().ok_or("missing content")?;
    let full = abs(wd, path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&full, content).map_err(|e| e.to_string())?;
    Ok(format!("wrote {} bytes to {path}", content.len()))
}

/// Edit_file with whole-file hash anchoring. `stale_canonical` is the text
/// the expected hash was computed over (from read_file's seen-ledger) — when
/// the current hash diverges it lets the error point at the exact drifted
/// lines instead of "re-read the whole file".
pub fn edit_file(
    wd: &Path,
    input: &serde_json::Value,
    stale_canonical: Option<&str>,
) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    let start = input["start_line"]
        .as_u64()
        .ok_or("missing start_line (must be a positive integer)")? as usize;
    // end_line is OPTIONAL — omitted = single-line replace (start_line only).
    let end = input["end_line"].as_u64().unwrap_or(start as u64) as usize;
    let content = input["content"].as_str().ok_or("missing content")?;
    let hash = input["hash"]
        .as_str()
        .ok_or("missing hash — pass the hash from the latest read_file header; re-read the file to get the current hash")?;

    if start == 0 {
        return Err("start_line must be >= 1 (lines are 1-based)".into());
    }

    let full = abs(wd, path);
    let original = std::fs::read_to_string(&full).map_err(|e| format!("read failed: {e}"))?;
    let lines: Vec<&str> = original.lines().collect();
    let total = lines.len();
    let trailing_newline = original.ends_with('\n');

    // Allow start == end + 1 as an insertion point (no lines replaced).
    if start > end + 1 {
        return Err(format!(
            "start_line ({start}) must be <= end_line ({end}) — or exactly end_line+1 for a pure insertion"
        ));
    }
    if start > total + 1 {
        return Err(format!(
            "start_line ({start}) is past the end of the file ({total} lines)"
        ));
    }
    if end > total {
        return Err(format!(
            "end_line ({end}) is past the end of the file ({total} lines)"
        ));
    }

    let old_start = start - 1; // 0-based
    let old_end = end.min(total); // inclusive → exclusive
    let old_lines = &lines[old_start..old_end];
    let old_count = old_end - old_start;

    // Verify the whole-file tag: the file must still be exactly what the last
    // read served — any drift anywhere fails the edit (stale-anchor reject).
    let actual_tag = file_tag(&tag_source(&full)?);
    if !hash.eq_ignore_ascii_case(&actual_tag) {
        let snippet: String = old_lines
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, l)| format!("  {:>4} | {l}", old_start + i + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let drift = stale_canonical
            .and_then(|old| changed_line_ranges(old, &original))
            .map(|(a, b)| format!(" The file changed since you read it — currently at lines {a}-{b} (re-read that range or the whole file, then retry with the new hash)."))
            .unwrap_or_else(|| " The file may have changed since you read it — re-read it and retry with the new hash.".to_string());
        return Err(format!(
            "hash mismatch: expected {hash} but the file has {actual_tag}.{drift}\n\
             Current content at your target lines {start}-{end}:\n{snippet}"
        ));
    }

    // Build the new file content.
    let new_lines: Vec<&str> = if content.is_empty() {
        Vec::new()
    } else {
        content.lines().collect()
    };
    let new_count = new_lines.len();

    let mut result: Vec<&str> = Vec::with_capacity(total + new_count);
    result.extend_from_slice(&lines[..old_start]);
    result.extend_from_slice(&new_lines);
    result.extend_from_slice(&lines[old_end..]);

    let mut output = result.join("\n");
    if trailing_newline || (!result.is_empty() && original.ends_with('\n')) {
        output.push('\n');
    }
    std::fs::write(&full, &output).map_err(|e| e.to_string())?;

    Ok(format!(
        "edited {path}: replaced lines {start}-{end} ({old_count} lines \u{2192} {new_count} lines); hash now {}",
        file_tag(&output)
    ))
}

pub fn delete_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    let full = abs(wd, path);
    std::fs::remove_file(&full).map_err(|e| format!("{e}: {}", full.display()))?;
    Ok(format!("deleted {path}"))
}

pub fn list_files(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let subdir = input["path"].as_str().unwrap_or(".");
    let pattern = input["pattern"].as_str();
    // depth 0 means unlimited; default 1 = immediate children only.
    let depth = input["depth"].as_u64().unwrap_or(1);
    let base = abs(wd, subdir);
    let mut walker = WalkDir::new(&base).min_depth(1);
    if depth > 0 {
        walker = walker.max_depth(depth as usize);
    }
    let mut entries = Vec::new();
    for entry in walker {
        let entry = entry.map_err(|e| e.to_string())?;
        let is_dir = entry.file_type().is_dir();
        let rel = rel_of(wd, entry.path());
        let display = if is_dir {
            format!("{rel}/")
        } else {
            rel.clone()
        };
        if let Some(pat) = pattern {
            if !glob_matches(pat, &rel) {
                continue;
            }
        }
        entries.push(display);
    }
    Ok(if entries.is_empty() {
        "(empty)".into()
    } else {
        entries.join("\n")
    })
}

pub fn search_files(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let pattern = input["pattern"].as_str().ok_or("missing pattern")?;
    let re = regex::Regex::new(pattern).map_err(|e| e.to_string())?;
    // Degenerate-pattern guard: a regex that matches the empty string (.*, ^,
    // a?, | …) matches every line of every file — scanning the whole workspace
    // and flooding the context window. Give guidance instead of running it.
    if re.is_match("") {
        return Ok(format!(
            "[search rejected]: the pattern {pattern:?} matches the empty string, so it would match nearly every line of every file. Use a more specific pattern (identifiers, keywords, error strings)."
        ));
    }
    let subdir = input["path"].as_str().unwrap_or(".");
    let file_glob = input["glob"].as_str();
    // Cap emitted matches so a broad pattern can't blow the context window;
    // the (possibly much larger) total is reported so the model knows to
    // narrow the pattern or page through.
    let max_results = input["max_results"]
        .as_u64()
        .map(|n| n.max(1) as usize)
        .unwrap_or(50);
    let base = abs(wd, subdir);
    let mut emitted: Vec<String> = Vec::new();
    let mut total = 0usize;
    for entry in WalkDir::new(&base) {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = rel_of(wd, entry.path());
        if let Some(g) = file_glob {
            if !glob_matches(g, &rel) {
                continue;
            }
        }
        if let Ok(text) = std::fs::read_to_string(entry.path()) {
            for (i, line) in text.lines().enumerate() {
                if re.is_match(line) {
                    total += 1;
                    if emitted.len() < max_results {
                        emitted.push(format!("{rel}:{}: {line}", i + 1));
                    }
                }
            }
        }
    }
    if emitted.is_empty() {
        return Ok("(no matches)".into());
    }
    let mut out = emitted.join("\n");
    if total > emitted.len() {
        out.push_str(&format!(
            "\n…showing {0} of {total} matches (set max_results higher or narrow the pattern to see more)",
            emitted.len()
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_write_then_read() {
        let dir = tempdir().unwrap();
        write_file(
            dir.path(),
            &serde_json::json!({"path":"hello.txt","content":"hello world"}),
        )
        .unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"hello.txt"})).unwrap();
        assert!(r.contains("hello world"), "expected content in: {r}");
        assert!(
            r.contains("[hello.txt: 1 lines | hash:"),
            "expected header in: {r}"
        );
    }

    // Whole-file hash of a literal file body (what read_file's header would
    // carry) — the value edit_file now REQUIRES.
    fn tag(body: &str) -> String {
        file_tag(body)
    }

    #[test]
    fn test_edit_file_basic() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
        let r = edit_file(dir.path(), &serde_json::json!({
            "path": "f.txt", "hash": tag("line1\nline2\nline3\n"), "start_line": 2, "content": "LINE2"
        }), None).unwrap();
        assert!(r.contains("replaced lines 2-2"), "result: {r}");
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "line1\nLINE2\nline3\n");
    }

    #[test]
    fn test_edit_file_multiline() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "a\nb\nc\nd\ne\n").unwrap();
        edit_file(dir.path(), &serde_json::json!({
            "path": "f.txt", "hash": tag("a\nb\nc\nd\ne\n"), "start_line": 2, "end_line": 4, "content": "X\nY"
        }), None).unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nX\nY\ne\n");
    }

    #[test]
    fn test_edit_file_delete() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        edit_file(
            dir.path(),
            &serde_json::json!({
                "path": "f.txt", "hash": tag("a\nb\nc\n"), "start_line": 2, "content": ""
            }),
            None,
        )
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nc\n");
    }

    #[test]
    fn test_edit_file_hash_match() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
        // The hash from read_file's header covers the WHOLE file, so a
        // sub-window edit (line 2 of 3) passes just like a full-window one.
        let hash = { file_tag(&fs::read_to_string(dir.path().join("f.txt")).unwrap()) };
        edit_file(
            dir.path(),
            &serde_json::json!({
                "path": "f.txt", "hash": hash, "start_line": 2, "content": "LINE2"
            }),
            None,
        )
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "line1\nLINE2\nline3\n");
    }

    #[test]
    fn test_edit_file_hash_mismatch() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
        let r = edit_file(
            dir.path(),
            &serde_json::json!({
                "path": "f.txt", "hash": "deadbeef", "start_line": 2, "content": "LINE2"
            }),
            None,
        );
        assert!(r.is_err(), "should fail on hash mismatch");
        let e = r.unwrap_err();
        assert!(e.contains("hash mismatch"), "error: {e}");
        // File should be unchanged.
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "line1\nline2\nline3\n");
    }

    #[test]
    fn test_edit_file_out_of_range() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "a\nb\n").unwrap();
        let r = edit_file(
            dir.path(),
            &serde_json::json!({
                "path": "f.txt", "hash": tag("a\nb\n"), "start_line": 5, "content": "x"
            }),
            None,
        );
        assert!(r.is_err(), "should fail for out of range");
    }

    #[test]
    fn test_create_dir_and_delete() {
        let dir = tempdir().unwrap();
        write_file(
            dir.path(),
            &serde_json::json!({"path":"sub/nested/x.txt","content":"x"}),
        )
        .unwrap();
        assert!(dir.path().join("sub/nested/x.txt").exists());
        delete_file(dir.path(), &serde_json::json!({"path":"sub/nested/x.txt"})).unwrap();
        assert!(!dir.path().join("sub/nested/x.txt").exists());
    }

    #[test]
    fn test_list_files_with_pattern() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "").unwrap();
        fs::write(dir.path().join("b.ts"), "").unwrap();
        let r = list_files(dir.path(), &serde_json::json!({"pattern":"*.rs"})).unwrap();
        assert!(r.contains("a.rs"));
        assert!(!r.contains("b.ts"));
    }

    #[test]
    fn test_search_files() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("main.rs"),
            "fn main() { println!(\"hello\"); }",
        )
        .unwrap();
        let r = search_files(dir.path(), &serde_json::json!({"pattern":"println"})).unwrap();
        assert!(r.contains("main.rs"));
    }

    #[test]
    fn search_rejects_everything_patterns() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello\nworld\n").unwrap();
        for bad in [".*", "^", "a?", ".*.*"] {
            let r = search_files(dir.path(), &serde_json::json!({"pattern": bad})).unwrap();
            assert!(
                r.contains("[search rejected]"),
                "pattern {:?} must be rejected, got: {r}",
                bad
            );
        }
    }

    #[test]
    fn search_caps_matches_and_reports_total() {
        let dir = tempdir().unwrap();
        let mut content = String::new();
        for i in 0..100 {
            content.push_str(&format!("needle {i}\n"));
        }
        fs::write(dir.path().join("many.txt"), content).unwrap();
        // Default cap of 50 with a total-count note.
        let r = search_files(dir.path(), &serde_json::json!({"pattern": "needle"})).unwrap();
        assert!(
            r.contains("…showing 50 of 100 matches"),
            "got: {}",
            &r[r.len().saturating_sub(80)..]
        );
        assert_eq!(r.matches("needle ").count(), 50);
        // max_results raises the emitted count.
        let r = search_files(
            dir.path(),
            &serde_json::json!({"pattern": "needle", "max_results": 75}),
        )
        .unwrap();
        assert_eq!(r.matches("needle ").count(), 75);
        // Under the cap: no note.
        let r = search_files(dir.path(), &serde_json::json!({"pattern": "needle 1"})).unwrap();
        assert!(!r.contains("…showing"));
    }

    // ── read_file engineering (M13): ceilings, hints, hygiene, guards ──

    #[test]
    fn test_read_line_window_hint() {
        let dir = tempdir().unwrap();
        let content = (1..=4000)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(dir.path().join("big.txt"), &content).unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"big.txt"})).unwrap();
        assert!(
            r.contains("big.txt: lines 1-2000 of 4000 | hash:"),
            "header in: {r}"
        );
        assert!(r.contains("1 | line 1"), "numbered start: {r}");
        assert!(r.contains("2000 | line 2000"), "numbered end: {r}");
        assert!(
            !r.contains("line 2001"),
            "must not include beyond the window: {r}"
        );
        // Paginate with the hint.
        let r2 = read_file(
            dir.path(),
            &serde_json::json!({"path":"big.txt","offset":2001,"limit":3}),
        )
        .unwrap();
        assert!(r2.contains("2001 | line 2001"));
        assert!(r2.contains("2003 | line 2003"));
        assert!(r2.contains("lines 2001-2003 of 4000"));
    }

    #[test]
    fn test_read_byte_cap_large_file() {
        let dir = tempdir().unwrap();
        // 256 KB of "x" lines → byte-capped at 128 KB.
        let mut content = String::new();
        for i in 1..=9000 {
            content.push_str(&format!(
                "{i} xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n"
            ));
        }
        fs::write(dir.path().join("huge.log"), &content).unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"huge.log"})).unwrap();
        assert!(r.contains("128 KB"), "byte-budget note in: {r}");
        assert!(r.contains("to resume"), "resume hint in: {r}");
        // Must not contain lines past the chunk.
        assert!(!r.contains("9000"), "late line leaked: {r}");
    }

    #[test]
    fn test_read_line_clamp() {
        let dir = tempdir().unwrap();
        let long = "y".repeat(50_000);
        fs::write(
            dir.path().join("bundle.js"),
            format!("let x = \"{long}\";\n"),
        )
        .unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"bundle.js"})).unwrap();
        assert!(
            r.contains("[line 1 truncated at 2000 chars"),
            "clamp note in: {r}"
        );
        assert!(r.contains("total 500"), "clamp total in: {r}");
    }

    #[test]
    fn test_read_empty_and_past_eof() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("empty.txt"), "").unwrap();
        fs::write(dir.path().join("few.txt"), "a\nb\nc\n").unwrap();
        let e = read_file(dir.path(), &serde_json::json!({"path":"empty.txt"})).unwrap();
        assert!(e.contains("is empty"), "empty fact: {e}");
        let p = read_file(
            dir.path(),
            &serde_json::json!({"path":"few.txt","offset":9}),
        )
        .unwrap();
        assert!(
            p.contains("past the end") && p.contains("retry smaller"),
            "eof fact: {p}"
        );
    }

    #[test]
    fn test_read_bom_and_crlf() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("bom.txt"), "\u{feff}hello\r\nworld\r\n").unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"bom.txt"})).unwrap();
        assert!(!r.contains("\u{feff}"), "BOM stripped: {r}");
        assert!(
            r.contains("1 | hello") && r.contains("2 | world"),
            "CRLF normalised: {r}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_read_device_blocklist() {
        let dir = tempdir().unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"/dev/zero"})).unwrap();
        assert!(r.contains("refused"), "device refused: {r}");
        let r2 = read_file(dir.path(), &serde_json::json!({"path":"/proc/1/fd/0"})).unwrap();
        assert!(r2.contains("refused"), "proc fd refused: {r2}");
    }

    #[test]
    fn test_read_input_repair() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        // Alias keys are repaired, not bounced.
        let r = read_file(dir.path(), &serde_json::json!({"filePath":"f.txt"})).unwrap();
        assert!(r.contains("f.txt"), "alias accepted: {r}");
        // Strict numeric coercion.
        let ok = read_file(
            dir.path(),
            &serde_json::json!({"path":"f.txt","offset":"2"}),
        )
        .unwrap();
        assert!(ok.contains("2 | b"), "numeric string accepted: {ok}");
        assert!(
            read_file(
                dir.path(),
                &serde_json::json!({"path":"f.txt","offset":"2abc"})
            )
            .is_err(),
            "garbage string rejected"
        );
        assert!(
            read_file(
                dir.path(),
                &serde_json::json!({"path":"f.txt","offset":1.5})
            )
            .is_err(),
            "fraction rejected"
        );
    }

    #[test]
    fn test_read_utf8_never_split_on_byte_cap() {
        let dir = tempdir().unwrap();
        // Land the 4-byte emoji exactly at the 128 KB boundary so the capped
        // read would split it — read_prefix must trim to a valid prefix.
        let boundary = MAX_BYTES as usize - 2;
        let content = format!("{}😀", "a".repeat(boundary));
        fs::write(dir.path().join("utf.txt"), &content).unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"utf.txt"})).unwrap();
        assert!(!r.contains('\u{fffd}'), "no replacement char: {r}");
        assert!(r.contains("128 KB"), "still byte-capped: {r}");
    }

    // ── M15: adversarial filenames + did-you-mean ──

    #[test]
    fn test_spelling_repair_narrow_space() {
        let dir = tempdir().unwrap();
        // macOS screenshots use U+202F NARROW NO-BREAK SPACE — a terminal
        // renders it as a regular space, so the model asks with U+0020.
        fs::write(
            dir.path().join("Screenshot 3.04\u{202f}PM.txt"),
            "the bytes\n",
        )
        .unwrap();
        let r = read_file(
            dir.path(),
            &serde_json::json!({"path":"Screenshot 3.04 PM.txt"}),
        )
        .unwrap();
        assert!(r.contains("filename corrected"), "repair note in: {r}");
        assert!(r.contains("the bytes"), "content of repaired file: {r}");
    }

    #[test]
    fn test_spelling_repair_nfd() {
        let dir = tempdir().unwrap();
        // NFC "café.txt" written; the model requests the NFD spelling — the
        // bytes differ on screen in no way the model can see. macOS
        // (APFS/HFS+) stores filenames NFD-normalized, so on-disk may already
        // be the decomposed form — request whichever form the disk does NOT
        // hold, so the repair bridges NFC ↔ NFD in either direction.
        fs::write(dir.path().join("caf\u{e9}.txt"), "espresso\n").unwrap();
        let on_disk = fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name()
            .to_string_lossy()
            .to_string();
        let requested = if on_disk == "caf\u{e9}.txt" {
            "cafe\u{301}.txt"
        } else {
            "caf\u{e9}.txt"
        };
        let r = read_file(dir.path(), &serde_json::json!({"path": requested})).unwrap();
        assert!(r.contains("filename corrected"), "repair note in: {r}");
        assert!(r.contains("espresso"), "content of repaired file: {r}");
    }

    #[test]
    fn test_did_you_mean() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "agents\n").unwrap();
        fs::write(dir.path().join("README.md"), "readme\n").unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"AGENT.md"})).unwrap();
        assert!(
            r.contains("not found") && r.contains("did you mean"),
            "fact in: {r}"
        );
        assert!(r.contains("AGENTS.md"), "suggests AGENTS.md: {r}");
        assert!(!r.contains("README.md"), "README is not a close match: {r}");
    }

    #[test]
    fn test_spelling_repair_subdir() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/ma\u{ef}n.rs"), "fn x() {}\n").unwrap();
        // Same NFC↔NFD bridge as the test above, one level down: request the
        // form the filesystem does NOT store (macOS normalizes to NFD).
        let on_disk = fs::read_dir(dir.path().join("src"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name()
            .to_string_lossy()
            .to_string();
        let requested = if on_disk == "ma\u{ef}n.rs" {
            "ma\u{69}\u{308}n.rs"
        } else {
            "ma\u{ef}n.rs"
        };
        let r = read_file(
            dir.path(),
            &serde_json::json!({"path": format!("src/{requested}")}),
        )
        .unwrap();
        assert!(r.contains("filename corrected"), "repair note in: {r}");
        assert!(r.contains("fn x() {}"), "content of repaired file: {r}");
    }

    // ── M16: no looping byte-budget hints ──

    #[test]
    fn test_byte_cap_no_looping_offset() {
        let dir = tempdir().unwrap();
        let mut content = String::new();
        for i in 1..=9000 {
            content.push_str(&format!(
                "{i} xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n"
            ));
        }
        fs::write(dir.path().join("huge.log"), &content).unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"huge.log"})).unwrap();
        // The head is all we load, so an "offset=N to resume" hint would loop
        // forever (re-reading returns the same head). Must not offer one.
        assert!(!r.contains("offset="), "no looping offset hint: {r}");
        assert!(r.contains("search_files"), "search_files recovery: {r}");
    }

    // ── M17: boring formats get one-line answers ──

    #[test]
    fn test_binary_png_fact() {
        let dir = tempdir().unwrap();
        let png = [
            0x89u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x01, 0x02, 0x03,
        ];
        fs::write(dir.path().join("img.png"), png).unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"img.png"})).unwrap();
        assert!(
            r.contains("image/png") && r.contains("binary file"),
            "png fact: {r}"
        );
        assert!(!r.contains("\u{fffd}"), "no junk bytes: {r}");
    }

    #[test]
    fn test_pdf_hint() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("doc.pdf"),
            b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n\xff\xff\xff",
        )
        .unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"doc.pdf"})).unwrap();
        assert!(r.contains("application/pdf"), "pdf fact: {r}");
        assert!(r.contains("pdftotext"), "pdftotext hint: {r}");
    }

    #[test]
    fn test_svg_text_ok() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("icon.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"10\"/></svg>\n",
        )
        .unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"icon.svg"})).unwrap();
        assert!(r.contains("1 | <svg"), "svg is text, numbered: {r}");
        assert!(!r.contains("binary"), "no binary fact for text svg: {r}");
    }

    #[test]
    fn test_text_under_misleading_extension() {
        let dir = tempdir().unwrap();
        // A text file named .png must still read as text (magic bytes win).
        fs::write(dir.path().join("notes.png"), "plain text notes\n").unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"notes.png"})).unwrap();
        assert!(r.contains("1 | plain text notes"), "text under .png: {r}");
    }
}
