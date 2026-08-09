use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

/// Output cap for a single command result (defense-in-depth: run_loop also
/// caps every tool result before persisting it).
const MAX_OUTPUT_CHARS: usize = 96_000;

/// Default hard timeout, matching the tool description — a server that never
/// exits (e.g. `npm run dev`) must not hold the agentic loop forever.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

pub fn run_command(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let command = input["command"].as_str().ok_or("missing command")?;

    let timeout = match input["timeout_secs"].as_u64() {
        Some(secs) if secs > 0 => Duration::from_secs(secs),
        Some(_) => return Err("timeout_secs must be a positive integer".into()),
        None => DEFAULT_TIMEOUT,
    };
    let max_output_chars = match input["max_output_chars"].as_u64() {
        Some(n) if n > 0 => n as usize,
        Some(_) => return Err("max_output_chars must be a positive integer".into()),
        None => MAX_OUTPUT_CHARS,
    };

    let child = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(wd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let output = run_with_timeout(child, timeout)
        .map_err(|e| format!("command failed: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    let result = if combined.trim().is_empty() {
        "(no output)".to_string()
    } else {
        combined
    };
    // Cap runaway output so huge results can't dominate context.
    if result.chars().count() > max_output_chars {
        let cut: String = result.chars().take(max_output_chars).collect();
        return Ok(format!(
            "{cut}\n…[output truncated: {} chars]",
            result.chars().count()
        ));
    }
    Ok(result)
}

/// Wait for the child with a deadline, draining stdout/stderr on the side.
/// Kills the child on timeout.
fn run_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<std::process::Output, String> {
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
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let (out, err) = reader.join().map_err(|_| "output reader failed".to_string())?;
                return Ok(std::process::Output {
                    status,
                    stdout: out,
                    stderr: err,
                });
            }
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("process error: {e}"));
            }
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("timed out after {}s", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_echo() {
        let dir = tempdir().unwrap();
        let r = run_command(dir.path(), &serde_json::json!({"command":"echo hello"})).unwrap();
        assert!(r.contains("hello"));
    }

    #[test]
    fn test_stderr_captured() {
        let dir = tempdir().unwrap();
        let r = run_command(dir.path(), &serde_json::json!({"command":"echo err >&2"})).unwrap();
        assert!(r.contains("err"));
    }

    #[test]
    fn test_timeout_kills_hung_command() {
        let r = run_with_timeout(
            std::process::Command::new("sh")
                .arg("-c")
                .arg("sleep 5")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
            Duration::from_millis(200),
        );
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("timed out"));
    }

    #[test]
    fn test_custom_timeout_secs() {
        let dir = tempdir().unwrap();
        let r = run_command(
            dir.path(),
            &serde_json::json!({"command":"sleep 5", "timeout_secs": 1}),
        );
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("timed out after 1s"));
    }

    #[test]
    fn test_output_capped() {
        let dir = tempdir().unwrap();
        let r = run_command(dir.path(), &serde_json::json!({"command":"yes x | head -c 200000"})).unwrap();
        assert!(r.chars().count() <= MAX_OUTPUT_CHARS + 100);
        assert!(r.contains("truncated"));
    }

    #[test]
    fn test_custom_output_cap() {
        let dir = tempdir().unwrap();
        let r = run_command(
            dir.path(),
            &serde_json::json!({"command":"printf 'abcdefghij'", "max_output_chars": 5}),
        )
        .unwrap();
        assert_eq!(r, "abcde\n…[output truncated: 10 chars]");
    }
}