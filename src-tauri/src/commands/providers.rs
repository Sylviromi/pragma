use crate::providers::types::{AgentEvent, ContentBlock};
use crate::tools::{definitions::standard_tools, ToolExecutor};
use crate::AppState;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::sync::Arc;
use tauri::Emitter;
use uuid::Uuid;

/// Cap a single tool result that gets persisted into history and fed back to the
/// model. Prevents one huge output (run_command, search_files, read_file …)
/// from blowing the context window for the whole conversation.
const MAX_TOOL_RESULT_CHARS: usize = 24_000;

/// Soft budget for the conversation history sent to the model on every
/// iteration. The history grows without bound over a long session (every tool
/// exchange is re-sent in full), so we trim the oldest complete tool exchanges
/// once the serialized size exceeds this. Always keeps the system prompt and
/// the newest user message.
const MAX_HISTORY_CHARS: usize = 100_000;

#[tauri::command]
pub async fn list_providers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::providers::types::ProviderInfo>, String> {
    Ok(state.registry.list_infos())
}

#[tauri::command]
pub async fn save_api_key(
    provider: String,
    key: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    {
        state.settings.write().unwrap().api_keys.insert(provider, key);
    }
    crate::save_settings(&state);
    Ok(())
}

#[tauri::command]
pub async fn get_api_keys(
    state: tauri::State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    Ok(state.settings.read().unwrap().api_keys.clone())
}

/// RAII guard releasing the agentic-loop lock on drop, so a panic mid-loop can
/// never leave a session permanently stuck (which would otherwise silently
/// block all future sends to that session).
struct RunningLoopGuard {
    session_id: String,
    running: Arc<tokio::sync::Mutex<HashSet<String>>>,
    db: SqlitePool,
    /// Token-checked delete: only remove the lock row we created, so a loop
    /// restarted on the same session isn't released by a stale cleanup task.
    started_at: String,
    released: bool,
}

impl RunningLoopGuard {
    fn new(
        session_id: String,
        running: Arc<tokio::sync::Mutex<HashSet<String>>>,
        db: SqlitePool,
        started_at: String,
    ) -> Self {
        Self {
            session_id,
            running,
            db,
            started_at,
            released: false,
        }
    }

    async fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        self.running.lock().await.remove(&self.session_id);
        let _ = sqlx::query("DELETE FROM active_loops WHERE session_id=? AND started_at=?")
            .bind(&self.session_id)
            .bind(&self.started_at)
            .execute(&self.db)
            .await;
    }
}

impl Drop for RunningLoopGuard {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        // Best-effort cleanup from a spawned task (Drop can't await). Only runs
        // when the normal `release()` path was never hit (panic, abort).
        let session_id = self.session_id.clone();
        let running = Arc::clone(&self.running);
        let db = self.db.clone();
        let started_at = self.started_at.clone();
        tokio::spawn(async move {
            running.lock().await.remove(&session_id);
            let _ = sqlx::query("DELETE FROM active_loops WHERE session_id=? AND started_at=?")
                .bind(&session_id)
                .bind(&started_at)
                .execute(&db)
                .await;
        });
    }
}

/// Returns the IDs of sessions that currently have an active agentic loop running.
/// Used by the frontend to reconnect after HMR restarts.
#[tauri::command]
pub async fn get_running_sessions(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(state.running_sessions.lock().await.iter().cloned().collect())
}

/// Emit a single agent event to the frontend via Tauri's global event bus.
/// Event name: `agent:{session_id}` — survives HMR page reloads.
fn emit(app: &tauri::AppHandle, session_id: &str, ev: AgentEvent) {
    app.emit(&format!("agent:{session_id}"), ev).ok();
}

/// The core agentic loop. Runs until the AI produces a final response with no tool_use blocks.
/// Events are emitted via `app_handle.emit()` (not a Channel) so they survive frontend HMR reloads.
#[tauri::command]
pub async fn run_agentic_loop(
    session_id: String,
    user_message: String,
    model: String,
    provider: String,
    working_dir: String,
    auto_approve: bool,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Clone the Arc before any borrows so we can remove the session after the call, even on error.
    let running = Arc::clone(&state.running_sessions);
    {
        let mut lock = running.lock().await;
        if lock.contains(&session_id) {
            return Err(format!("Session {session_id} already has an active agentic loop"));
        }
        lock.insert(session_id.clone());
    }

    // Cross-process lock: two app instances share sessions.db, so also record
    // the running loop in the DB. Concurrent loops on one session corrupt the
    // persisted history (tool results interleaving out of order), which is what
    // made the model re-call tools and lose the user's prompts.
    let started_at = chrono::Utc::now().to_rfc3339();
    let insert = sqlx::query(
        "INSERT INTO active_loops (session_id, started_at) VALUES (?, ?) ON CONFLICT(session_id) DO NOTHING",
    )
    .bind(&session_id)
    .bind(&started_at)
    .execute(&state.db)
    .await;
    let acquired = match insert {
        Ok(res) => res.rows_affected() == 1,
        Err(e) => {
            running.lock().await.remove(&session_id);
            return Err(format!("failed to acquire loop lock: {e}"));
        }
    };
    if !acquired {
        running.lock().await.remove(&session_id);
        return Err(format!("Session {session_id} already has an active agentic loop"));
    }

    let mut guard = RunningLoopGuard::new(session_id.clone(), running, state.db.clone(), started_at);
    let result = run_loop(&session_id, user_message, model, provider, working_dir, auto_approve, &app_handle, &state).await;
    // Release synchronously on the happy/error path (Drop covers panics).
    guard.release().await;
    result
}

async fn run_loop(
    session_id: &str,
    user_message: String,
    model: String,
    provider: String,
    working_dir: String,
    auto_approve: bool,
    app_handle: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
) -> Result<(), String> {
    let api_key = state
        .settings
        .read()
        .unwrap()
        .api_keys
        .get(&provider)
        .cloned()
        .unwrap_or_default();
    if provider == "anthropic" && api_key.is_empty() {
        return Err(
            "No Anthropic API key configured. Add it in Settings → Claude (Providers page)."
                .into(),
        );
    }
    let tools = standard_tools();
    let mut executor = ToolExecutor::new(if working_dir.is_empty() {
        ".".into()
    } else {
        working_dir
    });

    let provider_handle = state
        .registry
        .get(&provider)
        .ok_or_else(|| format!("unknown provider: {provider}"))?;

    // Base system prompt for the coding agent.
    const BASE_PROMPT: &str = "\
You are Pragma, an AI coding assistant running inside a desktop IDE. \
You have direct access to the user's file system and development environment through tools.\n\
\n\
## Tools available\n\
- **read_file** / **write_file** / **patch_file** — read and modify source files\n\
- **create_directory** / **delete_file** / **move_file** — file system operations\n\
- **list_files** / **search_files** — explore and search the codebase\n\
- **run_command** — execute shell commands (compile, test, run scripts, install packages)\n\
- **git_status** / **git_diff** / **git_log** / **git_add** / **git_commit** — git operations\n\
- **web_search** / **fetch_url** — look up documentation, research solutions\n\
- **change_directory** — change the session working directory\n\
- **load_skill** — load a plugin skill's full content (see Available skills section)\n\
\n\
## Rules\n\
- Do exactly what was asked — no extra features, no unsolicited refactors, no cleanup.\n\
- NEVER run git_add or git_commit unless the user explicitly asks you to commit or stage files.\n\
- If a tool call is denied by the user, stop and ask what they want instead. Do not retry.\n\
- Read files before editing — understand context first.\n\
- Use patch_file for targeted edits; write_file for new files.\n\
- patch_file needs an exact unified diff — accurate `@@ -start,count +start,count @@` line numbers and verbatim context lines (no fuzzy matching); read the file first to pin positions.\n\
- Run tests or the build after changes to verify correctness.";

    // Build skill index for system prompt — skills are loaded on demand via load_skill tool.
    let skill_index = state.plugins.skill_index();
    let system_content = if skill_index.is_empty() {
        BASE_PROMPT.to_string()
    } else {
        let index_lines: String = skill_index
            .iter()
            .map(|(id, desc)| format!("- **{id}**: {desc}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{BASE_PROMPT}\n\n## Available skills\nUse the `load_skill` tool to load a skill's full content when it seems relevant to the task.\n{index_lines}")
    };

    // Load existing conversation history from DB
    let db_messages = sqlx::query_as::<_, crate::commands::messages::Message>(
        "SELECT * FROM messages WHERE session_id=? ORDER BY timestamp ASC",
    )
    .bind(&session_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    // Convert to provider message format and repair the history before use.
    // Two failure modes corrupt the stored conversation and make the model
    // re-call the same tool while ignoring new prompts:
    //   1. An assistant `tool_use` whose result never arrived (loop interrupted
    //      mid-tool by HMR reload, app restart, or interleaved concurrent loop)
    //      — a dangling tool call. Sanitising here guarantees the provider
    //      never receives an unpaired tool_use, for both Anthropic (which
    //      hard-rejects it) and OpenAI-compat (whose sanitizer used to drop
    //      everything — including the user's new prompt — downstream of it).
    let raw_history: Vec<serde_json::Value> = db_messages
        .iter()
        .map(|m| {
            let content: serde_json::Value =
                serde_json::from_str(&m.content).unwrap_or(serde_json::Value::String(m.content.clone()));
            serde_json::json!({ "role": m.role, "content": content })
        })
        .collect();
    let mut history = sanitize_history(raw_history);

    // Append the new user message
    history.push(serde_json::json!({ "role": "user", "content": user_message }));

    history.insert(0, serde_json::json!({ "role": "system", "content": system_content }));

    // Keep the request inside the context budget before the first call. The
    // loop re-trims every iteration since history keeps growing.
    trim_history_to_budget(&mut history, MAX_HISTORY_CHARS);

    // Persist user message
    persist_message(
        &state.db,
        &session_id,
        "user",
        &serde_json::to_string(&user_message).unwrap(),
        &model,
        &provider,
    )
    .await?;

    // Agentic loop
    loop {
        // Fit the (ever-growing) history into the context window before each call.
        trim_history_to_budget(&mut history, MAX_HISTORY_CHARS);

        // Call the AI
        let blocks = provider_handle
            .call(&history, &model, &api_key, &tools, app_handle, session_id)
            .await?;

        // Find tool_use blocks
        let tool_calls: Vec<_> = blocks
            .iter()
            .filter_map(|b| {
                if let ContentBlock::ToolUse { id, name, input } = b {
                    Some((id.clone(), name.clone(), input.clone()))
                } else {
                    None
                }
            })
            .collect();

        // Persist assistant turn
        let blocks_json = serde_json::to_string(&blocks).unwrap();
        persist_message(
            &state.db,
            &session_id,
            "assistant",
            &blocks_json,
            &model,
            &provider,
        )
        .await?;
        history.push(serde_json::json!({
            "role": "assistant",
            "content": serde_json::from_str::<serde_json::Value>(&blocks_json).unwrap()
        }));

        if tool_calls.is_empty() {
            // No tool calls — done
            emit(app_handle, session_id, AgentEvent::Done);
            break;
        }

        // Handle tool calls
        let run_id = Uuid::new_v4().to_string();
        let mut tool_results: Vec<serde_json::Value> = Vec::new();
        let mut any_denied = false;

        for (tool_use_id, name, input) in &tool_calls {
            // Emit tool call event to frontend
            emit(app_handle, session_id, AgentEvent::ToolCall {
                run_id: run_id.clone(),
                tool_use_id: tool_use_id.clone(),
                name: name.clone(),
                input: input.clone(),
            });

            let approved = if auto_approve {
                true
            } else {
                // Wait for frontend to approve or deny
                let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
                state
                    .pending_approvals
                    .lock()
                    .await
                    .insert(tool_use_id.clone(), tx);
                rx.await.unwrap_or(false)
            };

            if !approved {
                any_denied = true;
            }

            let (mut output, is_error) = if !approved {
                ("Tool call denied by user.".to_string(), true)
            } else if name == "load_skill" {
                let skill_id = input.get("skill_id").and_then(|v| v.as_str()).unwrap_or("");
                if !state.plugins.is_enabled(skill_id) {
                    (format!("No enabled skill found with id: {skill_id}"), true)
                } else {
                    match state.plugins.skill_md(skill_id) {
                        Ok(md) => (md, false),
                        Err(e) => (format!("Failed to load skill '{skill_id}': {e}"), true),
                    }
                }
            } else if name == "change_directory" {
                let new_dir = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let abs = if std::path::Path::new(new_dir).is_absolute() {
                    std::path::PathBuf::from(new_dir)
                } else {
                    executor.working_dir.join(new_dir)
                };
                if !abs.is_dir() {
                    (format!("Directory does not exist: {}", abs.display()), true)
                } else {
                    let path_str = abs.canonicalize()
                        .unwrap_or(abs)
                        .to_string_lossy()
                        .to_string();
                    let now = chrono::Utc::now().to_rfc3339();
                    // Persist the model's preferred working directory
                    let _ = sqlx::query(
                        "UPDATE sessions SET working_dir=?,updated_at=? WHERE id=?"
                    )
                    .bind(&path_str)
                    .bind(&now)
                    .bind(&session_id)
                    .execute(&state.db)
                    .await;
                    executor = ToolExecutor::new(&path_str);
                    emit(app_handle, session_id, AgentEvent::WorkingDirChanged { new_path: path_str.clone() });
                    (format!("Working directory changed to: {path_str}"), false)
                }
            } else {
                match executor.execute(name, input) {
                    Ok(out) => (out, false),
                    Err(e) => (e, true),
                }
            };
            // Cap oversized outputs so one big result can't blow the context
            // window (and the persisted history) for the whole conversation.
            output = truncate_output(&output);

            emit(app_handle, session_id, AgentEvent::ToolResult {
                tool_use_id: tool_use_id.clone(),
                output: output.clone(),
                is_error,
            });

            tool_results.push(serde_json::json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "content": output,
                "is_error": is_error,
            }));
        }

        // Add tool results as user turn
        let results_content = serde_json::to_string(&tool_results).unwrap();
        persist_message(&state.db, &session_id, "user", &results_content, "", "").await?;
        history.push(serde_json::json!({ "role": "user", "content": tool_results }));

        // Stop the loop if any tool was denied — don't let the AI re-ask unprompted.
        if any_denied {
            emit(app_handle, session_id, AgentEvent::Done);
            break;
        }
    }

    Ok(())
}

/// Repair a raw DB history (list of `{role, content}`) into a well-formed
/// conversation for the provider:
/// - drop denied tool exchanges (assistant tool_use followed by all-denied results),
/// - drop dangling assistant `tool_use` messages that have no matching
///   `tool_result` turn (interrupted/interleaved runs),
/// - drop orphaned `tool_result` turns that don't follow a tool_use message.
///
/// Keeps every well-formed exchange pair so the provider always sees results
/// attached to their calls.
fn sanitize_history(raw: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let msg = &raw[i];
        let role = msg["role"].as_str().unwrap_or("");

        if role == "assistant" {
            let has_tool_use = msg["content"]
                .as_array()
                .map(|blocks| blocks.iter().any(|b| b["type"] == "tool_use"))
                .unwrap_or(false);
            if !has_tool_use {
                out.push(msg.clone());
                i += 1;
                continue;
            }
            // The immediately following turn must be the user tool_result for this call.
            let Some(next) = raw.get(i + 1) else {
                // dangling tool_use with no result at all — drop it
                i += 1;
                continue;
            };
            let results = next["content"]
                .as_array()
                .filter(|blocks| {
                    next["role"].as_str() == Some("user")
                        && blocks.iter().any(|b| b["type"] == "tool_result")
                });
            match results {
                None => {
                    // Something (a user prompt, another assistant turn, …) sits
                    // between this tool_use and its result → drop the dangling call.
                    i += 1;
                }
                Some(blocks) => {
                    let all_denied = blocks
                        .iter()
                        .filter(|b| b["type"] == "tool_result")
                        .all(|r| r["content"].as_str() == Some("Tool call denied by user."));
                    if all_denied {
                        // Don't feed the model a "tried → denied → tried again" trail.
                        i += 2;
                    } else {
                        out.push(msg.clone());
                        out.push(next.clone());
                        i += 2;
                    }
                }
            }
        } else if role == "user" {
            let is_tool_result = msg["content"]
                .as_array()
                .map(|blocks| blocks.iter().any(|b| b["type"] == "tool_result"))
                .unwrap_or(false);
            if is_tool_result {
                // Only keep a tool_result when the previous kept message is an
                // assistant turn with tool_use — i.e. it's the direct result of
                // a call that survived. Anything else is an orphaned leftover
                // from an interrupted/interleaved run.
                let matches_last_call = out
                    .last()
                    .map(|m| {
                        m["role"].as_str() == Some("assistant")
                            && m["content"]
                                .as_array()
                                .map(|b| b.iter().any(|x| x["type"] == "tool_use"))
                                .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if matches_last_call {
                    out.push(msg.clone());
                }
                i += 1;
            } else {
                out.push(msg.clone());
                i += 1;
            }
        } else {
            out.push(msg.clone());
            i += 1;
        }
    }
    out
}

/// Truncate a tool result so a single huge output can't blow the context window.
fn truncate_output(s: &str) -> String {
    if s.chars().count() <= MAX_TOOL_RESULT_CHARS {
        return s.to_string();
    }
    let cut: String = s.chars().take(MAX_TOOL_RESULT_CHARS).collect();
    format!("{cut}\n…[output truncated: {} chars]", s.chars().count())
}

/// Rough size of a history message (content length when plain text, serialized
/// length otherwise).
fn history_size(history: &[serde_json::Value]) -> usize {
    history
        .iter()
        .map(|m| match &m["content"] {
            serde_json::Value::String(s) => s.len(),
            _ => serde_json::to_string(m).map(|s| s.len()).unwrap_or(0),
        })
        .sum()
}

/// Trim the history to fit `budget` chars before it's sent to the provider.
/// Always keeps the first message (system prompt) and the last message (the
/// newest user turn — the model must always see what the user just asked).
/// Oldest complete tool exchanges are removed first, so pairing between
/// tool_use and tool_result is never broken.
fn trim_history_to_budget(history: &mut Vec<serde_json::Value>, budget: usize) {
    while history.len() > 2 && history_size(history) > budget {
        // Drop index 1 (never the system prompt at 0, never the last message).
        let msg = &history[1];
        let is_tool_call = msg["role"].as_str() == Some("assistant")
            && msg["content"]
                .as_array()
                .map(|b| b.iter().any(|x| x["type"] == "tool_use"))
                .unwrap_or(false);
        if is_tool_call && history[2]["role"].as_str() == Some("user") {
            // Remove the paired result too so the conversation stays well-formed.
            history.remove(2);
        }
        history.remove(1);
    }
}


async fn persist_message(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    role: &str,
    content: &str,
    model: &str,
    provider: &str,
) -> Result<(), String> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO messages (id,session_id,role,content,model,provider,timestamp,tokens) VALUES (?,?,?,?,?,?,?,0)")
        .bind(&id)
        .bind(session_id)
        .bind(role)
        .bind(content)
        .bind(model)
        .bind(provider)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_use(id: &str, name: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": { "path": "x" },
        })
    }

    fn tool_result(id: &str, content: &str, is_error: bool) -> serde_json::Value {
        serde_json::json!({
            "type": "tool_result",
            "tool_use_id": id,
            "content": content,
            "is_error": is_error,
        })
    }

    fn asst(blocks: Vec<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({ "role": "assistant", "content": blocks })
    }

    fn user_tools(blocks: Vec<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({ "role": "user", "content": blocks })
    }

    fn user_text(s: &str) -> serde_json::Value {
        serde_json::json!({ "role": "user", "content": s })
    }

    #[test]
    fn sanitize_keeps_well_formed_exchanges() {
        let raw = vec![
            user_text("hello"),
            asst(vec![tool_use("c1", "read_file")]),
            user_tools(vec![tool_result("c1", "file contents", false)]),
        ];
        let clean = sanitize_history(raw);
        assert_eq!(clean.len(), 3);
        assert_eq!(clean[0]["role"], "user");
        assert_eq!(clean[1]["role"], "assistant");
        assert_eq!(clean[2]["role"], "user");
    }

    #[test]
    fn sanitize_drops_dangling_tool_use() {
        // Interrupted run: assistant tool_use with no result at all.
        let raw = vec![
            user_text("hello"),
            asst(vec![tool_use("c1", "run_command")]),
        ];
        let clean = sanitize_history(raw);
        assert_eq!(clean.len(), 1);
        assert_eq!(clean[0]["role"], "user");
    }

    #[test]
    fn sanitize_drops_interleaved_exchanges() {
        // Concurrent loops wrote: assistant(tool_use) → assistant(text) → result.
        let raw = vec![
            user_text("hello"),
            asst(vec![tool_use("c1", "run_command")]),
            asst(vec![serde_json::json!({ "type": "text", "text": "Done." })]),
            user_tools(vec![tool_result("c1", "output", false)]),
            user_text("unrelated follow-up"),
        ];
        let clean = sanitize_history(raw);
        // dangling tool_use and the orphaned result are dropped; the final user
        // prompt survives
        assert!(!clean.iter().any(|m| {
            m["content"]
                .as_array()
                .map(|b| b.iter().any(|x| x["type"] == "tool_use"))
                .unwrap_or(false)
        }));
        assert!(!clean.iter().any(|m| {
            m["content"]
                .as_array()
                .map(|b| b.iter().any(|x| x["type"] == "tool_result"))
                .unwrap_or(false)
        }));
        assert!(clean.iter().any(|m| m["content"] == "unrelated follow-up"));
        assert_eq!(clean.iter().filter(|m| m["role"] == "user").count(), 2);
    }

    #[test]
    fn sanitize_drops_denied_exchanges() {
        let raw = vec![
            user_text("hello"),
            asst(vec![tool_use("c1", "run_command")]),
            user_tools(vec![tool_result("c1", "Tool call denied by user.", true)]),
            user_text("what now"),
        ];
        let clean = sanitize_history(raw);
        assert_eq!(clean.len(), 2);
        assert_eq!(clean[0]["content"], "hello");
        assert_eq!(clean[1]["content"], "what now");
    }

    #[test]
    fn sanitize_drops_orphaned_tool_results() {
        let raw = vec![
            user_text("hello"),
            user_tools(vec![tool_result("c1", "orphan", false)]),
        ];
        let clean = sanitize_history(raw);
        assert_eq!(clean.len(), 1);
        assert_eq!(clean[0]["role"], "user");
        assert_eq!(clean[0]["content"], "hello");
    }

    #[test]
    fn truncate_output_caps_long_results() {
        let long = "x".repeat(MAX_TOOL_RESULT_CHARS + 100);
        let capped = truncate_output(&long);
        assert!(capped.len() < long.len());
        assert!(capped.contains("truncated"));
        // Short outputs pass through untouched
        assert_eq!(truncate_output("short"), "short");
    }

    #[test]
    fn trim_keeps_system_and_newest_user() {
        let mut history: Vec<serde_json::Value> = vec![
            serde_json::json!({ "role": "system", "content": "sys" }),
            user_text(&"a".repeat(400)),
            asst(vec![tool_use("c1", "read_file")]),
            user_tools(vec![tool_result("c1", &"b".repeat(400), false)]),
            user_text(&"c".repeat(400)),
        ];
        trim_history_to_budget(&mut history, 600);
        // system kept, oldest exchange (user "aaa…") dropped, newest user kept
        assert_eq!(history[0]["role"], "system");
        assert!(history.iter().any(|m| m["role"] == "user" && m["content"].as_str().unwrap_or("").starts_with('c')));
        assert!(history.len() <= 4);
    }

    #[test]
    fn trim_removes_entire_old_exchange() {
        let mut history: Vec<serde_json::Value> = vec![
            serde_json::json!({ "role": "system", "content": "sys" }),
            asst(vec![tool_use("c1", "run_command")]),
            user_tools(vec![tool_result("c1", "out", false)]),
            user_text("newest prompt"),
        ];
        trim_history_to_budget(&mut history, 10);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0]["role"], "system");
        assert_eq!(history[1]["content"], "newest prompt");
    }
}
