//! Telemetry backing the right sidebar: context usage, session cost, plan
//! checklist, files touched and the git working tree. All data is derived
//! from the SQLite DB (session_usage / touched_files) plus live filesystem
//! state (`PLAN.md` in the working dir, `git status/diff`).

use crate::AppState;
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::Path;
use tauri::Emitter;
use tauri::Manager;

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
    pub kind: String,
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
#[derive(Default)]
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
#[derive(Default)]
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
        Self {
            rows: Vec::new(),
            total: "$0.00".into(),
        }
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
    system_tokens: i64,
    conversation_tokens: i64,
    files_tokens: i64,
    kind: &str,
) -> Result<(), String> {
    let cost = model_cost_usd(model, input_tokens, output_tokens);
    sqlx::query(
        "INSERT INTO session_usage (id, session_id, model, provider, input_tokens, output_tokens, cost_usd, system_tokens, conversation_tokens, files_tokens, kind) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(session_id)
    .bind(model)
    .bind(provider)
    .bind(input_tokens as i64)
    .bind(output_tokens as i64)
    .bind(cost)
    .bind(system_tokens)
    .bind(conversation_tokens)
    .bind(files_tokens)
    .bind(kind)
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
        "read_file" => (
            "read",
            input["path"].as_str().unwrap_or("").to_string(),
            None,
            None,
        ),
        "search_files" => (
            "read",
            input["pattern"].as_str().unwrap_or("").to_string(),
            None,
            None,
        ),
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

/// The context that would be sent on the NEXT call ≈ the input the provider
/// just counted for the most recent call (system prompt + full history).
/// Returns (input_tokens, output_tokens) of the newest AGENT-CALL row.
/// Using the provider's own count is exact — no local tokenizer needed, and
/// it reflects the true live context (unlike a cumulative SUM, which
/// re-counts the history on every turn and grows unbounded). Verification
/// rows (kind='verification') are excluded — they are reviewer calls with a
/// tiny, unrelated prompt and would corrupt the ring.
async fn last_call_tokens(db: &SqlitePool, session_id: &str) -> Option<(i64, i64)> {
    sqlx::query_as::<_, (i64, i64)>(
        "SELECT input_tokens, output_tokens FROM session_usage WHERE session_id=? AND kind != 'verification' ORDER BY rowid DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

/// Estimate the context breakdown from persisted messages (chars / 4 ≈
/// tokens). Returns (conversation, files):
///   conversation — user prose, assistant text/reasoning/thinking, and
///                  tool_use inputs (the model's own words + commands)
///   files       — tool_result payloads (read_file/search output, file
///                  contents carried back) — the bulk of "files in context"
/// These are estimates (the ring's exact sum comes from the provider's
/// last-call input_tokens); they just make the ring's composition honest.
/// Pure split of persisted message rows into (conversation, files) token
/// estimates — extracted from context_breakdown so it's unit-testable.
async fn context_breakdown(db: &SqlitePool, session_id: &str) -> (i64, i64) {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT role, content FROM messages WHERE session_id=?")
            .bind(session_id)
            .fetch_all(db)
            .await
            .unwrap_or_default();
    breakdown_from_rows(&rows)
}

fn breakdown_from_rows(rows: &[(String, String)]) -> (i64, i64) {
    let (mut conv, mut files) = (0i64, 0i64);
    for (_role, content) in rows {
        // content is either a plain string or a ContentBlock[] — serialize both to text
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(s) = v.as_str() {
                conv += (s.chars().count() / 4) as i64;
            } else if let Some(arr) = v.as_array() {
                for b in arr {
                    let t = serde_json::to_string(b).unwrap_or_default();
                    let n = (t.chars().count() / 4) as i64;
                    if b["type"].as_str() == Some("tool_result") {
                        files += n;
                    } else {
                        conv += n;
                    }
                }
            }
        }
    }
    (conv, files)
}

/// The real per-part prompt breakdown recorded by the agentic loop at call
/// time (chars/4 of what was actually sent): (system, conversation, files).
/// Legacy usage rows (pre-006, or sessions written before this feature) have
/// NULLs — the caller falls back to estimates for those. Verification rows
/// are excluded for the same reason as `last_call_tokens`.
async fn usage_breakdown(db: &SqlitePool, session_id: &str) -> Option<(i64, i64, i64)> {
    sqlx::query_as::<_, (i64, i64, i64)>(
        "SELECT system_tokens, conversation_tokens, files_tokens FROM session_usage WHERE session_id=? AND system_tokens IS NOT NULL AND kind != 'verification' ORDER BY rowid DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

async fn cost_rows(db: &SqlitePool, session_id: &str) -> SessionCost {
    struct Row {
        kind: String,
        model: String,
        input: i64,
        output: i64,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT kind, model, COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0) FROM session_usage WHERE session_id=? GROUP BY kind, model ORDER BY SUM(input_tokens)+SUM(output_tokens) DESC",
    )
    .bind(session_id)
    .fetch_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(kind, model, input, output)| Row { kind, model, input, output })
    .collect();

    let mut out_rows = Vec::new();
    let mut total = 0.0;
    // All verification usage (harness reviewer calls) collapses into ONE special
    // row — "Verification" — so the user sees its cost next to the model rows.
    let mut verify_in = 0i64;
    let mut verify_out = 0i64;
    let mut verify_cost = 0.0;
    for r in rows {
        if r.kind == "verification" {
            verify_in += r.input;
            verify_out += r.output;
            verify_cost += model_cost_usd(&r.model, r.input as u64, r.output as u64);
            continue;
        }
        let tokens = r.input + r.output;
        out_rows.push(CostRow {
            kind: "model".into(),
            model: r.model.clone(),
            tokens: format_tokens(tokens),
            cost: {
                let c = model_cost_usd(&r.model, r.input as u64, r.output as u64);
                total += c;
                format!("${:.2}", c)
            },
        });
    }
    if verify_in > 0 || verify_out > 0 {
        total += verify_cost;
        out_rows.push(CostRow {
            kind: "verification".into(),
            model: "Verification".into(),
            tokens: format_tokens(verify_in + verify_out),
            cost: format!("${:.2}", verify_cost),
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

/// Parse `PLAN.md` in the working dir into checklist items (tasks written as
/// `### Task …` with optional `Status: done|active` markers). The sidebar's
/// Plan section renders only when the file exists and has parseable tasks.
async fn plan_from_file(working_dir: Option<&str>) -> SessionPlan {
    let Some(wd) = working_dir else {
        return SessionPlan::default();
    };
    let plan_path = Path::new(wd).join("PLAN.md");
    let text = match std::fs::read_to_string(&plan_path) {
        Ok(t) => t,
        Err(_) => return SessionPlan::default(),
    };
    let mut items: Vec<PlanItem> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("### Task ") {
            // "### Task 1.1: Bundle fonts" → label keeps the number: "1.1 — Bundle fonts"
            items.push(PlanItem {
                label: rest.trim().to_string(),
                state: "todo".to_string(),
            });
        } else if trimmed.starts_with("Status:") {
            // A `Status:` line applies to the task it sits under (the most
            // recently parsed one), not the next.
            let st = trimmed.trim_start_matches("Status:").trim().to_lowercase();
            if let Some(last) = items.last_mut() {
                last.state = match st.as_str() {
                    "done" => "done".to_string(),
                    "active" => "active".to_string(),
                    _ => "todo".to_string(),
                };
            }
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
    let Some(wd) = working_dir else {
        return GitWorkingTree::default();
    };
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
    // A file can be recorded more than once (read → edited → …). Keep only
    // the NEWEST row per path — the sidebar lists each touched file once and
    // the frontend keys the list on `path` (duplicates would trip Svelte's
    // each_key_duplicate check). MAX(rowid) = most recent insert for a path.
    sqlx::query_as::<_, (String, String, Option<i64>, Option<i64>)>(
        "SELECT path, kind, added, removed FROM touched_files \
         WHERE session_id=? AND rowid IN (SELECT MAX(rowid) FROM touched_files WHERE session_id=? GROUP BY path) \
         ORDER BY updated_at DESC, rowid DESC LIMIT 60",
    )
    .bind(session_id)
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
///
/// Context semantics: `used` = the newest provider-counted input_tokens
/// (what the next call would send), NOT a cumulative lifetime sum. The three
/// segments come from the agentic loop's REAL recorded breakdown of what it
/// sent on that call (system prompt + tool schemas / conversation / tool
/// results — measured at request-build time and stored with the usage row);
/// only sessions with no recorded breakdown fall back to char-estimates.
/// Auto-compaction keys off `percent` crossing 100.
pub async fn build_session_stats(
    db: &SqlitePool,
    session_id: &str,
    working_dir: Option<&str>,
    context_limit: Option<i64>,
) -> SessionStats {
    let limit = context_limit.unwrap_or(200_000).max(1);
    // Live context = the last call the provider counted (system + full
    // history); a session that never ran has no provider count, so fall
    // back to the char-estimates (legacy sessions pre-telemetry).
    //
    // Segment source: the newest usage row carries the REAL per-part break-
    // down measured when that request was built (what we actually sent)
    // rather than a hardcoded system estimate. Legacy rows (no breakdown)
    // keep the old char-estimate path.
    let (sys, conv, files) = match usage_breakdown(db, session_id).await {
        Some((s, c, f)) => (s, c, f),
        None => {
            let (conv, files) = context_breakdown(db, session_id).await;
            (3_800i64, conv, files)
        }
    };
    let used = match last_call_tokens(db, session_id).await {
        Some((input, _)) => input,
        None => sys + conv + files,
    };
    let est_total = sys + conv + files;
    // The ring must visually equal used/limit: the recorded breakdown is a
    // chars/4 proxy (it can drift from the provider's exact tokenizer on JSON
    // overhead etc.), so scale it onto the exact provider-counted `used`.
    // Legacy fallback (used == est_total) keeps the estimates as-is.
    let segments = scale_to_used(est_total, used, &[sys, files, conv]);
    let percent = ((used as f64 / limit as f64) * 100.0).floor() as i64;

    SessionStats {
        session_id: session_id.to_string(),
        context: ContextUsage {
            segments: vec![
                ContextSegment {
                    label: "System + skills".into(),
                    tokens: segments[0],
                    color: "var(--text-muted)".into(),
                },
                ContextSegment {
                    label: "Files in context".into(),
                    tokens: segments[1],
                    color: "var(--warning)".into(),
                },
                ContextSegment {
                    label: "Conversation".into(),
                    tokens: segments[2],
                    color: "var(--accent)".into(),
                },
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

/// Scale token estimates onto the exact provider-counted `used` so the
/// segments always sum to it (the ring's filled fraction then equals
/// percent exactly). Keeps the estate composition ratios; legacy fallback
/// passes through unchanged when `used` == `est_total`.
fn scale_to_used(est_total: i64, used: i64, tokens: &[i64]) -> Vec<i64> {
    if est_total <= 0 || used <= 0 {
        return tokens.iter().map(|_| 0).collect();
    }
    let s = used as f64 / est_total as f64;
    tokens
        .iter()
        .map(|t| ((*t as f64) * s).round() as i64)
        .collect()
}

/// Resolve a session's provider context window from the registry (fallback
/// 200k) — shared by get_session_stats AND emit_stats so a live push and a
/// direct load agree on the limit (the ring must not change its scale
/// mid-run).
async fn context_limit_for(
    db: &SqlitePool,
    registry: &crate::providers::ProviderRegistry,
    session_id: &str,
) -> i64 {
    let (_, model, provider) = session_basics(db, session_id).await;
    match (provider.as_deref(), model.as_deref()) {
        (Some(p_id), Some(m_id)) => registry
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
    }
}

/// Emit the `session:stats` Tauri event so the sidebar updates live.
pub async fn emit_stats(
    app_handle: &tauri::AppHandle,
    db: &SqlitePool,
    session_id: &str,
    working_dir: Option<&str>,
) {
    let limit = match app_handle.try_state::<crate::AppState>() {
        Some(s) => context_limit_for(db, &s.registry, session_id).await,
        None => 200_000,
    };
    let stats = build_session_stats(db, session_id, working_dir, Some(limit)).await;
    let _ = app_handle.emit(&format!("session:stats:{session_id}"), &stats);
    let _ = app_handle.emit("session:stats", &stats);
}

/// Resolve a session's working dir + provider context window (fallback 200k).
async fn session_basics(
    db: &SqlitePool,
    session_id: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let row: Option<(Option<String>, String, String)> =
        sqlx::query_as("SELECT working_dir, model, provider FROM sessions WHERE id=?")
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
    let (working_dir, _, _) = session_basics(&state.db, &session_id).await;
    let limit = context_limit_for(&state.db, &state.registry, &session_id).await;
    Ok(build_session_stats(&state.db, &session_id, working_dir.as_deref(), Some(limit)).await)
}
#[cfg(test)]
mod tests {
    use super::{
        breakdown_from_rows, build_session_stats, cost_rows, plan_from_file, scale_to_used,
        touched_files, usage_breakdown,
    };

    #[tokio::test]
    async fn touched_files_dedupes_per_path_keeping_newest() {
        let pool = crate::db::init_db(":memory:").await.unwrap();
        sqlx::query("INSERT INTO sessions (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind("s1")
            .bind("test")
            .bind("2026-01-01T00:00:00Z")
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
        // Same file touched twice: read first, then edited. The edited row is
        // inserted later → highest rowid → must win.
        for (kind, added, removed) in [
            ("read", None::<i64>, None::<i64>),
            ("edited", Some(5), Some(2)),
        ] {
            sqlx::query(
                "INSERT INTO touched_files (id, session_id, path, kind, added, removed) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind("s1")
            .bind("/mnt/Projects/frontend-tests/README.md")
            .bind(kind)
            .bind(added)
            .bind(removed)
            .execute(&pool)
            .await
            .unwrap();
        }
        let files = touched_files(&pool, "s1").await;
        assert_eq!(files.len(), 1, "one row per path — no duplicate keys");
        assert_eq!(files[0].path, "/mnt/Projects/frontend-tests/README.md");
        assert_eq!(files[0].kind, "edited", "newest row wins");
        assert_eq!(files[0].added, Some(5));
        // Different paths are untouched by the dedup.
        sqlx::query("INSERT INTO touched_files (id, session_id, path, kind) VALUES (?, ?, ?, ?)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind("s1")
            .bind("src/lib/types.ts")
            .bind("read")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(touched_files(&pool, "s1").await.len(), 2);
    }

    #[tokio::test]
    async fn plan_reads_root_plan_md_and_degrades_when_absent() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        // No PLAN.md → empty plan (sidebar hides the section).
        let empty = plan_from_file(Some(dir.path().to_str().unwrap())).await;
        assert!(empty.items.is_empty());
        assert!(empty.progress.is_empty());
        // None at all (no working dir) → empty too.
        assert!(plan_from_file(None).await.items.is_empty());
        // Root PLAN.md → tasks parsed with statuses, progress = done/total.
        fs::write(
            dir.path().join("PLAN.md"),
            "# Plan\n\n## Milestone 1\n\n### Task 1.1: Bundle fonts\nStatus: done\n\n### Task 1.2: Wire the theme\nStatus: active\n\n### Task 1.3: Ship it\nStatus: pending\n",
        )
        .unwrap();
        let plan = plan_from_file(Some(dir.path().to_str().unwrap())).await;
        assert_eq!(plan.items.len(), 3);
        assert_eq!(plan.items[0].label, "1.1: Bundle fonts");
        assert_eq!(plan.items[0].state, "done");
        assert_eq!(plan.items[1].state, "active");
        assert_eq!(plan.items[2].state, "todo");
        assert_eq!(plan.progress, "1/3");
    }

    #[test]
    fn scale_maps_estimates_onto_exact_used() {
        // Used 2k of a window while estimates claim 41k → every segment
        // shrinks so the ring's filled fraction equals used/limit exactly.
        let scaled = scale_to_used(41_000, 2_000, &[7_400, 28_000, 5_600]);
        assert_eq!(scaled.iter().sum::<i64>(), 2_000);
        let per = scaled.iter().map(|t| t * 1000 / 2000).collect::<Vec<_>>();
        // Ratios preserved: sys ≈ 18% of estimates, stays ≈18% of used.
        assert!((per[0] - 180).abs() < 20, "sys share drifted: {per:?}");
    }

    #[test]
    fn scale_is_identity_for_legacy_fallback() {
        // No usage recorded → used == estimates → pass-through.
        let scaled = scale_to_used(41_000, 41_000, &[7_400, 28_000, 5_600]);
        assert_eq!(scaled, vec![7_400, 28_000, 5_600]);
    }

    #[test]
    fn scale_guards_zero() {
        assert_eq!(scale_to_used(0, 5_000, &[100, 200]), vec![0, 0]);
        assert_eq!(scale_to_used(5_000, 0, &[100, 200]), vec![0, 0]);
    }

    #[test]
    fn breakdown_counts_tool_results_as_files_only() {
        let rows = vec![
            ("user".to_string(), serde_json::json!("a short prompt").to_string()),
            (
                "user".to_string(),
                serde_json::json!([
                    { "type": "tool_result", "tool_use_id": "x", "content": [{ "type": "text", "text": "this is a big file payload with lots and lots of characters to count toward files — file content read back into context is far longer than any single chat turn in this test" }] }
                ]).to_string(),
            ),
            (
                "assistant".to_string(),
                serde_json::json!([
                    { "type": "reasoning", "thinking": "brief" },
                    { "type": "text", "text": "short answer" },
                    { "type": "tool_use", "id": "x", "name": "read_file", "input": { "path": "a.txt" } }
                ]).to_string(),
            ),
        ];
        let (conv, files) = breakdown_from_rows(&rows);
        assert!(conv > 0, "conversation must be counted: {conv}");
        assert!(files > 0, "tool_result payload must land in files: {files}");
        // The tool_result payload is the longest single block — make sure the
        // bulk lands in files, not conversation.
        assert!(
            files > conv,
            "files payload should dominate: files={files} conv={conv}"
        );
    }

    #[test]
    fn breakdown_plain_string_messages_are_conversation() {
        let rows = vec![
            (
                "user".to_string(),
                "hello world this is a plain user message".to_string(),
            ),
            (
                "assistant".to_string(),
                serde_json::json!([{ "type": "text", "text": "hi back" }]).to_string(),
            ),
        ];
        let (conv, files) = breakdown_from_rows(&rows);
        assert!(conv > 0 && files == 0, "conv={conv} files={files}");
    }

    #[test]
    fn breakdown_empty_and_garbage_rows_are_zero() {
        let rows = vec![
            ("user".to_string(), String::new()),
            ("assistant".to_string(), "not json".to_string()),
            ("user".to_string(), serde_json::json!(null).to_string()),
        ];
        let (conv, files) = breakdown_from_rows(&rows);
        assert_eq!((conv, files), (0, 0));
    }

    #[tokio::test]
    async fn recorded_breakdown_wins_over_estimates() {
        let pool = crate::db::init_db(":memory:").await.unwrap();
        sqlx::query("INSERT INTO sessions (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind("s1")
            .bind("test")
            .bind("2026-01-01T00:00:00Z")
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
        // Legacy usage row (no breakdown) then a modern one with the real
        // per-part measurement recorded by the agentic loop.
        sqlx::query(
            "INSERT INTO session_usage (id, session_id, model, provider, input_tokens, output_tokens, cost_usd) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind("s1")
        .bind("m")
        .bind("p")
        .bind(1_000i64)
        .bind(10i64)
        .bind(0f64)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_usage (id, session_id, model, provider, input_tokens, output_tokens, cost_usd, system_tokens, conversation_tokens, files_tokens) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind("s1")
        .bind("m")
        .bind("p")
        .bind(4_200i64)
        .bind(50i64)
        .bind(0f64)
        .bind(4_100i64)
        .bind(0i64)
        .bind(99i64)
        .execute(&pool)
        .await
        .unwrap();

        let parts = usage_breakdown(&pool, "s1").await;
        assert_eq!(parts, Some((4_100, 0, 99)), "newest recorded row wins");

        // build_session_stats must show the RECORDED system bucket (4.1k),
        // not the hardcoded 3.8k estimate, and used = the provider's count.
        let stats = build_session_stats(&pool, "s1", None, Some(100_000)).await;
        assert_eq!(stats.context.used, 4_200);
        // Recorded system bucket (4.1k) wins over the hardcoded 3.8k
        // estimate; the ≤1-token scale drift onto used is rounding.
        assert!((stats.context.segments[0].tokens - 4_100).abs() <= 2);
        assert!((stats.context.segments[1].tokens - 99).abs() <= 2); // files
        assert_eq!(stats.context.segments[2].tokens, 0); // conversation
        assert_eq!(stats.context.percent, 4);
    }

    #[tokio::test]
    async fn verification_rows_stay_out_of_context_but_show_in_cost() {
        let pool = crate::db::init_db(":memory:").await.unwrap();
        sqlx::query("INSERT INTO sessions (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind("s1")
            .bind("test")
            .bind("2026-01-01T00:00:00Z")
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
        // One agent call, then a verification (reviewer) call recorded AFTER it
        // (the real order at loop-done) — the ring must ignore the latter.
        sqlx::query(
            "INSERT INTO session_usage (id, session_id, model, provider, input_tokens, output_tokens, cost_usd, system_tokens, conversation_tokens, files_tokens, kind) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind("s1")
        .bind("m")
        .bind("p")
        .bind(10_000i64)
        .bind(200i64)
        .bind(0f64)
        .bind(4_100i64)
        .bind(5_000i64)
        .bind(900i64)
        .bind("agent")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_usage (id, session_id, model, provider, input_tokens, output_tokens, cost_usd, system_tokens, conversation_tokens, files_tokens, kind) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind("s1")
        .bind("m")
        .bind("p")
        .bind(2_000i64)
        .bind(100i64)
        .bind(0f64)
        .bind(0i64)
        .bind(0i64)
        .bind(0i64)
        .bind("verification")
        .execute(&pool)
        .await
        .unwrap();

        // Context ring: the newest row is the verification one, but the ring
        // must still reflect the agent call.
        let stats = build_session_stats(&pool, "s1", None, Some(100_000)).await;
        assert_eq!(
            stats.context.used, 10_000,
            "verification must not skew the ring"
        );

        // Cost: verification shows as its own special row, model rows intact.
        let cost = cost_rows(&pool, "s1").await;
        assert_eq!(cost.rows.len(), 2);
        assert_eq!(cost.rows[0].kind, "model");
        assert_eq!(cost.rows[0].model, "m");
        assert_eq!(cost.rows[1].kind, "verification");
        assert_eq!(cost.rows[1].model, "Verification");
        assert_eq!(cost.rows[1].tokens, "2.1k");
        assert!(cost.total.starts_with("$"));
    }

    #[tokio::test]
    async fn no_usage_falls_back_to_estimates() {
        let pool = crate::db::init_db(":memory:").await.unwrap();
        sqlx::query("INSERT INTO sessions (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind("s2")
            .bind("legacy")
            .bind("2026-01-01T00:00:00Z")
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
        // A session with messages but NO usage rows (pre-telemetry): the
        // estimate path (system 3.8k + char-estimates) must kick in.
        sqlx::query("INSERT INTO messages (id, session_id, role, content, timestamp) VALUES (?, ?, ?, ?, ?)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind("s2")
            .bind("user")
            .bind(serde_json::json!("a fairly long user prompt with some words to count").to_string())
            .bind("2026-01-01T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
        let stats = build_session_stats(&pool, "s2", None, Some(100_000)).await;
        assert_eq!(stats.context.segments[0].tokens, 3_800); // estimate fallback
        assert_eq!(
            stats.context.used,
            3_800 + stats.context.segments[1].tokens + stats.context.segments[2].tokens
        );
    }
}
