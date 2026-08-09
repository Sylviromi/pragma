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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    {
        state.settings.write().unwrap().api_keys.insert(provider.clone(), key);
    }
    crate::save_settings(&state);
    // Fetch fresh model lists for this provider now that the key changed.
    state.registry.trigger_discovery(&provider, &app);
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

/// Is an `active_loops` row for this session stale? A live loop refreshes its
/// heartbeat every ~10s, so anything with a heartbeat older than two minutes
/// (or — for rows written before the heartbeat column existed — a start older
/// than ten minutes) belongs to a dead process and can be reclaimed.
pub(crate) async fn loop_row_is_stale(pool: &sqlx::SqlitePool, session_id: &str) -> Result<bool, String> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT started_at, heartbeat_at FROM active_loops WHERE session_id=?",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some((started_at, heartbeat_at)) = row else {
        return Ok(true); // no row at all — nothing to keep
    };
    let now = chrono::Utc::now();
    let age_secs = |s: &str| -> i64 {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| (now - d.with_timezone(&chrono::Utc)).num_seconds())
            .unwrap_or(i64::MAX)
    };
    if !heartbeat_at.trim().is_empty() {
        return Ok(age_secs(&heartbeat_at) > 120);
    }
    Ok(age_secs(&started_at) > 600)
}

/// Cancel the active agentic loop for a session (Esc key / Stop button).
/// Idempotent: if nothing is running the session is untouched.
#[tauri::command]
pub async fn interrupt_session(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let cancel = state.loop_cancels.lock().await.get(&session_id).cloned();
    if let Some(tx) = cancel {
        // watch::Sender::send returns Err only when the receiver was dropped,
        // and a stale sender is harmless — ignore both cases.
        let _ = tx.send(true);
    }
    Ok(())
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
        "INSERT INTO active_loops (session_id, started_at, heartbeat_at) VALUES (?, ?, ?) ON CONFLICT(session_id) DO NOTHING",
    )
    .bind(&session_id)
    .bind(&started_at)
    .bind(&started_at)
    .execute(&state.db)
    .await;
    let mut acquired = match insert {
        Ok(res) => res.rows_affected() == 1,
        Err(e) => {
            running.lock().await.remove(&session_id);
            return Err(format!("failed to acquire loop lock: {e}"));
        }
    };
    if !acquired {
        // The in-memory lock is empty, so this DB row belongs to a dead/crashed
        // process (or a concurrent one). Reclaim it only if its heartbeat is
        // stale — a fresh heartbeat means another instance is genuinely running.
        if loop_row_is_stale(&state.db, &session_id).await.unwrap_or(false) {
            let _ = sqlx::query("DELETE FROM active_loops WHERE session_id=?")
                .bind(&session_id)
                .execute(&state.db)
                .await;
            match sqlx::query(
                "INSERT INTO active_loops (session_id, started_at, heartbeat_at) VALUES (?, ?, ?) ON CONFLICT(session_id) DO NOTHING",
            )
            .bind(&session_id)
            .bind(&started_at)
            .bind(&started_at)
            .execute(&state.db)
            .await
            {
                Ok(res) => acquired = res.rows_affected() == 1,
                Err(_) => acquired = false,
            }
        }
    }
    if !acquired {
        running.lock().await.remove(&session_id);
        return Err(format!("Session {session_id} already has an active agentic loop"));
    }

    // Keep the lock row's heartbeat fresh while the loop runs, so a crash is
    // detectable as a stale heartbeat by the next run / revert.
    let hb_db = state.db.clone();
    let hb_sid = session_id.clone();
    let heartbeat_task = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            let res = sqlx::query("UPDATE active_loops SET heartbeat_at=? WHERE session_id=?")
                .bind(chrono::Utc::now().to_rfc3339())
                .bind(&hb_sid)
                .execute(&hb_db)
                .await;
            if res.is_err() {
                break; // db gone — nothing to keep alive
            }
        }
    });

    let mut guard = RunningLoopGuard::new(session_id.clone(), running, state.db.clone(), started_at);

    // Interrupt channel for this session (consumed by interrupt_session/Esc).
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    state.loop_cancels.lock().await.insert(session_id.clone(), cancel_tx);

    // Set by the reload_app tool; when true, exit(23) right after this run
    // so the dev launcher rebuilds and relaunches with the new Rust code.
    let mut exit_app = false;

    let result = run_loop(
        &session_id,
        user_message,
        model,
        provider,
        working_dir,
        auto_approve,
        &app_handle,
        &state,
        cancel_rx,
        &mut exit_app,
    )
    .await;

    // Stop refreshing the heartbeat, then release the lock + cancel handle.
    heartbeat_task.abort();
    // A stale cancel handle would let an old interrupt no-op onto a fresh
    // loop for the same session — drop ours before releasing the guard.
    state.loop_cancels.lock().await.remove(&session_id);
    guard.release().await;

    // The agent requested a reload (reload_app tool): exit the process with
    // RELOAD_CODE (23) so the dev launcher (scripts/dev-app.sh) rebuilds and
    // relaunches with the new binary. Delay so final events flush out.
    if result.is_ok() && exit_app {
        let handle = app_handle.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            eprintln!("[reload_app] exiting (code 23) for relaunch");
            handle.exit(23);
        });
    }
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
    mut cancel: tokio::sync::watch::Receiver<bool>,
    exit_app: &mut bool,
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
You are Pragma, an AI coding agent inside a desktop IDE, working hands-on with the user's code. You work turn by turn, reading files and calling tools to get things done.\n\
\n\
## How this loop works\n\
You converse with the user in turns. In each turn you can write text and call tools. Every tool call is executed for you, and its result is returned to you as a tool_result message. Keep taking actions until the task is complete, then reply with a short final summary. Do not ask permission to do what was already asked — do it with tools. If the user interrupts, stop immediately.\n\
\n\
## Mandatory workflow\n\
1. **Understand first — never guess.** Read the relevant files before editing. Never assume file contents, line numbers, or APIs; verify with read_file / search_files / list_files.\n\
2. **Plan the minimal change.** Make the smallest set of edits that fully satisfies the request. No scope creep, no unsolicited refactoring, no extra features.\n\
3. **Edit precisely.** patch_file for surgical edits (read the file first to pin exact line numbers and context — the diff must match verbatim, no fuzzy matching); write_file for new files or full rewrites.\n\
4. **Verify.** After changing code, run the project's checks (cargo check / cargo test / npm run check / npm run test) and fix any failures before declaring done.\n\
5. **Summarize.** Finish with a concise summary of what you changed and why.\n\
\n\
## Using tools\n\
- Tool inputs must be valid JSON matching the tool's schema exactly.\n\
- Prefer batched read-only calls (read_file / search_files / list_files) when exploring.\n\
- Use run_command for builds, tests, scripts, or inspecting git.\n\
- git_status / git_diff / git_log are read-only. NEVER run git_add or git_commit unless the user explicitly asks.\n\
- web_search / fetch_url only when you need external documentation.\n\
- change_directory switches the working directory when a task targets another subproject.\n\
- reload_app: after Rust/backend changes, it verifies the build then restarts the whole app so the new code takes effect. It ends this run, so call it LAST — never for frontend-only or plugin-only changes (those hot-reload automatically).\n\
\n\
## Failure recovery\n\
- If a tool errors: read the error carefully, fix the cause, and retry. If the identical call fails twice, stop and change approach (different tool, different path, or read more context) — never loop identical calls.\n\
- If the user denies a tool call: do not re-issue it; adapt or ask.\n\
- Tool results are the only source of truth — never invent what a file contains or whether a command succeeded.\n\
\n\
## Style\n\
- Keep your text between tool calls brief; put your effort into the work.\n\
- Write code that matches the project's existing conventions.\n\
- If anything is ambiguous, read more context instead of inventing answers.";

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

    // Consecutive empty/whitespace-only replies get a "continue" nudge; too
    // many in a row means the upstream is actually dead.
    let mut empty_replies = 0u64;

    // Agentic loop
    loop {
        // Esc pressed while we were outside a cancellable await — stop cleanly.
        if interrupted(&cancel) {
            emit(app_handle, session_id, AgentEvent::Interrupted);
            return Ok(());
        }

        // Fit the (ever-growing) history into the context window before each call.
        trim_history_to_budget(&mut history, MAX_HISTORY_CHARS);

        // Call the AI — abortable at the press of Esc (the in-flight HTTP
        // request is dropped, so nothing hangs in the background). On the
        // retryable upstream errors (thinking-mode/`reasoning_content` or
        // tool-pairing validation), re-sanitize the in-memory history and
        // strip reasoning before ONE retry — the relay's session can get out
        // of sync transiently, and this usually lets the request through.
        let mut history_used_for_call = history.clone();
        let blocks = {
            let mut have_retried = false;
            'call: loop {
                let deadline = cancel.changed();
                tokio::pin!(deadline);
                match tokio::select! {
                    res = provider_handle
                        .call(&history_used_for_call, &model, &api_key, &tools, app_handle, session_id) => res,
                    _ = &mut deadline => {
                        emit(app_handle, session_id, AgentEvent::Interrupted);
                        return Ok(());
                    }
                } {
                    Ok(b) => break 'call b,
                    Err(e) if !have_retried => {
                        let retryable = e.contains("reasoning_content")
                            || e.contains("did not have response")
                            || e.contains("tool_call_ids")
                            || e.contains("Invalid tool");
                        if !retryable {
                            return Err(e);
                        }
                        eprintln!(
                            "[providers] session {session_id}: retryable upstream error (attempt 1/2): {e}"
                        );
                        have_retried = true;
                        // Re-sanitize (the in-memory history may have drifted)
                        // and drop all reasoning/thinking blocks for the retry.
                        history_used_for_call = sanitize_history(history.clone());
                        for m in history_used_for_call.iter_mut() {
                            if m["role"].as_str() != Some("assistant") {
                                continue;
                            }
                            let Some(blocks) = m["content"].as_array_mut() else { continue };
                            blocks.retain(|b| {
                                b["type"].as_str() != Some("reasoning")
                                    && b["type"].as_str() != Some("thinking")
                            });
                        }
                        continue 'call;
                    }
                    Err(e) => return Err(e),
                }
            }
        };
        // Adopt the (possibly repaired) history used for the successful call.
        history = history_used_for_call;

        // Drop whitespace-only text blocks (models occasionally emit a bare
        // "\n\n\n" block — it renders as a tall empty bubble and, if it's
        // all the model produced, must not count as a usable answer).
        let mut blocks = blocks;
        blocks.retain(|b| !matches!(b, ContentBlock::Text { text } if text.trim().is_empty()));

        // A turn with no text and no tool calls (empty body, reasoning-only
        // stop, or a mid-stream cut) must not end the loop silently like the
        // model "died" — nudge the model to continue instead of erroring out
        // immediately (the relay often recovers on the next attempt).
        let has_usable = blocks.iter().any(|b| matches!(
            b,
            ContentBlock::Text { .. } | ContentBlock::ToolUse { .. }
        ));
        if !has_usable {
            empty_replies += 1;
            if empty_replies >= 3 {
                let msg = "The model returned no content three times in a row (empty or interrupted response) — check the provider/relay and try again.".to_string();
                emit(app_handle, session_id, AgentEvent::Error { message: msg.clone() });
                return Err(msg);
            }
            eprintln!(
                "[providers] session {session_id}: empty/whitespace-only reply (attempt {empty_replies}/3) — nudging the model to continue"
            );
            // Not persisted to the DB — this is just an in-memory kick so the
            // next API call tells the model what's expected.
            history.push(serde_json::json!({
                "role": "user",
                "content": "Your previous response contained no usable text or tool calls (empty response). Continue now: answer the user's most recent request.",
            }));
            continue;
        }

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

        if interrupted(&cancel) {
            emit(app_handle, session_id, AgentEvent::Interrupted);
            return Ok(());
        }

        // Persist assistant turn (rowid is the checkpoint for file snapshots).
        let blocks_json = serde_json::to_string(&blocks).unwrap();
        let asst_rowid = persist_message(
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
                // Wait for frontend to approve or deny — Esc cancels the wait.
                let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
                state
                    .pending_approvals
                    .lock()
                    .await
                    .insert(tool_use_id.clone(), tx);
                let result = tokio::select! {
                    v = rx => v.unwrap_or(false),
                    _ = cancel.changed() => {
                        // Remove the stale entry so a late approve can't leak.
                        state.pending_approvals.lock().await.remove(tool_use_id);
                        emit(app_handle, session_id, AgentEvent::Interrupted);
                        return Ok(());
                    }
                };
                result
            };

            if !approved {
                any_denied = true;
            }

            let (mut output, is_error) = if !approved {
                ("Tool call denied by user.".to_string(), true)
            } else if name == "reload_app" {
                // Rebuild the backend first — only restart if it compiles.
                let wd = executor.working_dir.clone();
                let out = tokio::select! {
                    res = tokio::task::spawn_blocking(move || {
                        let manifest = if wd.join("src-tauri/Cargo.toml").exists() {
                            wd.join("src-tauri/Cargo.toml")
                        } else {
                            wd.join("Cargo.toml")
                        };
                        if !manifest.exists() {
                            return Err(format!("No Cargo manifest found under {}", wd.display()));
                        }
                        let o = std::process::Command::new("cargo")
                            .args(["build", "--manifest-path", manifest.to_str().unwrap_or("")])
                            .output()
                            .map_err(|e| format!("failed to run cargo build: {e}"))?;
                        if o.status.success() {
                            Ok(())
                        } else {
                            let err = String::from_utf8_lossy(&o.stderr);
                            let tail: String = err
                                .trim_end()
                                .chars()
                                .rev()
                                .take(2800)
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .collect();
                            Err(format!("cargo build FAILED:\n{tail}"))
                        }
                    }) => match res {
                        Ok(r) => r,
                        Err(e) => Err(e.to_string()),
                    },
                    _ = cancel.changed() => {
                        state.pending_approvals.lock().await.remove(tool_use_id);
                        emit(app_handle, session_id, AgentEvent::Interrupted);
                        return Ok(());
                    }
                };
                match out {
                    Ok(()) => {
                        *exit_app = true;
                        ("Build succeeded. A full app reload was scheduled — the app will restart with the new Rust code, so this run ends here. Your conversation is persisted and will be restored after the restart.".to_string(), false)
                    }
                    Err(e) => (e, true),
                }
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
                // Journal the pre-op file state so a later "return to this
                // point" can restore the filesystem.
                if crate::commands::revert::is_mutating_file_tool(name) {
                    let _ = crate::commands::revert::record_snapshot(
                        &state.db,
                        session_id,
                        asst_rowid,
                        tool_use_id,
                        name,
                        input,
                        &executor.working_dir,
                    )
                    .await
                    .map_err(|e| eprintln!("[revert] snapshot failed: {e}"));
                }
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
        if any_denied || *exit_app {
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

    // Id-based pairing. A leftover from interrupted/concurrent runs may have
    // assistant `tool_use` calls and `tool_result` messages stored OUT OF
    // ORDER (their ids don't line up by row position). Pairing by position
    // then emits a call whose result belongs to a different call — the API
    // rejects that ("tool_call_ids did not have response messages" /
    // "reasoning_content must be passed back"). We pair by `id` instead:
    // an assistant tool message is only emitted once every call id has its
    // matching result; results for unknown ids are orphaned and dropped.
    let mut held: Option<serde_json::Value> = None;
    let mut pending: Vec<String> = Vec::new();
    let mut results_buf: Vec<serde_json::Value> = Vec::new();

    for msg in raw {
        let role = msg["role"].as_str().unwrap_or("");
        let blocks = msg["content"].as_array();

        if role == "assistant" {
            let call_ids: Vec<String> = blocks
                .map(|bs| {
                    bs.iter()
                        .filter(|b| b["type"] == "tool_use")
                        .filter_map(|b| b["id"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            if call_ids.is_empty() {
                // Plain (or reasoning-only) assistant turn — emit as-is.
                out.push(msg.clone());
            } else if pending.is_empty() {
                // Start a new tool exchange; buffer until its results arrive.
                held = Some(msg.clone());
                pending = call_ids;
                results_buf = Vec::new();
            } else {
                // A new call appeared before the previous one completed — the
                // previous exchange died (interleaved run). Drop it and start
                // tracking this one.
                held = Some(msg.clone());
                pending = call_ids;
                results_buf = Vec::new();
            }
        } else if role == "user" && pending.is_empty() {
            // Outside any open exchange. Everything is kept as-is, but a
            // user message that is a tool_result here is an orphaned leftover
            // — drop it so the API never sees a response without a call.
            let is_tool_result = blocks
                .map(|bs| bs.iter().any(|b| b["type"] == "tool_result"))
                .unwrap_or(false);
            if !is_tool_result {
                out.push(msg.clone());
            }
        } else if role == "user" && blocks.is_some() {
            // While a tool exchange is open: keep only results for ids we are
            // waiting on; ignore everything else (orphan / foreign results).
            let matched: Vec<serde_json::Value> = blocks
                .unwrap()
                .iter()
                .filter(|b| {
                    b["type"] == "tool_result"
                        && b["tool_use_id"]
                            .as_str()
                            .map(|id| pending.iter().any(|p| p == id))
                            .unwrap_or(false)
                })
                .cloned()
                .collect();
            if matched.is_empty() {
                // A non-tool-result user message interleaved mid-exchange →
                // the buffered call never completed; drop the exchange.
                held = None;
                pending.clear();
                results_buf.clear();
            }
            let matched_ids: Vec<String> = matched
                .iter()
                .filter_map(|m| m["tool_use_id"].as_str().map(|s| s.to_string()))
                .collect();
            pending.retain(|p| !matched_ids.iter().any(|m| m == p));
            results_buf.extend(matched);

            if pending.is_empty() {
                // Every call in this exchange has its result — emit the pair.
                let all_denied = !results_buf.is_empty()
                    && results_buf
                        .iter()
                        .filter(|r| r["type"] == "tool_result")
                        .all(|r| r["content"].as_str() == Some("Tool call denied by user."));
                if all_denied {
                    // Don't feed the model a "tried → denied" trail.
                    held = None;
                } else if let Some(h) = held.take() {
                    out.push(h);
                    out.push(serde_json::json!({"role": "user", "content": results_buf}));
                }
                held = None;
                pending.clear();
                results_buf.clear();
            }
        } else {
            // Plain-text user message while a tool exchange is open — the
            // buffered call never completed. Drop it, keep the new prompt.
            if role == "user" {
                held = None;
                pending.clear();
                results_buf.clear();
            }
            out.push(msg.clone());
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
) -> Result<i64, String> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let res = sqlx::query("INSERT INTO messages (id,session_id,role,content,model,provider,timestamp,tokens) VALUES (?,?,?,?,?,?,?,0)")
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
    Ok(res.last_insert_rowid())
}

/// Has the user pressed Esc/Stop for this loop? Checked at every turn boundary
/// so the loop stops promptly even between cancellable awaits.
fn interrupted(cancel: &tokio::sync::watch::Receiver<bool>) -> bool {
    *cancel.borrow()
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

    #[tokio::test]
    async fn stale_loop_detection() {
        let pool = crate::db::init_db(":memory:").await.unwrap();
        let sid = "sess-stale";
        let now = chrono::Utc::now().to_rfc3339();
        let old = (chrono::Utc::now() - chrono::Duration::minutes(30)).to_rfc3339();
        let fresh = (chrono::Utc::now() - chrono::Duration::seconds(5)).to_rfc3339();

        // No row → nothing to keep alive
        assert!(loop_row_is_stale(&pool, sid).await.unwrap());

        // Fresh heartbeat → live
        sqlx::query("INSERT INTO active_loops (session_id, started_at, heartbeat_at) VALUES (?,?,?)")
            .bind(sid).bind(&old).bind(&fresh).execute(&pool).await.unwrap();
        assert!(!loop_row_is_stale(&pool, sid).await.unwrap());

        // Old heartbeat → stale (crashed process)
        sqlx::query("UPDATE active_loops SET heartbeat_at=? WHERE session_id=?")
            .bind(&old).bind(sid).execute(&pool).await.unwrap();
        assert!(loop_row_is_stale(&pool, sid).await.unwrap());

        // No heartbeat column value (pre-004 rows): fall back to started_at
        sqlx::query("UPDATE active_loops SET heartbeat_at='' WHERE session_id=?")
            .bind(sid).execute(&pool).await.unwrap();
        sqlx::query("UPDATE active_loops SET started_at=? WHERE session_id=?")
            .bind(&now).bind(sid).execute(&pool).await.unwrap();
        assert!(!loop_row_is_stale(&pool, sid).await.unwrap());
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
    fn sanitize_repairs_interleaved_exchange() {
        // Concurrent loops wrote: assistant(tool_use) → assistant(text) → result.
        let raw = vec![
            user_text("hello"),
            asst(vec![tool_use("c1", "run_command")]),
            asst(vec![serde_json::json!({ "type": "text", "text": "Done." })]),
            user_tools(vec![tool_result("c1", "output", false)]),
            user_text("unrelated follow-up"),
        ];
        let clean = sanitize_history(raw);
        // The call legitimately received its result, so the pair is KEPT — the
        // new code just re-orders it after the stray text assistant turn.
        let calls: Vec<_> = clean
            .iter()
            .filter(|m| {
                m["content"]
                    .as_array()
                    .map(|b| b.iter().any(|x| x["type"] == "tool_use"))
                    .unwrap_or(false)
            })
            .collect();
        let results: Vec<_> = clean
            .iter()
            .filter(|m| {
                m["content"]
                    .as_array()
                    .map(|b| b.iter().any(|x| x["type"] == "tool_result"))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(calls.len(), 1);
        assert_eq!(results.len(), 1);
        assert!(clean.iter().any(|m| m["content"] == "unrelated follow-up"));
    }

    #[test]
    fn sanitize_pairs_by_id_not_position() {
        // Two exchanges whose rows are interleaved: c1's call came first, but
        // c2's result arrived first. Position pairing would emit call c1 with
        // result c2 attached → API rejects. Id pairing must drop the dead c1
        // exchange and emit only the complete c2 pair.
        let raw = vec![
            user_text("go"),
            asst(vec![tool_use("c1", "read_file")]),
            asst(vec![tool_use("c2", "write_file")]),
            user_tools(vec![tool_result("c2", "wrote", false)]),
            user_tools(vec![tool_result("c1", "read", false)]),
        ];
        let clean = sanitize_history(raw);
        let calls = clean
            .iter()
            .filter(|m| {
                m["content"]
                    .as_array()
                    .map(|b| b.iter().any(|x| x["type"] == "tool_use"))
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(calls, 1);
        // Every emitted call must be immediately followed by a matching result.
        let mut i = 0;
        while i < clean.len() {
            let b = clean[i]["content"].as_array().cloned().unwrap_or_default();
            let ids: Vec<&str> = b
                .iter()
                .filter(|x| x["type"] == "tool_use")
                .filter_map(|x| x["id"].as_str())
                .collect();
            if !ids.is_empty() {
                let next = &clean[i + 1];
                let res_ids: Vec<&str> = next["content"]
                    .as_array()
                    .map(|bs| {
                        bs.iter()
                            .filter(|x| x["type"] == "tool_result")
                            .filter_map(|x| x["tool_use_id"].as_str())
                            .collect()
                    })
                    .unwrap_or_default();
                assert_eq!(ids.len(), res_ids.len(), "call ids must equal result ids");
                for id in ids {
                    assert!(res_ids.contains(&id), "{id} must have a result");
                }
                i += 2;
            } else {
                i += 1;
            }
        }
    }

    #[test]
    fn sanitize_kills_exchange_when_plain_prompt_interrupts() {
        // A fresh user prompt arrives while the tool result is still missing →
        // the call died; only the follow-up survives.
        let raw = vec![
            user_text("hello"),
            asst(vec![tool_use("c1", "run_command")]),
            user_text("this is a new prompt"),
        ];
        let clean = sanitize_history(raw);
        assert_eq!(clean.len(), 2);
        assert_eq!(clean[0]["content"], "hello");
        assert_eq!(clean[1]["content"], "this is a new prompt");
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
