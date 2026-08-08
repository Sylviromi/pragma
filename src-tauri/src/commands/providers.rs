use crate::providers::types::{AgentEvent, ContentBlock};
use crate::tools::{definitions::standard_tools, ToolExecutor};
use crate::AppState;
use tauri::ipc::Channel;
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
    let settings = state.settings.read().unwrap();
    let json = serde_json::json!({ "api_keys": settings.api_keys });
    std::fs::write(
        &state.settings_path,
        serde_json::to_string_pretty(&json).unwrap(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_api_keys(
    state: tauri::State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    Ok(state.settings.read().unwrap().api_keys.clone())
}

/// The core agentic loop. Runs until the AI produces a final response with no tool_use blocks.
/// Emits AgentEvent::ToolCall for each tool — waits for approve/deny from frontend before executing.
#[tauri::command]
pub async fn run_agentic_loop(
    session_id: String,
    user_message: String,
    model: String,
    provider: String,
    working_dir: String,
    auto_approve: bool,
    channel: Channel<AgentEvent>,
    state: tauri::State<'_, AppState>,
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
    let executor = ToolExecutor::new(if working_dir.is_empty() {
        ".".into()
    } else {
        working_dir
    });

    let provider_handle = state
        .registry
        .get(&provider)
        .ok_or_else(|| format!("unknown provider: {provider}"))?;

    // Load existing conversation history from DB
    let db_messages = sqlx::query_as::<_, crate::commands::messages::Message>(
        "SELECT * FROM messages WHERE session_id=? ORDER BY timestamp ASC",
    )
    .bind(&session_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    // Convert to provider message format
    let mut history: Vec<serde_json::Value> = db_messages
        .iter()
        .map(|m| {
            let content: serde_json::Value =
                serde_json::from_str(&m.content).unwrap_or(serde_json::Value::String(m.content.clone()));
            serde_json::json!({ "role": m.role, "content": content })
        })
        .collect();

    // Append the new user message
    history.push(serde_json::json!({ "role": "user", "content": user_message }));

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
            .call(&history, &model, &api_key, &tools, &channel)
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
            channel.send(AgentEvent::Done).ok();
            break;
        }

        // Handle tool calls
        let run_id = Uuid::new_v4().to_string();
        let mut tool_results: Vec<serde_json::Value> = Vec::new();

        for (tool_use_id, name, input) in &tool_calls {
            // Emit tool call event to frontend
            channel
                .send(AgentEvent::ToolCall {
                    run_id: run_id.clone(),
                    tool_use_id: tool_use_id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                })
                .ok();

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

            let (output, is_error) = if approved {
                match executor.execute(name, input) {
                    Ok(out) => (out, false),
                    Err(e) => (e, true),
                }
            } else {
                ("Tool call denied by user.".to_string(), false)
            };

            channel
                .send(AgentEvent::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    output: output.clone(),
                    is_error,
                })
                .ok();

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