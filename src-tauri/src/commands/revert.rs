use crate::AppState;
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

/// File tools whose effects can be rolled back via the snapshot journal.
pub const MUTATING_FILE_TOOLS: [&str; 3] = ["write_file", "edit_file", "delete_file"];

pub fn is_mutating_file_tool(name: &str) -> bool {
    MUTATING_FILE_TOOLS.contains(&name)
}

/// A minimal view of a message row with its rowid (stable insertion order).
#[derive(sqlx::FromRow)]
struct RowMsg {
    id: String,
    role: String,
    content: String,
    #[allow(dead_code)]
    _rowid: i64,
}

#[derive(sqlx::FromRow)]
struct SnapshotRow {
    tool_name: String,
    path: String,
    content_before: String,
    existed_before: i64,
    extra_before: String,
}

/// Before the agentic loop executes a mutating file tool, journal the pre-op
/// state of every affected file. Reverting replays these in reverse, restoring
/// the filesystem to the checkpoint moment.
pub async fn record_snapshot(
    pool: &SqlitePool,
    session_id: &str,
    message_rowid: i64,
    tool_use_id: &str,
    tool_name: &str,
    input: &serde_json::Value,
    wd: &Path,
) -> Result<(), String> {
    let read_opt = |rel: &str| -> (String, bool) {
        match std::fs::read_to_string(wd.join(rel)) {
            Ok(c) => (c, true),
            Err(_) => (String::new(), false),
        }
    };

    let (path, content, existed, extra) = match tool_name {
        "write_file" | "edit_file" | "delete_file" => {
            let rel = input["path"].as_str().unwrap_or("");
            let (c, e) = read_opt(rel);
            (wd.join(rel).display().to_string(), c, e, serde_json::json!({}))
        }
        _ => return Ok(()),
    };

    sqlx::query(
        "INSERT INTO file_snapshots (session_id, message_rowid, tool_use_id, tool_name, path, content_before, existed_before, extra_before) \
         VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(session_id)
    .bind(message_rowid)
    .bind(tool_use_id)
    .bind(tool_name)
    .bind(&path)
    .bind(&content)
    .bind(existed as i64)
    .bind(extra.to_string())
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// How many leading messages survive a revert to `to_message_id`: the target
/// itself plus everything before it, minus any trailing *incomplete* exchange
/// (an assistant tool_use whose result was cut off, or an orphaned tool_result)
/// so the conversation always ends on a complete turn.
fn kept_count(rows: &[RowMsg], to_message_id: &str) -> Option<usize> {
    let idx = rows.iter().position(|r| r.id == to_message_id)?;
    let mut keep = idx + 1;
    while keep > 0 {
        let last = &rows[keep - 1];
        let content: serde_json::Value = serde_json::from_str(&last.content)
            .unwrap_or_else(|_| serde_json::Value::String(last.content.clone()));
        let is_tool_use = last.role == "assistant"
            && content
                .as_array()
                .map(|b| b.iter().any(|x| x["type"] == "tool_use"))
                .unwrap_or(false);
        let is_tool_result = last.role == "user"
            && content
                .as_array()
                .map(|b| b.iter().any(|x| x["type"] == "tool_result"))
                .unwrap_or(false);
        if is_tool_use {
            // An assistant tool_use with nothing after it is an unfinished
            // exchange — cut it so the conversation ends on a complete turn.
            keep -= 1;
            continue;
        }
        if is_tool_result {
            // Complete only when the previous kept message is its tool_use
            // (an orphaned result comes from an interrupted/interleaved run).
            let paired = keep >= 2 && {
                let prev = &rows[keep - 2];
                let pc: serde_json::Value = serde_json::from_str(&prev.content)
                    .unwrap_or_else(|_| serde_json::Value::String(prev.content.clone()));
                prev.role == "assistant"
                    && pc.as_array()
                        .map(|b| b.iter().any(|x| x["type"] == "tool_use"))
                        .unwrap_or(false)
            };
            if paired {
                break;
            } else {
                keep -= 1;
                continue;
            }
        }
        break;
    }
    Some(keep)
}

fn write_with_dirs(p: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(p, content).map_err(|e| format!("write {}: {e}", p.display()))
}

/// Restore the pre-op state recorded by one snapshot. Returns the paths touched.
fn restore_snapshot(snap: &SnapshotRow) -> Result<Vec<String>, String> {
    let mut touched = Vec::new();
    match snap.tool_name.as_str() {
        "write_file" | "edit_file" | "delete_file" => {
            let p = PathBuf::from(&snap.path);
            if snap.existed_before == 1 {
                write_with_dirs(&p, &snap.content_before)?;
            } else {
                let _ = std::fs::remove_file(&p);
            }
            touched.push(snap.path.clone());
        }
        _ => {}
    }
    Ok(touched)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertSummary {
    pub messages_removed: usize,
    pub files_reverted: Vec<String>,
}

/// Roll the conversation (and optionally the filesystem) back to a message.
/// Everything after the target message is deleted; file changes made by tool
/// calls after that point are restored to their pre-call state when
/// `revert_files` is set.
#[tauri::command]
pub async fn revert_session(
    session_id: String,
    to_message_id: String,
    revert_files: bool,
    state: tauri::State<'_, AppState>,
) -> Result<RevertSummary, String> {
    // Never rewrite history under a live loop — it would re-persist deleted rows.
    if state.running_sessions.lock().await.contains(&session_id) {
        return Err("Session is still running — press Esc to stop it first.".into());
    }
    let active: Option<(String,)> = sqlx::query_as("SELECT session_id FROM active_loops WHERE session_id=?")
        .bind(&session_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    if active.is_some() {
        // Only a *live* loop blocks a revert — a stale row from a crashed
        // process is cleared so the rollback can proceed.
        let stale = crate::commands::agentic::loop_row_is_stale(&state.db, &session_id)
            .await
            .unwrap_or(false);
        if !stale {
            return Err("Session is still running — press Esc to stop it first.".into());
        }
        let _ = sqlx::query("DELETE FROM active_loops WHERE session_id=?")
            .bind(&session_id)
            .execute(&state.db)
            .await;
    }

    let rows: Vec<RowMsg> = sqlx::query_as(
        "SELECT id, role, content, rowid AS _rowid FROM messages WHERE session_id=? ORDER BY rowid ASC",
    )
    .bind(&session_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let keep = kept_count(&rows, &to_message_id).ok_or("message not found in this session")?;
    let messages_removed = rows.len().saturating_sub(keep);
    // Last kept message's rowid — everything strictly after it is dropped.
    let cutoff_rowid = if keep > 0 { rows[keep - 1]._rowid } else { 0 };

    let mut files_reverted: Vec<String> = Vec::new();
    if revert_files {
        let snaps: Vec<SnapshotRow> = sqlx::query_as(
            "SELECT tool_name, path, content_before, existed_before, extra_before \
             FROM file_snapshots WHERE session_id=? AND message_rowid > ? ORDER BY id ASC",
        )
        .bind(&session_id)
        .bind(cutoff_rowid)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        for snap in snaps.iter().rev() {
            match restore_snapshot(snap) {
                Ok(paths) => files_reverted.extend(paths),
                Err(e) => eprintln!("[revert] restore failed for {}: {e}", snap.path),
            }
        }
    }

    if cutoff_rowid > 0 {
        sqlx::query("DELETE FROM messages WHERE session_id=? AND rowid > ?")
            .bind(&session_id)
            .bind(cutoff_rowid)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
    } else {
        sqlx::query("DELETE FROM messages WHERE session_id=?")
            .bind(&session_id)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
    }
    sqlx::query("DELETE FROM file_snapshots WHERE session_id=? AND message_rowid > ?")
        .bind(&session_id)
        .bind(cutoff_rowid)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    Ok(RevertSummary {
        messages_removed,
        files_reverted,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertInfo {
    pub files: Vec<String>,
    pub messages_removed: usize,
    pub preview: String,
}

/// Preview for the revert dialog: how many messages will be removed and which
/// files were touched by tool calls in the span being discarded.
#[tauri::command]
pub async fn get_revert_info(
    session_id: String,
    to_message_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<RevertInfo, String> {
    let rows: Vec<RowMsg> = sqlx::query_as(
        "SELECT id, role, content, rowid AS _rowid FROM messages WHERE session_id=? ORDER BY rowid ASC",
    )
    .bind(&session_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let idx = rows
        .iter()
        .position(|r| r.id == to_message_id)
        .ok_or("message not found in this session")?;
    let keep = kept_count(&rows, &to_message_id).unwrap_or(idx + 1);

    let mut files: Vec<String> = Vec::new();
    for r in rows.iter().skip(keep) {
        if r.role != "assistant" {
            continue;
        }
        let content: serde_json::Value = serde_json::from_str(&r.content)
            .unwrap_or_else(|_| serde_json::Value::String(r.content.clone()));
        let Some(blocks) = content.as_array() else { continue };
        for b in blocks {
            if b["type"] != "tool_use" {
                continue;
            }
            let name = b["name"].as_str().unwrap_or("");
            if !is_mutating_file_tool(name) {
                continue;
            }
            let path = b["input"]["path"]
                .as_str()
                .or_else(|| b["input"]["src"].as_str())
                .unwrap_or("");
            if !path.is_empty() && !files.contains(&path.to_string()) {
                files.push(path.to_string());
            }
        }
    }

    Ok(RevertInfo {
        files,
        messages_removed: rows.len().saturating_sub(keep),
        preview: preview_text(&rows[idx]),
    })
}

fn preview_text(r: &RowMsg) -> String {
    let content: serde_json::Value = serde_json::from_str(&r.content)
        .unwrap_or_else(|_| serde_json::Value::String(r.content.clone()));
    let text = match content {
        serde_json::Value::String(s) => s,
        v => v
            .as_array()
            .and_then(|blocks| {
                blocks
                    .iter()
                    .find(|b| b["type"] == "text")
                    .and_then(|b| b["text"].as_str().map(|s| s.to_string()))
            })
            .unwrap_or_else(|| v.to_string()),
    };
    let trimmed: String = text.chars().take(160).collect();
    if text.chars().count() > 160 {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, role: &str, content: &str) -> RowMsg {
        RowMsg {
            id: id.into(),
            role: role.into(),
            content: content.into(),
            _rowid: 0,
        }
    }

    fn tool_use_blocks() -> String {
        serde_json::json!([{ "type": "tool_use", "id": "c1", "name": "write_file", "input": { "path": "a.txt", "content": "x" } }])
            .to_string()
    }

    fn tool_result_blocks() -> String {
        serde_json::json!([{ "type": "tool_result", "tool_use_id": "c1", "content": "ok", "is_error": false }])
            .to_string()
    }

    fn text_msg(id: &str, text: &str) -> RowMsg {
        row(id, "user", &serde_json::to_string(text).unwrap())
    }

    #[test]
    fn kept_count_keeps_buffer_and_target() {
        let rows = vec![
            text_msg("u1", "hello"),
            row("a1", "assistant", &serde_json::to_string("hi!").unwrap()),
            text_msg("u2", "do it"),
        ];
        // revert to a1: keep u1 + a1; u2 goes away
        let keep = kept_count(&rows, "a1").unwrap();
        assert_eq!(keep, 2);
    }

    #[test]
    fn kept_count_drops_trailing_incomplete_tool_exchange() {
        let rows = vec![
            text_msg("u1", "hello"),
            row("a1", "assistant", &tool_use_blocks()),
            row("r1", "user", &tool_result_blocks()),
            text_msg("u2", "next"),
        ];
        // Revert to the tool_use message itself: the pending exchange (tool_use
        // + result) is incomplete at that point, so both are dropped.
        let keep = kept_count(&rows, "a1").unwrap();
        assert_eq!(keep, 1);
        // Reverting to the result keeps the whole exchange.
        let keep = kept_count(&rows, "r1").unwrap();
        assert_eq!(keep, 3);
    }

    #[test]
    fn kept_count_trailing_text_survives() {
        let rows = vec![
            text_msg("u1", "hello"),
            row("a1", "assistant", &serde_json::to_string("final answer").unwrap()),
        ];
        assert_eq!(kept_count(&rows, "a1").unwrap(), 2);
        assert_eq!(kept_count(&rows, "u1").unwrap(), 1);
    }

    #[test]
    fn is_mutating_tool_whitelist() {
        assert!(is_mutating_file_tool("write_file"));
        assert!(is_mutating_file_tool("edit_file"));
        assert!(is_mutating_file_tool("delete_file"));
        assert!(!is_mutating_file_tool("read_file"));
        assert!(!is_mutating_file_tool("run_command"));
    }

    #[test]
    fn restore_rewrites_deleted_and_creates_new() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.txt");
        std::fs::write(&p, "before").unwrap();
        std::fs::remove_file(&p).unwrap();

        // delete_file snapshot: file existed with "before"
        let snap = SnapshotRow {
            tool_name: "delete_file".into(),
            path: p.display().to_string(),
            content_before: "before".into(),
            existed_before: 1,
            extra_before: "{}".into(),
        };
        restore_snapshot(&snap).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "before");

        // write_file on a file that didn't exist → restore deletes it
        let p2 = dir.path().join("new.txt");
        std::fs::write(&p2, "created").unwrap();
        let snap = SnapshotRow {
            tool_name: "write_file".into(),
            path: p2.display().to_string(),
            content_before: String::new(),
            existed_before: 0,
            extra_before: "{}".into(),
        };
        restore_snapshot(&snap).unwrap();
        assert!(!p2.exists());
    }

}
