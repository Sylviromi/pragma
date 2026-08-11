//! The agentic coding loop: turn-by-turn model calls, tool execution,
//! history sanitization and context-window trimming. Split out of
//! `commands/providers.rs` so the provider CRUD commands stay small.

use crate::providers::types::{AgentEvent, ContentBlock, ThinkingPref};
use crate::tools::{definitions, ToolExecutor};
use crate::AppState;
use futures_util::StreamExt;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;
use uuid::Uuid;

/// Tools that operate relative to the working directory — they have nothing
/// to act on when the session has no working dir set, and running them in the
/// app's own cwd (the "..." fallback) would be a silent guess.
/// Deliberately NOT gated: web_search/fetch_url (network), load_skill,
/// plugin sidecar tools (incl. reload-style tools), ask_question (control
/// flow), and change_directory (the fix).
const PATH_GATED_TOOLS: &[&str] = &[
    "read_file", "write_file", "edit_file", "delete_file",
    "list_files", "search_files", "run_command",
    "git_status", "git_diff", "git_log", "git_add", "git_commit",
];

/// True when a tool needs a working directory and the session doesn't have
/// one — the harness blocks it (no execution) and asks the user for a path.
fn path_gated(name: &str, has_wd: bool) -> bool {
    !has_wd && PATH_GATED_TOOLS.contains(&name)
}

/// The "no working directory" fact handed to the model instead of running the
/// tool — named, not an error: the model should change_directory to a path the
/// user has named, or ask_question for one.
fn no_working_dir_fact(tool_name: &str, user_said: Option<&str>) -> String {
    match user_said {
        Some(path) if !path.trim().is_empty() => {
            format!(
                "[tool blocked]: no working directory is set for this session, so {tool_name} has nothing to operate on. The user said to use: {} — call change_directory \"{}\" first, then retry {tool_name}.",
                path.trim(),
                path.trim()
            )
        }
        _ => format!(
            "[tool blocked]: no working directory is set for this session, so {tool_name} has nothing to operate on. If the user already named a project folder, call change_directory <path> first; otherwise ask the user which folder to open (ask_question), then change_directory there before retrying {tool_name}."
        ),
    }
}

/// Extract a plausible existing directory from free text (a user prompt or an
/// answer): absolute paths (leading `/` or `~/`) that exist on disk, picking
/// the longest match when several are mentioned. Returns None for text with no
/// usable path — callers then ask or hand the model a "sort it out" fact.
fn guess_dir_from_text(text: &str) -> Option<String> {
    let mut best: Option<String> = None;
    for raw in text.split_ascii_whitespace() {
        let tok = raw.trim_matches(|c: char| {
            matches!(c, '"' | '\'' | '`' | '(' | ')' | ',' | '.' | ';' | ':' | '!' | '?' | '*',)
        });
        if tok.is_empty() {
            continue;
        }
        let path = if let Some(rest) = tok.strip_prefix("~/") {
            match std::env::var("HOME") {
                Ok(home) => std::path::Path::new(&home).join(rest),
                Err(_) => continue,
            }
        } else if tok.starts_with('/') {
            std::path::PathBuf::from(tok)
        } else {
            continue; // relative fragment or not a path at all
        };
        if !path.is_dir() {
            continue;
        }
        let s = path.to_string_lossy().to_string();
        if best.as_ref().map_or(true, |b| s.len() > b.len()) {
            best = Some(s);
        }
    }
    best
}

/// Persist a new session working dir, rebuild the executor with it, and tell
/// the frontend. Shared by the change_directory tool and the auto-resolution
/// path (guess from message / user's answer).
async fn set_working_dir(
    executor: &mut ToolExecutor,
    has_wd: &mut bool,
    path: &str,
    db: &SqlitePool,
    session_id: &str,
    app_handle: &tauri::AppHandle,
    ledger: crate::tools::SeenLedger,
) -> Result<String, String> {
    let p = std::path::PathBuf::from(path);
    if !p.is_dir() {
        return Err(format!("Directory does not exist: {}", p.display()));
    }
    let path_str = p.canonicalize().unwrap_or(p).to_string_lossy().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE sessions SET working_dir=?,updated_at=? WHERE id=?")
        .bind(&path_str)
        .bind(&now)
        .bind(session_id)
        .execute(db)
        .await;
    *executor = ToolExecutor::with_ledger(&path_str, session_id, ledger);
    *has_wd = true;
    emit(
        app_handle,
        session_id,
        AgentEvent::WorkingDirChanged { new_path: path_str.clone() },
    );
    Ok(path_str)
}

/// Fallback history budget (chars) when the model's context window is unknown
/// (e.g. selected before discovery landed). Real budgets scale with the model's
/// context window via `history_budget_for`; this keeps the pre-discovery
/// behavior identical to the old constant.
const MAX_HISTORY_CHARS: usize = 320_000;

/// Default output ceiling (tokens) reserved out of the context window before
/// history is budgeted, matching the harness's non-thinking max_tokens.
const DEFAULT_MAX_TOKENS: usize = 16_384;

/// Output ceiling (tokens) when extended thinking is enabled — the API forces
/// max_tokens > thinking budget — never handed to history either.
const THINKING_MAX_TOKENS: usize = 25_600;

/// Total wall-clock budget for executing one batch of read-only tool calls in
/// parallel. Bounds the "N tools × 60s" blowup: a batch of 50 tools can no
/// longer stall the loop for 50 minutes — anything still running when this
/// expires is reported to the model as timed out.
const BATCH_EXEC_BUDGET_SECS: u64 = 300;

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

/// Pause the loop and ask the user something — renders the "Needs your answer"
/// block (chips when options are given, free text otherwise) and fires an OS
/// notification when the window is in the background. Returns the answer, or
/// None on timeout. On cancel/interrupt, emits Interrupted and returns Err so
/// the caller unwinds the loop.
async fn ask_user(
    app_handle: &tauri::AppHandle,
    state: &AppState,
    session_id: &str,
    text: &str,
    options: Vec<String>,
    timeout_secs: u64,
    cancel: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<Option<String>, ()> {
    let question_id = Uuid::new_v4().to_string();
    emit(
        app_handle,
        session_id,
        AgentEvent::Question {
            question_id: question_id.clone(),
            text: text.to_string(),
            options,
        },
    );
    crate::notify::notify(app_handle, "Pragma needs your answer", text);
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();
    state.question_channels.lock().await.insert(question_id.clone(), tx);
    let answer = tokio::select! {
        v = rx => v.unwrap_or(None),
        _ = tokio::time::sleep(Duration::from_secs(timeout_secs)) => None,
        _ = cancel.changed() => {
            state.question_channels.lock().await.remove(&question_id);
            emit(app_handle, session_id, AgentEvent::Interrupted);
            return Err(());
        }
    };
    state.question_channels.lock().await.remove(&question_id);
    Ok(answer)
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
    thinking: ThinkingPref,
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

    // Set by a sidecar tool's `reloadApp` marker (pragma-dev's pd_reload_app);
    // when true, exit(23) right after this run so the dev launcher rebuilds
    // and relaunches with the new Rust code.
    let mut exit_app = false;

    let mut active_skills = load_active_skills(&state.db, &session_id).await;
    // Drop stale ids (plugin removed/disabled or native skill gone) so a dead
    // skill can't advertise tools that no longer resolve.
    active_skills.retain(|sid| state.plugins.skill(sid).is_some());

    let result = run_loop(
        &session_id,
        user_message,
        model,
        provider,
        working_dir,
        auto_approve,
        thinking,
        &app_handle,
        &state,
        cancel_rx,
        &mut active_skills,
        &mut exit_app,
    )
    .await;

    // Persist the session's loaded-skill set (grown by load_skill mid-run) so
    // the next run restores the same tools without a re-load.
    persist_active_skills(&state.db, &session_id, &active_skills).await;

    // Stop refreshing the heartbeat, then release the lock + cancel handle.
    heartbeat_task.abort();
    // A stale cancel handle would let an old interrupt no-op onto a fresh
    // loop for the same session — drop ours before releasing the guard.
    state.loop_cancels.lock().await.remove(&session_id);
    guard.release().await;

    // A sidecar tool requested a reload (e.g. pragma-dev's pd_reload_app):
    // exit the process with RELOAD_CODE (23) so the dev launcher
    // (scripts/dev-app.sh) rebuilds and relaunches with the new binary. Delay so final events flush out.
    if result.is_ok() && exit_app {
        let handle = app_handle.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            eprintln!("[pd_reload_app] exiting (code 23) for relaunch");
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
    thinking: ThinkingPref,
    app_handle: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
    mut cancel: tokio::sync::watch::Receiver<bool>,
    active_skills: &mut Vec<String>,
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
    // Tool set = core tools + the tools of every skill already loaded for this
    // session (restored from metadata_json). `load_skill` grows the set mid-run;
    // the caller persists the accumulated list when the run ends.
    let mut tools = definitions::core_tools();
    for sid in active_skills.iter() {
        for t in skill_tool_definitions(&state.plugins, sid) {
            if !tools.iter().any(|x| x.name == t.name) {
                tools.push(t);
            }
        }
    }
    // Bring up sidecars for handler-bearing loaded skills (respawn after
    // plugin hot-reload / app restart). Failures are logged, not fatal — the
    // tool call itself then reports the down sidecar clearly.
    for sid in active_skills.iter() {
        if let Some(skill) = state.plugins.skill(sid) {
            if skill.has_handler {
                let wd = if working_dir.is_empty() {
                    None
                } else {
                    Some(working_dir.as_str())
                };
                if let Err(e) = state
                    .skill_sidecars
                    .ensure(&state.plugins, sid, wd)
                    .await
                {
                    eprintln!("[sidecar] failed to start for '{sid}': {e}");
                }
            }
        }
    }
    let ledger = state.seen_ledger.clone();
    // The session's working dir. Empty = no project is open: path-gated tools
    // are blocked (never silently run in the app's own cwd) until the model
    // calls change_directory with a real folder.
    let mut has_wd = !working_dir.is_empty();
    // Ask the user for a folder at most once per run (the answer rides into
    // every subsequent blocked-tool fact, so it can't get lost).
    let mut path_question_asked = false;
    let mut executor = ToolExecutor::with_ledger(
        if working_dir.is_empty() { ".".into() } else { working_dir },
        &session_id,
        ledger.clone(),
    );

    let provider_handle = state
        .registry
        .get(&provider)
        .ok_or_else(|| format!("unknown provider: {provider}"))?;

    // Derive the per-model context budgets from the provider's model list:
    // long-context models keep proportionally more history instead of being
    // clipped by a global constant, and short-context models trim earlier.
    // Unknown models (pre-discovery) fall back to the old fixed budgets.
    let model_info = provider_handle
        .info()
        .models
        .iter()
        .find(|m| m.id == model)
        .cloned();
    let history_budget = history_budget_for(model_info.as_ref(), thinking.enabled());
    let tool_result_budget = tool_result_budget_for(history_budget);

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
3. **Edit precisely.** edit_file for targeted edits — always pass the hash from the last read_file header (the edit is rejected if the file changed since that read), line numbers from read_file, replacement code only (no diff format, no restating removed lines); write_file for new files or full rewrites (both create parent directories automatically); move/rename: run `mv` via run_command.\n\
4. **Verify.** After changing code, run the project's checks (cargo check / cargo test / npm run check / npm run test) and fix any failures before declaring done.\n\
5. **Summarize.** Finish with a concise summary of what you changed and why.\n\
\n\
## Using tools efficiently\n\
- Tool inputs must be valid JSON matching the tool's schema exactly.\n\
- **Locate then read.** Use search_files or list_files to find which file contains what you need, then read_file on that specific file. Don't blindly read files hoping they contain what you want.\n\
- **Use read_file offset/limit for large files.** When you know the line range (from search_files results or a previous read), read just that range instead of the whole file. Full reads of 500+ line files waste context.\n\
- **Batch read-only calls.** Issue multiple read_file / search_files / list_files calls in the same turn when exploring — don't wait for one result before issuing the next if they're independent.\n\
- **Stop searching once found.** If search_files found the target, don't search again with a different query — read the file at the reported line.\n\
- Use run_command for builds, tests, scripts, or inspecting git.\n\
- git_status / git_diff / git_log are read-only. NEVER run git_add or git_commit unless the user explicitly asks.\n\
- web_search / fetch_url only when you need external documentation.\n\
- **Skills**: your tool set is sized to the task. Core tools (file ops, shell, change_directory, ask_question, load_skill) are always available; everything else — git, web, plugin-provided tools — arrives by loading a skill. Call load_skill on any skill from the index below that seems relevant: its guidance is injected AND its tools are added to your set for the rest of the session. Don't solve a git problem without loading the git skill; don't fetch the web without the web skill.\n\
- change_directory switches the working directory when a task targets another subproject; it is instant and never needs permission.\n\
- **Working directory**: file/shell/git tools run relative to it. When none is set, the harness auto-picks an existing path from your message when it finds one, otherwise asks the user ONE time and then uses whatever path they name. You can also call change_directory yourself with a path you already know (from a loaded skill, the user's message, or their earlier answer) — it always works. Never ask the user for a directory twice; reuse what they already told you.\n\
- **Apply Rust changes**: a loaded skill may contribute a tool that rebuilds the backend and relaunches the app (e.g. pragma-dev's pd_reload_app, available when the pragma-dev skill is loaded). Such a tool ends this run, so call it LAST — never for frontend-only or plugin-only changes (those hot-reload automatically).\n\
- ask_question pauses the loop and asks the user a question when a decision genuinely blocks the work — the user gets an app notification when the window is in the background. Use it only for blocking ambiguity or plan branches the user must decide; NEVER for routine confirmation of work already requested, and never when you can decide by reading more context.\n\
\n\
## Failure recovery\n\
- If a tool errors: read the error carefully, fix the cause, and retry. If the identical call fails twice, stop and change approach (different tool, different path, or read more context) — never loop identical calls.\n\
- If the user denies a tool call: do not re-issue it; adapt or ask.\n\
- Tool results are the only source of truth — never invent what a file contains or whether a command succeeded.\n\
\n\
## Style\n\
- Keep your text between tool calls brief; put your effort into the work.\n\
- Write code that matches the project's existing conventions.\n\
- If anything is ambiguous, read more context instead of inventing answers.\n\
- Always complete your sentences. Never end a response mid-sentence or with trailing whitespace.";

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
    trim_history_to_budget(&mut history, history_budget);

    // First message in this session? Kick off the tiny auto-naming completion
    // (same provider, zero tools) so the session shows a short title in the
    // tab bar / sidebar shortly after the first prompt. Runs before the user
    // message persists so the COUNT(*) detector below sees a fresh session,
    // and never blocks the loop.
    spawn_session_naming(&state, &app_handle, &session_id, &model, &provider, &user_message).await;

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
        trim_history_to_budget(&mut history, history_budget);

        // Call the AI — abortable at the press of Esc (the in-flight HTTP
        // request is dropped, so nothing hangs in the background). On the
        // retryable upstream errors (thinking-mode/`reasoning_content` or
        // tool-pairing validation), re-sanitize the in-memory history and
        // strip reasoning before ONE retry — the relay's session can get out
        // of sync transiently, and this usually lets the request through.
        let mut history_used_for_call = history.clone();
        let (blocks, usage) = {
            let mut have_retried = false;
            'call: loop {
                let deadline = cancel.changed();
                tokio::pin!(deadline);
                match tokio::select! {
                    res = provider_handle
                        .call(&history_used_for_call, &model, &api_key, &tools, thinking, app_handle, session_id) => res,
                    _ = &mut deadline => {
                        emit(app_handle, session_id, AgentEvent::Interrupted);
                        return Ok(());
                    }
                } {
                    Ok(r) => break 'call r,
                    Err(e) if !have_retried => {
                        let retryable = e.contains("reasoning_content")
                            || e.contains("did not have response")
                            || e.contains("tool_call_ids")
                            || e.contains("Invalid tool");
                        if !retryable {
                            crate::notify::notify(app_handle, "Pragma hit an error", &format!("Model request failed: {e}"));
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

        // Milestone 6 telemetry: persist this call's token usage (cost is
        // derived from a best-effort pricing map) so the sidebar's context
        // ring + session cost stay live.
        let _ = crate::commands::stats::record_usage(
            &state.db,
            &session_id,
            &model,
            &provider,
            usage.input_tokens,
            usage.output_tokens,
        )
        .await;

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
                crate::notify::notify(app_handle, "Pragma hit an error", &msg);
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
            // No tool calls — done; push final sidebar stats.
            let _ = crate::commands::stats::emit_stats(
                app_handle,
                &state.db,
                &session_id,
                Some(&executor.working_dir.to_string_lossy()),
            )
            .await;
            emit(app_handle, session_id, AgentEvent::Done);
            break;
        }

        // Handle tool calls
        let run_id = Uuid::new_v4().to_string();
        let mut tool_results: Vec<serde_json::Value> = Vec::new();
        // Plain (executor-backed) tool calls are queued here during the loop
        // and flushed as a parallel batch afterwards: read-only calls run
        // concurrently, so a multi-file exploration turn takes one round trip
        // instead of N. (tool_use_id, name, input, approved, working_dir)
        let mut plain_queue: Vec<(String, String, serde_json::Value, bool, std::path::PathBuf)> = Vec::new();

        for (tool_use_id, name, input) in &tool_calls {
            // ask_question is a control-flow tool: no tool card in the UI and
            // no approval gate — the `question` event itself is the UI. The
            // tool_use/tool_result pair still lands in history (below) so the
            // model's context stays consistent with what it called.
            let is_question = name == "ask_question";

            // A path-gated tool needs a working directory. Resolve it in this
            // order so the user is NEVER asked unless every other source fails:
            //   1. a path mentioned in the user's own message — auto-applied,
            //      no question at all;
            //   2. a one-shot question answered by the user — the path inside
            //      their answer is applied immediately (a sentence like "just
            //      use /mnt/Projects/pragma" works);
            //   3. nothing found — a fact is handed to the model, which has the
            //      user's answer text to mine for a path on its own.
            // Once a working dir is set the tool falls through to the normal
            // approval + execution path (non-path tools always run freely).
            if path_gated(&name, has_wd) {
                if !has_wd {
                    if let Some(guess) = guess_dir_from_text(&user_message) {
                        let _ = set_working_dir(
                            &mut executor, &mut has_wd, &guess,
                            &state.db, &session_id, app_handle, ledger.clone(),
                        ).await;
                    }
                }
                let mut user_said: Option<String> = None;
                if !has_wd {
                    let answer = if !path_question_asked {
                        path_question_asked = true;
                        ask_user(
                            app_handle,
                            &state,
                            &session_id,
                            "Which project folder should I use as the working directory? Name a path (e.g. /home/you/project) or just say where to work.",
                            Vec::new(),
                            600,
                            &mut cancel,
                        )
                        .await
                    } else {
                        Ok(None)
                    };
                    match answer {
                        Err(()) => return Ok(()), // user pressed Esc during the ask
                        Ok(a) => {
                            let a = a.filter(|s| !s.trim().is_empty());
                            user_said = a.clone();
                            if let Some(p) = a.as_deref().and_then(guess_dir_from_text) {
                                let _ = set_working_dir(
                                    &mut executor, &mut has_wd, &p,
                                    &state.db, &session_id, app_handle, ledger.clone(),
                                ).await;
                            }
                        }
                    }
                }
                if !has_wd {
                    let fact = no_working_dir_fact(&name, user_said.as_deref());
                    emit(app_handle, session_id, AgentEvent::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        output: fact.clone(),
                        is_error: false,
                    });
                    tool_results.push(serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": fact,
                        "is_error": false,
                    }));
                    continue;
                }
                // A working directory was resolved (guessed or answered) — fall
                // through and run the tool normally.
            }

            // Emit tool call event to frontend (questions render from the
            // `question` event instead — no ToolCall, no approval buttons).
            if !is_question {
                emit(app_handle, session_id, AgentEvent::ToolCall {
                    run_id: run_id.clone(),
                    tool_use_id: tool_use_id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                });
            }

            // OS notification when a *state-changing* tool is about to block on
            // approval (read-only tools don't interrupt the user's flow;
            // questions notify via the Question event instead).
            if !auto_approve && !is_question {
                let mutating = crate::commands::revert::is_mutating_file_tool(name)
                    || matches!(name.as_str(), "run_command" | "git_add" | "git_commit");
                if mutating {
                    crate::notify::notify(app_handle, "Pragma needs approval", &format!("Do you want to allow the tool: {name}?"));
                }
            }

            let approved = if auto_approve || is_question {
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

            let (mut output, is_error) = if !approved {
                ("Tool call denied by user.".to_string(), true)
            } else if name == "load_skill" {
                let skill_id = input.get("skill_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                match state.plugins.skill(&skill_id) {
                    None => (
                        format!("No enabled skill found with id: {skill_id} — see the skill index in the system prompt for valid skill ids"),
                        true,
                    ),
                    Some(_) => match state.plugins.skill_doc(&skill_id) {
                        Ok(md) => {
                            if !active_skills.iter().any(|s| s == &skill_id) {
                                active_skills.push(skill_id.clone());
                            }
                            // Advertise the skill's tools to the model from the
                            // next provider call onward (tools is borrowed per
                            // call payload, so mutating it here is safe).
                            let mut added: Vec<String> = Vec::new();
                            for t in skill_tool_definitions(&state.plugins, &skill_id) {
                                if !tools.iter().any(|x| x.name == t.name) {
                                    added.push(t.name.clone());
                                    tools.push(t);
                                }
                            }
                            let mut msg = md;
                            msg.push('\n');
                            msg.push_str(&if added.is_empty() {
                                format!("Skill '{skill_id}' is now active. It declares no extra tools.")
                            } else {
                                format!("Skill '{skill_id}' is now active. New tools available to you: {}", added.join(", "))
                            });
                            (msg, false)
                        }
                        Err(e) => (format!("Failed to load skill '{skill_id}': {e}"), true),
                    },
                }
            } else if name == "change_directory" {
                let new_dir = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let abs = if std::path::Path::new(new_dir).is_absolute() {
                    std::path::PathBuf::from(new_dir)
                } else {
                    executor.working_dir.join(new_dir)
                };
                match set_working_dir(
                    &mut executor, &mut has_wd,
                    &abs.to_string_lossy(),
                    &state.db, &session_id, app_handle, ledger.clone(),
                ).await {
                    Ok(path_str) => (format!("Working directory changed to: {path_str}"), false),
                    Err(e) => (e, true),
                }
            } else if name == "ask_question" {
                // Ask the user a question and pause until they answer (or the
                // timeout elapses / Esc is pressed). The frontend renders the
                // "Needs your answer" block and resumes via answer_question.
                let text = input.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let options: Vec<String> = input
                    .get("options")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                if text.trim().is_empty() {
                    ("ask_question requires a non-empty \"text\" argument.".to_string(), true)
                } else {
                    let timeout_secs = input
                        .get("timeout_secs")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(600)
                        .max(10);
                    match ask_user(app_handle, &state, &session_id, &text, options, timeout_secs, &mut cancel).await {
                        Ok(Some(ans)) if !ans.trim().is_empty() => {
                            (format!("User answered: {ans}"), false)
                        }
                        Ok(_) => {
                            // Timeout (or an explicit no-answer) — continue with best judgment.
                            ("User did not answer within the timeout — proceed with your best judgment, and do not re-ask the same question.".to_string(), false)
                        }
                        Err(()) => return Ok(()), // Esc pressed — unwind
                    }
                }
            } else if let Some(skill_id) = state.skill_sidecars.owner(&name) {
                // Plugin-skill sidecar tool: routed to the skill's handler
                // process over JSON-RPC (a tool the skill declared in its
                // tools.json). Runs inline — plugin tools are usually
                // stateful (they control the plugin's own server), so no
                // batch parallelism. Approval already happened above.
                let out = tokio::select! {
                    res = state.skill_sidecars.call_tool(&skill_id, &name, input) => res,
                    _ = cancel.changed() => {
                        emit(app_handle, session_id, AgentEvent::Interrupted);
                        return Ok(());
                    }
                };
                match out {
                    Ok(s) => {
                        // Relaunch convention: a sidecar tool (e.g. pragma-dev's
                        // pd_reload_app) may request an app relaunch after the
                        // run by returning a JSON object with `"reloadApp": true`.
                        // The handler sets it ONLY after a successful build, so a
                        // failed build never triggers a restart. The post-run
                        // exit(23) relaunch is handled by the caller.
                        let wants_reload = serde_json::from_str::<serde_json::Value>(&s)
                            .ok()
                            .and_then(|v| {
                                v.get("reloadApp").and_then(|b| b.as_bool())
                            })
                            .unwrap_or(false);
                        if wants_reload {
                            *exit_app = true;
                        }
                        (s, false)
                    }
                    Err(e) => (e, true),
                }
            } else {
                // Plain tool (file/shell/git/web): queue for the parallel
                // batch after the loop — snapshots, execution and telemetry
                // happen at flush time so read-only calls run concurrently.
                plain_queue.push((
                    tool_use_id.clone(),
                    name.clone(),
                    input.clone(),
                    approved,
                    executor.working_dir.clone(),
                ));
                continue;
            };
            // Cap oversized outputs so one big result can't blow the context
            // window (and the persisted history) for the whole conversation.
            output = truncate_output(&output, tool_result_budget);

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

        // ── Flush the plain batch: parallel-safe execution ───────────────
        // Denied + mutating calls run serially (writes must not race);
        // read-only calls run concurrently under one budget. Results are
        // collected by call index so the persisted turn keeps the model's
        // original order, and every ToolResult event fires before the turn
        // is persisted.
        if !plain_queue.is_empty() {
            let mut flush_results: std::collections::BTreeMap<usize, serde_json::Value> =
                std::collections::BTreeMap::new();
            let mut read_only: Vec<(usize, String, String, serde_json::Value, std::path::PathBuf)> =
                Vec::new();

            // 1. Serial pass: denied (instant) and mutating (ordered) calls.
            for (i, (tool_use_id, name, input, approved, wd)) in plain_queue.iter().enumerate() {
                let is_mutating = crate::commands::revert::is_mutating_file_tool(name)
                    || matches!(name.as_str(), "run_command" | "git_add" | "git_commit");
                if *approved && !is_mutating {
                    read_only.push((i, tool_use_id.clone(), name.clone(), input.clone(), wd.clone()));
                    continue;
                }
                let (mut output, is_error) = if !approved {
                    ("Tool call denied by user.".to_string(), true)
                } else {
                    let ex = ToolExecutor::with_ledger(wd, &session_id, ledger.clone());
                    // Journal the pre-op file state so a later "return to
                    // this point" can restore the filesystem.
                    if crate::commands::revert::is_mutating_file_tool(name) {
                        let _ = crate::commands::revert::record_snapshot(
                            &state.db,
                            session_id,
                            asst_rowid,
                            tool_use_id,
                            name,
                            input,
                            &ex.working_dir,
                        )
                        .await
                        .map_err(|e| eprintln!("[revert] snapshot failed: {e}"));
                    }
                    match ex.execute(name, input) {
                        Ok(out) => {
                            // Telemetry: what the tool did to the project tree.
                            let _ = crate::commands::stats::record_tool_touch(
                                &state.db,
                                &session_id,
                                name,
                                input,
                            )
                            .await;
                            (out, false)
                        }
                        Err(e) => (e, true),
                    }
                };
                output = truncate_output(&output, tool_result_budget);
                emit(app_handle, session_id, AgentEvent::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    output: output.clone(),
                    is_error,
                });
                flush_results.insert(i, serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": tool_use_id,
                    "content": output,
                    "is_error": is_error,
                }));
            }

            // 2. Parallel pass for read-only calls, streamed as each finishes,
            //    under one batch budget (Esc cancels the whole batch).
            if !read_only.is_empty() {
                let mut futs: futures_util::stream::FuturesUnordered<_> = read_only
                    .iter()
                    .map(|(i, id, name, input, wd)| {
                        let ex = ToolExecutor::with_ledger(wd, &session_id, ledger.clone());
                        let id = id.clone();
                        let name = name.clone();
                        let input = input.clone();
                        async move {
                            let name_arg = name.clone();
                            let input_arg = input.clone();
                            let res =
                                tokio::task::spawn_blocking(move || ex.execute(&name_arg, &input_arg))
                                    .await;
                            (*i, id, name, res)
                        }
                    })
                    .collect();
                let deadline = tokio::time::Instant::now()
                    + tokio::time::Duration::from_secs(BATCH_EXEC_BUDGET_SECS);
                let mut completed: HashSet<usize> = HashSet::new();
                loop {
                    tokio::select! {
                        maybe = futs.next() => {
                            let Some((i, id, name, res)) = maybe else { break };
                            completed.insert(i);
                            let (mut output, is_error) = match res {
                                Ok(Ok(out)) => {
                                    let _ = crate::commands::stats::record_tool_touch(
                                        &state.db,
                                        &session_id,
                                        &name,
                                        &read_only[i].3,
                                    )
                                    .await;
                                    (out, false)
                                }
                                Ok(Err(e)) => (e, true),
                                Err(e) => (format!("tool execution panicked: {e}"), true),
                            };
                            output = truncate_output(&output, tool_result_budget);
                            emit(app_handle, session_id, AgentEvent::ToolResult {
                                tool_use_id: id.clone(),
                                output: output.clone(),
                                is_error,
                            });
                            flush_results.insert(i, serde_json::json!({
                                "type": "tool_result",
                                "tool_use_id": id,
                                "content": output,
                                "is_error": is_error,
                            }));
                        }
                        _ = tokio::time::sleep_until(deadline) => break,
                        _ = cancel.changed() => {
                            for (_, id, _, _, _) in &read_only {
                                state.pending_approvals.lock().await.remove(id);
                            }
                            emit(app_handle, session_id, AgentEvent::Interrupted);
                            return Ok(());
                        }
                    }
                }
                // Anything still pending hit the batch budget — tell the model
                // so it doesn't silently lose an answer.
                for (i, id, name, _, _) in &read_only {
                    if completed.contains(i) {
                        continue;
                    }
                    let msg = format!(
                        "Tool {name} did not finish within the {BATCH_EXEC_BUDGET_SECS}s batch budget and was cancelled. Retry it, or narrow the request."
                    );
                    emit(app_handle, session_id, AgentEvent::ToolResult {
                        tool_use_id: id.clone(),
                        output: msg.clone(),
                        is_error: true,
                    });
                    flush_results.insert(*i, serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": id,
                        "content": msg,
                        "is_error": true,
                    }));
                }
            }

            // Emitted in the model's original call order.
            tool_results.extend(flush_results.into_values());
        }

        // Add tool results as user turn
        let results_content = serde_json::to_string(&tool_results).unwrap();
        persist_message(&state.db, &session_id, "user", &results_content, "", "").await?;
        history.push(serde_json::json!({ "role": "user", "content": tool_results }));

        // Push refreshed sidebar stats after every tool round.
        let _ = crate::commands::stats::emit_stats(
            app_handle,
            &state.db,
            &session_id,
            Some(&executor.working_dir.to_string_lossy()),
        )
        .await;

        // Stop only when the app is reloading. A denied tool is fed back as an
        // error result and the model is expected to adapt (the system prompt
        // forbids re-issuing it) — the turn continues with the approved ones.
        if *exit_app {
            emit(app_handle, session_id, AgentEvent::Done);
            break;
        }
    }

    Ok(())
}

/// Tool definitions contributed by a skill: native Rust tools (git/web) plus
/// any tools declared in a plugin skill's tools.json. Used both at run start
/// (restoring a session's already-loaded skills) and when load_skill fires.
fn skill_tool_definitions(
    plugins: &crate::plugins::PluginManager,
    skill_id: &str,
) -> Vec<crate::providers::types::ToolDefinition> {
    let mut out = definitions::tools_for_skill(skill_id);
    if let Ok(tools) = plugins.skill_tools_json(skill_id) {
        out.extend(tools);
    }
    out
}

/// The session's metadata_json as a JSON value ({} when missing/broken).
async fn session_metadata(pool: &SqlitePool, session_id: &str) -> serde_json::Value {
    let meta: Option<String> = sqlx::query_scalar("SELECT metadata_json FROM sessions WHERE id=?")
        .bind(session_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    meta.and_then(|m| serde_json::from_str(&m).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Skills the model has loaded for this session (persisted in
/// sessions.metadata_json.activeSkills) — restored at every run start so their
/// tools stay available across user turns and app restarts.
async fn load_active_skills(pool: &SqlitePool, session_id: &str) -> Vec<String> {
    session_metadata(pool, session_id)
        .await
        .get("activeSkills")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Merge the session's loaded-skill list into its metadata_json (keeps other
/// keys, updates updated_at).
async fn persist_active_skills(pool: &SqlitePool, session_id: &str, skills: &[String]) {
    let mut obj = session_metadata(pool, session_id)
        .await
        .as_object()
        .cloned()
        .unwrap_or_default();
    obj.insert("activeSkills".into(), serde_json::json!(skills));
    let _ = sqlx::query("UPDATE sessions SET metadata_json=?, updated_at=? WHERE id=?")
        .bind(serde_json::Value::Object(obj).to_string())
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(session_id)
        .execute(pool)
        .await;
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
                if !all_denied {
                    // all_denied: don't feed the model a "tried -> denied" trail.
                    if let Some(h) = held.take() {
                        out.push(h);
                        out.push(serde_json::json!({"role": "user", "content": results_buf}));
                    }
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

/// History budget (chars) for a model, derived from its context window so
/// long-context models keep proportionally more history instead of being
/// clipped by a fixed constant: `(context − output_reserve) × 2`. Real content
/// runs ≈4 chars/token, so this hands about half the window to history and
/// leaves the rest for the system prompt, tool schemas, and model output.
/// Falls back to the old global constant (320K chars) when the model is unknown.
fn history_budget_for(model: Option<&crate::providers::types::ModelInfo>, thinking: bool) -> usize {
    let Some(m) = model else {
        return MAX_HISTORY_CHARS;
    };
    let ctx = m.context_window.unwrap_or(200_000) as usize;
    // Never let history crowd out the model's own output. With thinking on the
    // API forces max_tokens > thinking budget, so reserve that ceiling too.
    let reserve = m.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS as u32) as usize;
    let reserve = if thinking { reserve.max(THINKING_MAX_TOKENS) } else { reserve };
    ctx.saturating_sub(reserve) * 2
}

/// Tool-result cap derived from the history budget: one result shouldn't eat
/// more than ~a fifth of the whole history body. Clamped so tiny windows don't
/// produce absurdly small results and huge windows stay bounded.
fn tool_result_budget_for(history_budget: usize) -> usize {
    (history_budget / 5).clamp(16_000, 256_000)
}

/// Truncate a tool result so a single huge output can't blow the context window.
fn truncate_output(s: &str, cap: usize) -> String {
    if s.chars().count() <= cap {
        return s.to_string();
    }
    let cut: String = s.chars().take(cap).collect();
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

/// If this is the session's very first message, spawn a fire-and-forget
/// "naming" completion (same provider, zero tools, thinking off) that persists
/// a short session title once it returns. Any failure leaves the default
/// timestamp name in place — naming must never disrupt the agentic loop.
async fn spawn_session_naming(
    state: &tauri::State<'_, AppState>,
    app_handle: &tauri::AppHandle,
    session_id: &str,
    model: &str,
    provider: &str,
    user_message: &str,
) {
    let first: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE session_id=?")
        .bind(session_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    if first != 0 {
        return;
    }
    let db = state.db.clone();
    let registry = state.registry.clone();
    let api_key = state
        .settings
        .read()
        .unwrap()
        .api_keys
        .get(provider)
        .cloned()
        .unwrap_or_default();
    let app = app_handle.clone();
    let sid = session_id.to_string();
    let model = model.to_string();
    let provider = provider.to_string();
    let prompt = user_message.to_string();
    tokio::spawn(async move {
        auto_name_session(&db, &registry, &api_key, &app, &sid, &model, &provider, &prompt).await;
    });
}

/// Run the naming completion and persist the result. Best-effort: a failure
/// just logs nothing and leaves the timestamp-based name untouched.
async fn auto_name_session(
    db: &SqlitePool,
    registry: &crate::providers::ProviderRegistry,
    api_key: &str,
    app_handle: &tauri::AppHandle,
    session_id: &str,
    model: &str,
    provider: &str,
    prompt: &str,
) {
    let Some(handle) = registry.get(provider) else { return };
    let clean: String = prompt
        .chars()
        .take(240)
        .collect::<String>()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if clean.is_empty() {
        return;
    }
    let messages = vec![
        serde_json::json!({
            "role": "system",
            "content": "You name sessions in an AI coding harness. Given the user's first message, reply with ONLY a short session title: 2-5 words, no quotes, no punctuation, no explanation.",
        }),
        serde_json::json!({ "role": "user", "content": clean }),
    ];
    // Fake event channel: call() internally emits agent events (tokens, etc.)
    // on `agent:{session_id}` — route the naming stream to a channel nobody
    // listens on so nothing leaks into the live chat render.
    let fake_sid = format!("{session_id}:naming");
    let Ok((blocks, _)) = handle
        .call(&messages, model, api_key, &[], ThinkingPref::Off, app_handle, &fake_sid)
        .await
    else {
        return;
    };
    let Some(name) = extract_name(&blocks) else { return };
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE sessions SET name=?, updated_at=? WHERE id=?")
        .bind(&name)
        .bind(&now)
        .bind(session_id)
        .execute(db)
        .await;
    let _ = app_handle.emit(
        "session:renamed",
        serde_json::json!({ "id": session_id, "name": name }),
    );
}

/// Take a short title out of the naming completion: first non-empty line,
/// stripped of quotes/dashes/markdown and trailing punctuation, capped at 48
/// chars. Returns None when there's nothing usable.
fn extract_name(blocks: &[ContentBlock]) -> Option<String> {
    let text: Vec<String> = blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    let joined = text.join(" ");
    let first_line = joined.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return None;
    }
    let cleaned = first_line.trim_matches(|c| matches!(c, '"' | '\'' | '\u{201c}' | '\u{201d}' | '-' | '*' | '`'));
    let cleaned = cleaned.trim_end_matches(|c: char| matches!(c, '.' | ',' | '!' | '?' | ':' | ';'));
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return None;
    }
    let capped: String = cleaned.chars().take(48).collect();
    let capped = capped.trim();
    if capped.is_empty() {
        return None;
    }
    Some(capped.to_string())
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
        let long = "x".repeat(74_000);
        let capped = truncate_output(&long, 64_000);
        assert!(capped.len() < long.len());
        assert!(capped.contains("truncated"));
        // Short outputs pass through untouched
        assert_eq!(truncate_output("short", 64_000), "short");
    }

    #[test]
    fn history_budget_scales_with_context_window() {
        // 200K model, thinking off: (200_000 − 16_384) × 2 ≈ 367K chars —
        // comparable to the old 320K constant, not clipped by it.
        let b200 = history_budget_for(Some(&crate::providers::catalog::enrich(
            "m", "M".into(), Some(200_000), None, None, None, None,
        )), false);
        assert_eq!(b200, (200_000 - 16_384) * 2);
        // Thinking reserves the higher output ceiling.
        let bt = history_budget_for(Some(&crate::providers::catalog::enrich(
            "m", "M".into(), Some(200_000), None, None, None, None,
        )), true);
        assert_eq!(bt, (200_000 - 25_600) * 2);
        // Long-context models scale up proportionally.
        let b1m = history_budget_for(Some(&crate::providers::catalog::enrich(
            "m", "M".into(), Some(1_000_000), None, None, None, None,
        )), false);
        assert_eq!(b1m, (1_000_000 - 16_384) * 2);
        // Unknown model → old constant.
        assert_eq!(history_budget_for(None, false), MAX_HISTORY_CHARS);
        // Tool-result cap is a fifth of the history budget, clamped.
        assert_eq!(tool_result_budget_for(b200), b200 / 5);
        assert_eq!(tool_result_budget_for(40_000), 16_000);
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
    #[test]
    fn path_tools_are_gated_without_wd() {
        for t in ["read_file", "write_file", "edit_file", "delete_file",
                  "list_files", "search_files", "run_command",
                  "git_status", "git_diff", "git_log", "git_add", "git_commit"] {
            assert!(path_gated(t, false), "{t} must be gated without a working dir");
        }
    }

    #[test]
    fn control_tools_are_never_gated() {
        // change_directory IS the fix — blocking it would deadlock the flow.
        for t in ["change_directory", "ask_question", "web_search", "fetch_url", "load_skill"] {
            assert!(!path_gated(t, false), "{t} must never be gated");
        }
        // Unknown tools fall through to the normal unknown-tool error.
        assert!(!path_gated("bogus_tool", false));
    }

    #[tokio::test]
    async fn active_skills_persist_and_restore_across_runs() {
        let pool = crate::db::init_db(":memory:").await.unwrap();
        let session_id = "s-skill-persist";
        sqlx::query(
            "INSERT INTO sessions (id,name,working_dir,created_at,updated_at,metadata_json) VALUES (?,?,?,?,?,?)",
        )
        .bind(session_id)
        .bind("t")
        .bind("")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(chrono::Utc::now().to_rfc3339())
        .bind("{\"other\": 1}")
        .execute(&pool)
        .await
        .unwrap();

        // nothing loaded initially
        assert!(load_active_skills(&pool, session_id).await.is_empty());

        // persist a set, keep other metadata keys
        persist_active_skills(&pool, session_id, &["git".into(), "web".into()]).await;
        let skills = load_active_skills(&pool, session_id).await;
        assert_eq!(skills, vec!["git".to_string(), "web".to_string()]);
        let meta = session_metadata(&pool, session_id).await;
        assert_eq!(meta["other"], 1, "unrelated metadata keys survive");

        // re-persist replaces the list
        persist_active_skills(&pool, session_id, &["git".into()]).await;
        assert_eq!(
            load_active_skills(&pool, session_id).await,
            vec!["git".to_string()]
        );
    }

    #[test]
    fn skill_tool_definitions_merge_native_and_plugin_tools() {
        // native git tools resolve even with no plugins installed
        let mgr = crate::plugins::PluginManager::new(
            std::env::temp_dir().join(format!("pragma-test-{}", std::process::id())),
            vec![],
        );
        let git = skill_tool_definitions(&mgr, "git");
        assert!(git.iter().any(|t| t.name == "git_status"));
        assert!(git.iter().any(|t| t.name == "git_commit"));
        // unknown skill → empty
        assert!(skill_tool_definitions(&mgr, "nope").is_empty());
    }

    #[test]
    fn nothing_is_gated_with_a_wd() {
        for t in ["read_file", "run_command", "git_commit", "search_files", "write_file"] {
            assert!(!path_gated(t, true), "{t} runs normally once a working dir is set");
        }
    }

    #[test]
    fn no_wd_fact_carries_the_user_answer() {
        let with = no_working_dir_fact("read_file", Some("/mnt/Projects/app  "));
        assert!(with.contains("change_directory \"/mnt/Projects/app\""), "fact: {with}");
        assert!(with.contains("retry read_file"), "fact: {with}");
        let none = no_working_dir_fact("write_file", None);
        assert!(none.contains("ask_question"), "fact: {none}");
        assert!(none.contains("change_directory"), "fact: {none}");
    }

    #[test]
    fn guess_dir_from_text_finds_paths() {
        let cwd = std::env::current_dir().unwrap();
        let cwd_s = cwd.to_string_lossy().to_string();
        // A sentence carrying an existing path — auto-applied, no question.
        assert_eq!(
            guess_dir_from_text(&format!("just work in {cwd_s} please")),
            Some(cwd_s.clone())
        );
        // Longest existing path wins when several are mentioned.
        let parent = cwd.parent().unwrap().to_string_lossy().to_string();
        assert_eq!(guess_dir_from_text(&format!("{parent} and {cwd_s}")), Some(cwd_s));
        // Nonexistent paths and relative fragments are ignored.
        assert_eq!(guess_dir_from_text("use /nonexistent/xyz or the local folder"), None);
        assert_eq!(guess_dir_from_text("no paths here at all"), None);
        assert_eq!(guess_dir_from_text(""), None);
    }

    #[test]
    fn extract_name_cleans_first_line() {
        // Strips quotes, keeps capitalization, removes trailing punctuation.
        assert_eq!(
            extract_name(&[ContentBlock::Text { text: "\"Fix the websocket leak\"".into() }]),
            Some("Fix the websocket leak".into())
        );
        // Leading dashes/markdown get trimmed; only the first line counts.
        assert_eq!(
            extract_name(&[ContentBlock::Text { text: "- Short title\n\nExtra chatter here".into() }]),
            Some("Short title".into())
        );
        // Long output is capped at 48 chars.
        let long = "a".repeat(80);
        assert_eq!(extract_name(&[ContentBlock::Text { text: long }]), Some("a".repeat(48)));
        // Nothing usable → None (no rename happens).
        assert_eq!(extract_name(&[ContentBlock::Text { text: "   ".into() }]), None);
        assert_eq!(extract_name(&[]), None);
    }

}
