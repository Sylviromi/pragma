use std::path::{Path, PathBuf};
use std::process::Command;

fn repo(wd: &Path, input: &serde_json::Value) -> PathBuf {
    input["repo_path"]
        .as_str()
        .map(|p| {
            if p.is_empty() {
                wd.to_path_buf()
            } else {
                PathBuf::from(p)
            }
        })
        .unwrap_or_else(|| wd.to_path_buf())
}

fn run_git(args: &[&str], cwd: &Path) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
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

pub fn git_status(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    run_git(&["status", "--short"], &repo(wd, input))
}

pub fn git_diff(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let r = repo(wd, input);
    let staged = input["staged"].as_bool().unwrap_or(false);
    let path_arg = input["path"].as_str();
    let mut args = vec!["diff"];
    if staged {
        args.push("--cached");
    }
    if let Some(p) = path_arg {
        args.push("--");
        args.push(p);
    }
    run_git(&args, &r)
}

pub fn git_log(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let limit = input["limit"].as_u64().unwrap_or(10);
    let n = format!("-{limit}");
    run_git(&["log", "--oneline", &n], &repo(wd, input))
}

pub fn git_add(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let r = repo(wd, input);
    let paths: Vec<&str> = input["paths"]
        .as_array()
        .ok_or("missing paths")?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    if paths.is_empty() {
        return Err("paths array is empty".into());
    }
    let mut args = vec!["add", "--"];
    args.extend(paths.iter().copied());
    run_git(&args, &r)
}

pub fn git_commit(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let message = input["message"].as_str().ok_or("missing message")?;
    run_git(&["commit", "-m", message], &repo(wd, input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn init_repo(dir: &Path) {
        Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()
            .unwrap();
    }

    #[test]
    fn test_git_status_untracked() {
        let dir = tempdir().unwrap();
        init_repo(dir.path());
        fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let r = git_status(dir.path(), &serde_json::json!({})).unwrap();
        assert!(r.contains("a.txt"));
    }

    #[test]
    fn test_git_add_and_commit() {
        let dir = tempdir().unwrap();
        init_repo(dir.path());
        fs::write(dir.path().join("a.txt"), "hi").unwrap();
        git_add(dir.path(), &serde_json::json!({"paths":["a.txt"]})).unwrap();
        let r = git_commit(dir.path(), &serde_json::json!({"message":"initial"})).unwrap();
        assert!(r.contains("initial") || r.contains("master") || r.contains("main"));
    }

    #[test]
    fn test_git_log() {
        let dir = tempdir().unwrap();
        init_repo(dir.path());
        fs::write(dir.path().join("a.txt"), "hi").unwrap();
        git_add(dir.path(), &serde_json::json!({"paths":["a.txt"]})).unwrap();
        git_commit(dir.path(), &serde_json::json!({"message":"first commit"})).unwrap();
        let r = git_log(dir.path(), &serde_json::json!({"limit":5})).unwrap();
        assert!(r.contains("first commit"));
    }
}
