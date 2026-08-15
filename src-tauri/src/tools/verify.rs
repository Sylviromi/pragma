//! Pure helpers for the harness verification layer. Everything here is
//! deterministic / model-free except `build_review_messages` (which only
//! assembles the minimal, asymmetric prompt) — the actual reviewer call and
//! loop integration live in `commands/verify.rs` + `commands/agentic.rs`.
//!
//! The design principle (from the arXiv research, e.g. OpenCodeReview /
//! RETRACE): the reviewer must see ONLY the task + the diff + check output,
//! never the conversation — that information boundary is what makes the
//! verification independent of the agent's own interpretation.

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

/// Cap for the diff fed to the reviewer — a giant refactor must not blow the
/// reviewer's context (and cost). Lines beyond the cap are summarized away.
pub const DIFF_CAP_CHARS: usize = 24_000;
/// Cap for check output carried into the report / reviewer prompt.
pub const CHECK_OUTPUT_CAP_CHARS: usize = 6_000;
/// Default timeout for verification checks (tests/builds can be slow).
pub const CHECK_TIMEOUT_SECS: u64 = 300;

/// Detect the project's check/test command from its manifest. Returns
/// (label, shell command). Precedence: explicit settings override → Cargo →
/// npm. `None` when nothing detectable (review-only verification).
pub fn infer_test_command(wd: &Path) -> Option<(String, String)> {
    if wd.join("Cargo.toml").exists() {
        return Some((
            "cargo check + test".to_string(),
            "cargo check && cargo test".to_string(),
        ));
    }
    let pkg = wd.join("package.json");
    if pkg.exists() {
        // Prefer the project's own test script; fall back to a typecheck-style
        // check script (common in Svelte/Vite/TS projects) when present.
        if let Ok(text) = std::fs::read_to_string(&pkg) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                let scripts = &v["scripts"];
                if scripts["test"].as_str().is_some() {
                    return Some(("npm test".to_string(), "npm test".to_string()));
                }
                if scripts["check"].as_str().is_some() {
                    return Some(("npm run check".to_string(), "npm run check".to_string()));
                }
            }
        }
    }
    None
}

/// Capture the working tree's changes for the reviewer: `git diff` for tracked
/// files plus the contents of untracked files (git diff ignores those). Not a
/// git repo → empty diff (verification then relies on checks + review of the
/// touched files as described in the report). Returns (diff_text, file_list).
pub fn capture_diff(wd: &Path) -> (String, Vec<String>) {
    let mut diff = String::new();
    let mut files: Vec<String> = Vec::new();

    // Tracked changes.
    let tracked = run_git(wd, &["diff", "--no-color"]);
    if let Ok(t) = tracked {
        if t != "(no output)" {
            diff.push_str(&t);
        }
    }
    // Untracked files: report their names and include (capped) contents.
    let status = run_git(wd, &["status", "--porcelain"]);
    if let Ok(s) = status {
        for line in s.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("??") {
                let rel = rest.trim();
                let full = wd.join(rel);
                // Bare directories (status shows `?? dir/`) have no content to
                // read; real files get their (capped) contents included.
                if full.is_file() {
                    files.push(rel.to_string());
                    if let Ok(text) = std::fs::read_to_string(&full) {
                        diff.push_str(&format!("\n--- untracked: {rel} ---\n{text}"));
                    }
                }
            }
        }
    }

    if diff.chars().count() > DIFF_CAP_CHARS {
        let cut: String = diff.chars().take(DIFF_CAP_CHARS).collect();
        diff = format!("{cut}\n…[diff truncated]");
    }
    (diff, files)
}

/// Run a shell command to completion with a timeout. Returns (exit_success,
/// combined output capped). Kills the child on timeout (success=false,
/// output explains). Never blocks the loop forever.
pub fn run_command_status(wd: &Path, command: &str, timeout_secs: u64) -> (bool, String) {
    let timeout = Duration::from_secs(timeout_secs.max(1));
    let mut child = match std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(wd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return (false, format!("failed to spawn command: {e}")),
    };

    use std::io::Read;
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let mut err = Vec::new();
        if let Some(mut so) = stdout_pipe.take() {
            let _ = so.read_to_end(&mut out);
        }
        if let Some(mut se) = stderr_pipe.take() {
            let _ = se.read_to_end(&mut err);
        }
        (out, err)
    });

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return (false, format!("process error: {e}"));
            }
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return (
                false,
                format!("timed out after {}s — command killed", timeout.as_secs()),
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let (out, err) = reader.join().unwrap_or((Vec::new(), Vec::new()));
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out),
        String::from_utf8_lossy(&err)
    );
    let combined = if combined.trim().is_empty() {
        "(no output)".to_string()
    } else {
        combined
    };
    let combined = cap_chars(&combined, CHECK_OUTPUT_CAP_CHARS);
    (status.success(), combined)
}

/// Cap a string at N chars with an elision marker.
pub fn cap_chars(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        let cut: String = s.chars().take(n).collect();
        format!("{cut}\n…[output truncated: {} chars]", s.chars().count())
    } else {
        s.to_string()
    }
}

/// Assemble the reviewer's minimal prompt. THE ASYMMETRIC BOUNDARY: only the
/// task, the diff, and the check output — no conversation, no tool calls, no
/// plan. `checks` is "(none)" when checks were skipped or inferred as absent.
pub fn build_review_messages(task: &str, diff: &str, checks: &str) -> Vec<serde_json::Value> {
    let system = "\
You are an independent code reviewer for an AI coding agent. Your job is to decide whether the \
agent's change actually solves the user's task and is safe to accept. You see ONLY the task, the \
diff, and the project check output — nothing else (no conversation, no agent reasoning). Judge the \
change on its own merits.\n\
\n\
Rules:\n\
- If the project checks FAILED, the change must not be accepted.\n\
- If the diff clearly does not address the task, or introduces a regression, bug, or safety \
problem, the change must not be accepted.\n\
- Small style nits that do not affect correctness are NOT grounds for rejection.\n\
- Do not ask for scope creep or unrelated improvements.\n\
\n\
Respond in exactly this format:\n\
VERDICT: PASS\n\
or\n\
VERDICT: FIX\n\
FINDINGS:\n\
- file:line — specific, actionable issue (one per line, only when VERDICT: FIX)\n";
    let user = format!("TASK:\n{task}\n\nDIFF:\n{diff}\n\nPROJECT CHECKS OUTPUT:\n{checks}");
    vec![
        serde_json::json!({ "role": "system", "content": system }),
        serde_json::json!({ "role": "user", "content": user }),
    ]
}

/// Parse the reviewer's verdict. Returns (passes, findings). Conservative
/// fallback: an unparseable / empty response counts as FIX (the harness
/// prefers to show a report than silently accept).
pub fn parse_verdict(text: &str) -> (bool, Vec<String>) {
    let mut findings: Vec<String> = Vec::new();
    let mut passes = false;
    for line in text.lines() {
        let t = line.trim();
        let upper = t.to_ascii_uppercase();
        if upper.starts_with("VERDICT:") {
            passes = upper.contains("PASS");
            if upper.contains("FIX") {
                passes = false;
            }
        } else if t.starts_with('-')
            || t.chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        {
            let f = t.trim_start_matches(|c: char| c == '-' || c.is_ascii_whitespace());
            if !f.is_empty() {
                findings.push(f.to_string());
            }
        }
    }
    if !text.trim().is_empty() && !text.to_ascii_uppercase().contains("VERDICT:") {
        // Unparseable — treat as FIX with the raw tail as a single finding.
        let tail: String = cap_chars(text, 800);
        findings = vec![format!(
            "reviewer output not in expected format — full response:\n{tail}"
        )];
    }
    (passes, findings)
}

/// Run a git command in the working dir (verification-local; the app's git
/// tool lives in tools/git.rs but its internals are private).
fn run_git(wd: &Path, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(wd)
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if out.status.success() {
        Ok(if stdout.trim().is_empty() {
            "(no output)".into()
        } else {
            stdout
        })
    } else {
        Err(if stderr.trim().is_empty() {
            stdout
        } else {
            stderr
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn infers_cargo_test_from_manifest() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        let (label, cmd) = infer_test_command(dir.path()).unwrap();
        assert_eq!(label, "cargo check + test");
        assert_eq!(cmd, "cargo check && cargo test");
    }

    #[test]
    fn infers_npm_test_over_check() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"scripts": {"check": "svelte-check", "test": "vitest run"}}"#,
        )
        .unwrap();
        let (label, _) = infer_test_command(dir.path()).unwrap();
        assert_eq!(label, "npm test");
    }

    #[test]
    fn infers_nothing_for_bare_dir() {
        let dir = tempdir().unwrap();
        assert!(infer_test_command(dir.path()).is_none());
    }

    #[test]
    fn parses_pass_verdict() {
        let (pass, findings) = parse_verdict("VERDICT: PASS\n");
        assert!(pass);
        assert!(findings.is_empty());
    }

    #[test]
    fn parses_fix_verdict_with_findings() {
        let text = "VERDICT: FIX\nFINDINGS:\n- src/main.rs:12 — off-by-one in the loop\n- src/util.rs:3 — unused import";
        let (pass, findings) = parse_verdict(text);
        assert!(!pass);
        assert_eq!(findings.len(), 2);
        assert!(findings[0].starts_with("src/main.rs:12"));
    }

    #[test]
    fn unparseable_counts_as_fix() {
        let (pass, findings) = parse_verdict("I think this looks okay overall!");
        assert!(!pass);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn review_messages_contain_no_conversation() {
        let msgs = build_review_messages("fix the bug", "diff...", "(none)");
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[1]["role"], "user");
        let user = msgs[1]["content"].as_str().unwrap();
        assert!(user.contains("TASK:"));
        assert!(user.contains("diff..."));
    }

    #[test]
    fn command_status_reports_failure_and_timeout_guard() {
        let dir = tempdir().unwrap();
        let (ok, out) = run_command_status(dir.path(), "echo hi && exit 0", 5);
        assert!(ok);
        assert!(out.contains("hi"));
        let (ok2, _) = run_command_status(dir.path(), "exit 3", 5);
        assert!(!ok2);
    }

    #[test]
    fn captures_untracked_file_content() {
        let dir = tempdir().unwrap();
        // Init a real repo so `git status --porcelain` lists the untracked file.
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        fs::write(dir.path().join("new.txt"), "hello").unwrap();
        let (diff, files) = capture_diff(dir.path());
        assert!(diff.contains("new.txt"));
        assert!(diff.contains("hello"));
        assert!(files.contains(&"new.txt".to_string()));
    }

    #[test]
    fn capture_diff_degrades_outside_git() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("new.txt"), "hello").unwrap();
        let (diff, files) = capture_diff(dir.path());
        // No repo → git errors are swallowed; never panics.
        assert!(files.is_empty());
        assert!(!diff.contains("new.txt"));
    }
}
