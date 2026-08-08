use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn abs(working_dir: &Path, rel: &str) -> PathBuf {
    working_dir.join(rel)
}

pub fn read_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    let full = abs(wd, path);
    std::fs::read_to_string(&full).map_err(|e| format!("{e}: {}", full.display()))
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

pub fn patch_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    let diff = input["diff"].as_str().ok_or("missing diff")?;
    let full = abs(wd, path);
    let original = std::fs::read_to_string(&full).map_err(|e| format!("read failed: {e}"))?;
    let patched = diffy::apply(&original, &diffy::Patch::from_str(diff).map_err(|e| format!("invalid diff: {e}"))?)
        .map_err(|e| format!("patch failed: {e}"))?;
    std::fs::write(&full, &patched).map_err(|e| e.to_string())?;
    Ok(format!("patched {path}"))
}

pub fn create_directory(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    std::fs::create_dir_all(abs(wd, path)).map_err(|e| e.to_string())?;
    Ok(format!("created {path}"))
}

pub fn delete_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    let full = abs(wd, path);
    std::fs::remove_file(&full).map_err(|e| format!("{e}: {}", full.display()))?;
    Ok(format!("deleted {path}"))
}

pub fn move_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let src = input["src"].as_str().ok_or("missing src")?;
    let dest = input["dest"].as_str().ok_or("missing dest")?;
    let dest_full = abs(wd, dest);
    if let Some(parent) = dest_full.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(abs(wd, src), &dest_full).map_err(|e| e.to_string())?;
    Ok(format!("moved {src} → {dest}"))
}

pub fn list_files(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let subdir = input["path"].as_str().unwrap_or(".");
    let pattern = input["pattern"].as_str();
    let base = abs(wd, subdir);
    let mut entries = Vec::new();
    for entry in WalkDir::new(&base).max_depth(6) {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(wd)
                .unwrap_or(entry.path())
                .display()
                .to_string();
            if let Some(pat) = pattern {
                if glob::Pattern::new(pat)
                    .map(|p| p.matches(&rel))
                    .unwrap_or(false)
                {
                    entries.push(rel);
                }
            } else {
                entries.push(rel);
            }
        }
    }
    Ok(if entries.is_empty() {
        "(no files)".into()
    } else {
        entries.join("\n")
    })
}

pub fn search_files(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let pattern = input["pattern"].as_str().ok_or("missing pattern")?;
    let subdir = input["path"].as_str().unwrap_or(".");
    let file_glob = input["glob"].as_str();
    let base = abs(wd, subdir);
    let re = regex::Regex::new(pattern).map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for entry in WalkDir::new(&base) {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(wd)
            .unwrap_or(entry.path())
            .display()
            .to_string();
        if let Some(g) = file_glob {
            if !glob::Pattern::new(g)
                .map(|p| p.matches(&rel))
                .unwrap_or(false)
            {
                continue;
            }
        }
        if let Ok(text) = std::fs::read_to_string(entry.path()) {
            for (i, line) in text.lines().enumerate() {
                if re.is_match(line) {
                    results.push(format!("{rel}:{}: {line}", i + 1));
                }
            }
        }
    }
    Ok(if results.is_empty() {
        "(no matches)".into()
    } else {
        results.join("\n")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_write_then_read() {
        let dir = tempdir().unwrap();
        write_file(dir.path(), &serde_json::json!({"path":"hello.txt","content":"hello world"})).unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"hello.txt"})).unwrap();
        assert_eq!(r, "hello world");
    }

    #[test]
    fn test_patch_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -2,1 +2,1 @@\n-line2\n+LINE2\n";
        patch_file(dir.path(), &serde_json::json!({"path":"f.txt","diff":diff})).unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"f.txt"})).unwrap();
        assert!(r.contains("LINE2"), "expected LINE2 in: {r}");
    }

    #[test]
    fn test_create_and_delete() {
        let dir = tempdir().unwrap();
        create_directory(dir.path(), &serde_json::json!({"path":"sub/nested"})).unwrap();
        assert!(dir.path().join("sub/nested").exists());
        write_file(dir.path(), &serde_json::json!({"path":"sub/nested/x.txt","content":"x"})).unwrap();
        delete_file(dir.path(), &serde_json::json!({"path":"sub/nested/x.txt"})).unwrap();
        assert!(!dir.path().join("sub/nested/x.txt").exists());
    }

    #[test]
    fn test_move_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "data").unwrap();
        move_file(dir.path(), &serde_json::json!({"src":"a.txt","dest":"b.txt"})).unwrap();
        assert!(!dir.path().join("a.txt").exists());
        assert!(dir.path().join("b.txt").exists());
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
        fs::write(dir.path().join("main.rs"), "fn main() { println!(\"hello\"); }").unwrap();
        let r = search_files(dir.path(), &serde_json::json!({"pattern":"println"})).unwrap();
        assert!(r.contains("main.rs"));
    }
}