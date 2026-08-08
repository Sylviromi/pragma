use std::path::Path;

pub fn run_command(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let command = input["command"].as_str().ok_or("missing command")?;
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(wd)
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    Ok(if combined.trim().is_empty() {
        "(no output)".into()
    } else {
        combined
    })
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
}