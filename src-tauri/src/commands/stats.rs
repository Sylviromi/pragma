//! Telemetry backing the right sidebar: context usage, session cost, plan
//! checklist, files touched and the git working tree. All data is derived
//! from the SQLite DB (session_usage / touched_files / plan_items) plus live
//! filesystem state (`.handsfree/plan.md`, `git status/diff`).

use crate::AppState;
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::Path;
use tauri::Emitter;

// ── Serialized shapes (camelCase to match src/lib/types.ts) ────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSegment {
    pub label: String,
    pub tokens: i64,
    pub color: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsage {
    pub segments: Vec<ContextSegment>,
    pub used: i64,
    pub limit: i64,
    pub percent: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostRow {
    pub model: String,
    pub tokens: String,
    pub cost: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCost {
    pub rows: Vec<CostRow>,
    pub total: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub label: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlan {
    pub items: Vec<PlanItem>,
    pub progress: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchedFile {
    pub path: String,
    pub kind: String,
    pub added: Option<i64>,
    pub removed: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRow {
    pub mark: String,
    pub path: String,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorkingTree {
    pub branch: String,
    pub added: i64,
    pub removed: i64,
    pub files: i64,
    pub rows: Vec<GitRow>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub session_id: String,
    pub context: ContextUsage,
    pub cost: SessionCost,
    pub plan: SessionPlan,
    pub files: Vec<TouchedFile>,
    pub git: GitWorkingTree,
}

impl Default for ContextUsage {
    fn default() -> Self {
        Self {
            segments: vec![
                ContextSegment {
                    label: "System + skills".into(),
                    tokens: 0,
                    color: "var(--text-muted)".into(),
                },
                ContextSegment {
                    label: "Files in context".into(),
                    tokens: 0,
                    color: "var(--warning)".into(),
                },
                ContextSegment {
                    label: "Conversation".into(),
                    tokens: 0,
                    color: "var(--accent)".into(),
                },
            ],
            used: 0,
            limit: 200_000,
            percent: 0,
        }
    }
}

impl Default for SessionCost {
    fn default() -> Self {
        Self { rows: Vec::new(), total: "$0.00".into() }
    }
}

impl Default for SessionPlan {
    fn default() -> Self {
        Self { items: Vec::new(), progress: String::new() }
    }
}

impl Default for GitWorkingTree {
    fn default() -> Self {
        Self { branch: String::new(), added: 0, removed: 0, files: 0, rows: Vec::new() }
    }
}

// ── Pricing (best-effort USD per 1M tokens; extendable, see TODO.md) ───────

fn price_per_1m(model: &str) -> Option<(f64, f64)> {
    let m = model.to_lowercase();
    if m.contains("claude-sonnet") {
        Some((3.0, 15.0))
    } else if m.contains("claude-opus") {
        Some((15.0, 75.0))
    } else if m.contains("claude-haiku") {
        Some((0.80, 4.0))
    } else if m.contains("deepseek") {
        Some((0.27, 1.10))
    } else if m.contains("mimo") {
        Some((0.0, 0.0))
    } else {
        None // unknown model — $0 until the table is extended
    }
}

/// Compute the USD cost of a call. Unknown models cost nothing (the pricing
/// table is best-effort and never blocks usage).
pub fn model_cost_usd(model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    match price_per_1m(model) {
        Some((in_rate, out_rate)) => {
            (input_tokens as f64 * in_rate + output_tokens as f64 * out_rate) / 1_000_000.0
        }
        None => 0.0,
    }
}

// ── Writers (called from the agentic loop) ─────────────────────────────────

pub async fn record_usage(
    db: &SqlitePool,
    session_id: &str,
    model: &str,
    provider: &str,
    input_tokens: u64,
    output_tokens: u64,
) -> Result<(), String> {
    let cost = model_cost_usd(model, input_tokens, output_tokens);
    sqlx::query(
        "INSERT INTO session_usage (id, session_id, model, provider, input_tokens, output_tokens, cost_usd) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(model)
    .bind(provider)
    .bind(input_tokens as i64)
    .bind(output_tokens as i64)
    .bind(cost)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Record a touched file after a successful file tool. kind/added/removed are
/// derived from the tool call's own input (the truthful, cheap source — the
/// file is on disk, this just describes what changed).
pub async fn record_tool_touch(
    db: &SqlitePool,
    session_id: &str,
    tool_name: &str,
    input: &serde_json::Value,
) -> Result<(), String> {
    let (kind, path, added, removed): (&str, String, Option<i64>, Option<i64>) = match tool_name {
        "write_file" | "edit_file" => {
            let path = input["path"].as_str().unwrap_or("").to_string();
            let content = input["content"].as_str().unwrap_or("");
            let n = content.lines().count().max(1) as i64;
            ("edited", path, Some(n), Some(0))
        }
        "delete_file" => (
            "deleted",
            input["path"].as_str().unwrap_or("").to_string(),
            Some(0),
            Some(0),
        ),
        "read_file" => ("read", input["path"].as_str().unwrap_or("").to_string(), None, None),
        "search_files" => ("read", input["pattern"].as_str().unwrap_or("").to_string(), None, None),
        _ => return Ok(()), // run_command / git / web tools don't touch the project tree
    };
    if path.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO touched_files (id, session_id, path, kind, added, removed) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(path)
    .bind(kind)
    .bind(added)
    .bind(removed)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Readers (used by get_session_stats + session:stats events) ─────────────

async fn total_usage(db: &SqlitePool, session_id: &str) -> (i64, i64) {
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0) FROM session_usage WHERE session_id=?",
    )
    .bind(session_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    row.unwrap_or((0, 0))
}

/// Estimate the conversation-half of the context budget from persisted
/// messages (chars / 4 ≈ tokens). Tool-result user messages are excluded —
/// they're counted under "files in context" style tool payloads.
async fn conversation_tokens(db: &SqlitePool, session_id: &str) -> i64 {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT role, content FROM messages WHERE session_id=?")
        .bind(session_id)
        .fetch_all(db)
        .await
        .unwrap_or_default();
    let mut n = 0i64;
    for (role, content) in rows {
        let is_tool_result = role == "user" && content.contains("\"tool_result\"");
        if is_tool_result {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            // content is either a plain string or a ContentBlock[] — serialize both to text
            if let Some(s) = v.as_str() {
                n += (s.chars().count() / 4) as i64;
            } else if let Some(arr) = v.as_array() {
                for b in arr {
                    let t = serde_json::to_string(b).unwrap_or_default();
                    n += (t.chars().count() / 4) as i64;
                }
            }
        }
    }
    n
}

async fn cost_rows(db: &SqlitePool, session_id: &str) -> SessionCost {
    struct Row {
        model: String,
        input: i64,
        output: i64,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT model, COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0) FROM session_usage WHERE session_id=? GROUP BY model ORDER BY SUM(input_tokens)+SUM(output_tokens) DESC",
    )
    .bind(session_id)
    .fetch_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(model, input, output)| Row { model, input, output })
    .collect();

    let mut out_rows = Vec::new();
    let mut total = 0.0;
    for r in rows {
        let tokens = r.input + r.output;
        out_rows.push(CostRow {
            model: r.model.clone(),
            tokens: format_tokens(tokens),
            cost: {
                let c = model_cost_usd(&r.model, r.input as u64, r.output as u64);
                total += c;
                format!("${:.2}", c)
            },
        });
    }
    SessionCost {
        rows: out_rows,
        total: format!("${:.2}", total),
    }
}

fn format_tokens(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}m", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

/// Parse `.handsfree/plan.md` in the working dir into checklist items.
/// Default source for the Plan panel; plugins/harness can replace it later
/// via a set_plan_items command (see TODO.md).
async fn plan_from_file(working_dir: Option<&str>) -> SessionPlan {
    let Some(wd) = working_dir else { return SessionPlan::default() };
    let plan_path = Path::new(wd).join(".handsfree").join("plan.md");
    let text = match std::fs::read_to_string(&plan_path) {
        Ok(t) => t,
        Err(_) => return SessionPlan::default(),
    };
    let mut items: Vec<PlanItem> = Vec::new();
    let mut next_state = String::from("todo");
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("### Task ") {
            // "### Task 1.1: Bundle fonts" → label keeps the number: "1.1 — Bundle fonts"
            let label = rest.trim();
            items.push(PlanItem {
                label: label.to_string(),
                state: next_state.clone(),
            });
            next_state = String::from("todo");
        } else if trimmed.starts_with("Status:") {
            let st = trimmed.trim_start_matches("Status:")
                .trim()
                .to_lowercase();
            next_state = match st.as_str() {
                "done" => "done".to_string(),
                "active" => "active".to_string(),
                _ => "todo".to_string(),
            };
        } else if trimmed.starts_with("### Milestone") && !items.is_empty() {
            // Reset to todo at milestone boundaries (unless a task follows).
            next_state = String::from("todo");
        }
    }
    let done = items.iter().filter(|i| i.state == "done").count();
    let progress = if items.is_empty() {
        String::new()
    } else {
        format!("{done}/{}", items.len())
    };
    SessionPlan { items, progress }
}

/// Best-effort git working-tree snapshot for a working dir.
fn git_working_tree(working_dir: Option<&str>) -> GitWorkingTree {
    let Some(wd) = working_dir else { return GitWorkingTree::default() };
    if !Path::new(wd).join(".git").exists() {
        return GitWorkingTree::default();
    }
    let run = |args: &[&str]| -> Option<String> {
        let o = std::process::Command::new("git")
            .args(args)
            .current_dir(wd)
            .output()
            .ok()?;
        if !o.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
    };

    let branch = run(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let status = run(&["status", "--short"]).unwrap_or_default();
    let numstat = run(&["diff", "--numstat"]).unwrap_or_default();

    let mut added = 0i64;
    let mut removed = 0i64;
    let mut rows: Vec<GitRow> = Vec::new();
    let mut max_lines = 1i64;
    let mut parsed: Vec<(String, String, i64)> = Vec::new();
    for line in numstat.lines() {
        let mut it = line.split('\t');
        let (Some(a), Some(r), Some(path)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let a: i64 = a.trim().parse().unwrap_or(0);
        let r: i64 = r.trim().parse().unwrap_or(0);
        if a == 0 && r == 0 {
            continue;
        }
        added += a;
        removed += r;
        max_lines = max_lines.max(a + r);
        parsed.push((path.to_string(), format!("+{a} −{r}"), a + r));
    }
    for line in status.lines() {
        if line.len() < 3 {
            continue;
        }
        let mark = line[..2].trim().to_string();
        let path = line[3..].trim().to_string();
        if path.is_empty() {
            continue;
        }
        // Staged+unstaged collapse; match on path against numstat for weight.
        let weight = if let Some((_, _, sz)) = parsed.iter().find(|(p, _, _)| p == &path) {
            *sz as f32 / max_lines as f32
        } else {
            0.15 // binary / untracked → small bar
        };
        rows.push(GitRow { mark, path, weight });
    }
    GitWorkingTree {
        branch,
        added,
        removed,
        files: rows.len() as i64,
        rows,
    }
}

async fn touched_files(db: &SqlitePool, session_id: &str) -> Vec<TouchedFile> {
    sqlx::query_as::<_, (String, String, Option<i64>, Option<i64>)>(
        "SELECT path, kind, added, removed FROM touched_files WHERE session_id=? ORDER BY updated_at DESC, rowid DESC LIMIT 60",
    )
    .bind(session_id)
    .fetch_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(path, kind, added, removed)| TouchedFile { path, kind, added, removed })
    .collect()
}

/// Assemble the full sidebar stats for a session (context ring, cost, plan,
/// files, git). Cheap enough to rebuild on every `session:stats` push.
pub async fn build_session_stats(
    db: &SqlitePool,
    session_id: &str,
    working_dir: Option<&str>,
    context_limit: Option<i64>,
) -> SessionStats {
    let limit = context_limit.unwrap_or(200_000).max(1);
    let (in_tok, out_tok) = total_usage(db, session_id).await;
    let used = in_tok + out_tok;
    let conv = conversation_tokens(db, session_id).await;
    // System+skills: a fixed estimate of the harness system prompt (rewritten
    // per turn from the system content — the actual prompt is never persisted,
    // so this is the honest best estimate).
    let sys = 7_400i64;
    let files_ctx = (used - sys - conv).max(0);
    let percent = ((used as f64 / limit as f64) * 100.0).floor() as i64;

    SessionStats {
        session_id: session_id.to_string(),
        context: ContextUsage {
            segments: vec![
                ContextSegment { label: "System + skills".into(), tokens: sys, color: "var(--text-muted)".into() },
                ContextSegment { label: "Files in context".into(), tokens: files_ctx, color: "var(--warning)".into() },
                ContextSegment { label: "Conversation".into(), tokens: conv, color: "var(--accent)".into() },
            ],
            used,
            limit,
            percent,
        },
        cost: cost_rows(db, session_id).await,
        plan: plan_from_file(working_dir).await,
        files: touched_files(db, session_id).await,
        git: git_working_tree(working_dir),
    }
}

/// Emit the `session:stats` Tauri event so the sidebar updates live.
pub async fn emit_stats(
    app_handle: &tauri::AppHandle,
    db: &SqlitePool,
    session_id: &str,
    working_dir: Option<&str>,
) {
    let stats = build_session_stats(db, session_id, working_dir, None).await;
    let _ = app_handle.emit(&format!("session:stats:{session_id}"), &stats);
    let _ = app_handle.emit("session:stats", &stats);
}

/// Resolve a session's working dir + provider context window (fallback 200k).
async fn session_basics(
    db: &SqlitePool,
    session_id: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let row: Option<(Option<String>, String, String)> = sqlx::query_as(
        "SELECT working_dir, model, provider FROM sessions WHERE id=?",
    )
    .bind(session_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    match row {
        Some((wd, model, provider)) => (wd, Some(model), Some(provider)),
        None => (None, None, None),
    }
}

#[tauri::command]
pub async fn get_session_stats(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<SessionStats, String> {
    let (working_dir, model, provider) = session_basics(&state.db, &session_id).await;
    let limit = match (provider.as_deref(), model.as_deref()) {
        (Some(p_id), Some(m_id)) => state
            .registry
            .get(p_id)
            .and_then(|h| {
                h.info()
                    .models
                    .iter()
                    .find(|m| m.id == m_id)
                    .and_then(|m| m.context_window)
                    .map(|c| c as i64)
            })
            .unwrap_or(200_000),
        _ => 200_000,
    };
    Ok(build_session_stats(&state.db, &session_id, working_dir.as_deref(), Some(limit)).await)
}