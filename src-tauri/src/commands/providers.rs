use crate::providers::types::{AgentEvent, ContentBlock};
use crate::tools::{definitions::standard_tools, ToolExecutor};
use crate::AppState;
use tauri::Emitter;
use uuid::Uuid;

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

/// Returns the IDs of sessions that currently have an active agentic loop running.
/// Used by the frontend to reconnect after HMR reloads.
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
    // Clone the Arc before any borrows so we can remove the session after the loop, even on error.
    let running = std::sync::Arc::clone(&state.running_sessions);
    running.lock().await.insert(session_id.clone());
    let result = run_loop(&session_id, user_message, model, provider, working_dir, auto_approve, &app_handle, &state).await;
    running.lock().await.remove(&session_id);
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

    // Convert to provider message format, filtering out denied tool exchanges.
    // A denied exchange is: assistant message containing tool_use blocks whose results
    // include "Tool call denied by user." — remove both the assistant and user turns.
    let raw_history: Vec<serde_json::Value> = db_messages
        .iter()
        .map(|m| {
            let content: serde_json::Value =
                serde_json::from_str(&m.content).unwrap_or(serde_json::Value::String(m.content.clone()));
            serde_json::json!({ "role": m.role, "content": content })
        })
        .collect();

    // Remove assistant+user pairs where all tool results are denials.
    // This prevents the model from seeing a trail of "tried → denied → tried again".
    let mut clean: Vec<serde_json::Value> = Vec::new();
    let mut i = 0;
    while i < raw_history.len() {
        let msg = &raw_history[i];
        if msg["role"].as_str() == Some("assistant") {
            if let Some(blocks) = msg["content"].as_array() {
                let has_tool_use = blocks.iter().any(|b| b["type"] == "tool_use");
                if has_tool_use {
                    // Check next message for denial results
                    if let Some(next) = raw_history.get(i + 1) {
                        if next["role"].as_str() == Some("user") {
                            if let Some(results) = next["content"].as_array() {
                                let all_denied = results.iter().all(|r| {
                                    r["type"] == "tool_result"
                                        && r["content"].as_str() == Some("Tool call denied by user.")
                                });
                                if all_denied {
                                    i += 2; // skip both turns
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
        }
        clean.push(msg.clone());
        i += 1;
    }
    let mut history = clean;

    // Append the new user message
    history.push(serde_json::json!({ "role": "user", "content": user_message }));

    history.insert(0, serde_json::json!({ "role": "system", "content": system_content }));

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

            let (output, is_error) = if !approved {
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
                    // Persist the model's preferred provider/model fields unchanged
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