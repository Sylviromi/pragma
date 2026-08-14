//! Harness verification orchestrator — called from the agentic loop at the
//! Done transition (when the model returns a final response with no tool
//! calls). Runs the project's checks deterministically, then (if checks pass)
//! one small asymmetric LLM review, and records the reviewer's usage so its
//! cost shows up in the sidebar as a special row.
//!
//! Independence guarantees (see tools/verify.rs): the reviewer sees only the
//! task + diff + check output — never the conversation. Reviewer messages are
//! NEVER persisted to `messages`, so the asymmetry survives. (The *outcome*
//! IS persisted, as a `role='verification'` message the model never sees.)

use crate::providers::types::{AgentEvent, ContentBlock, ThinkingPref};
use crate::tools::verify as v;
use crate::AppState;
use std::path::PathBuf;
use std::time::Instant;
use tauri::Emitter;

/// Outcome of one verification pass. The loop decides what to do with it
/// (accept / feed back / surface) and persists a `role='verification'`
/// message with all fields the UI bubble shows.
pub struct VerificationOutcome {
    pub passed: bool,
    /// User pressed Esc during verification — distinct from failure (the
    /// loop must not feed a fix round back on a cancel).
    pub cancelled: bool,
    /// Human-readable report — shown in the UI card and (on failure) fed back
    /// to the agent as a targeted user turn.
    pub summary: String,
    /// Specific findings (file:line where the reviewer gave them).
    pub findings: Vec<String>,
    /// Reviewer input+output tokens (0 when review was skipped / cancelled).
    pub tokens: u64,
    /// Wall-clock time of the whole phase.
    pub duration_ms: u64,
    /// Label of the checks command that ran ("cargo check + test", …).
    pub checks_label: Option<String>,
    pub checks_ran: bool,
    pub review_used: bool,
}

fn emit(app: &tauri::AppHandle, session_id: &str, ev: AgentEvent) {
    let _ = app.emit(&format!("agent:{session_id}"), ev);
}

/// Run one verification pass. Never blocks the user's turn fatally: any
/// failure to run the machinery itself (no provider, network error, …) falls
/// back to `passed = true` so the loop completes normally.
///
/// `has_wd` gates the whole phase: a session with no working directory has
/// nothing to check or review (path tools were blocked all run).
/// `task` is the user's message for this run.
#[allow(clippy::too_many_arguments)]
pub async fn run_verification(
    settings: &crate::VerificationSettings,
    state: &tauri::State<'_, AppState>,
    app_handle: &tauri::AppHandle,
    session_id: &str,
    working_dir: &PathBuf,
    has_wd: bool,
    provider: &str,
    session_model: &str,
    keys: &[String],
    task: &str,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> VerificationOutcome {
    let started = Instant::now();

    if !has_wd {
        return finish(
            started,
            true,
            false,
            "No working directory — nothing to verify.".into(),
            Vec::new(),
            0,
            None,
            false,
            false,
        );
    }
    let wd = working_dir.as_path();

    // 1. Deterministic checks (no LLM). Settings override wins; otherwise
    //    auto-detect from the project manifest.
    let mut checks_ran = false;
    let mut checks_ok = true;
    let mut checks_label: Option<String> = None;
    let mut checks_output = "(checks skipped — nothing to run)".to_string();

    let check_cmd = settings
        .test_command
        .clone()
        .map(|c| ("custom".to_string(), c))
        .or_else(|| v::infer_test_command(wd));

    if settings.run_checks {
        if let Some((label, cmd)) = &check_cmd {
            emit(
                app_handle,
                session_id,
                AgentEvent::VerificationStarted {
                    phase: "checks".into(),
                },
            );
            checks_ran = true;
            checks_label = Some(label.clone());
            let (ok, out) = v::run_command_status(wd, cmd, v::CHECK_TIMEOUT_SECS);
            checks_ok = ok;
            checks_output = out;
        }
    }

    // 2. Diff for the reviewer.
    let (diff, _files) = v::capture_diff(wd);

    // Checks failed → short-circuit (DARC-style: diagnose first, don't spend
    // review budget on a build that doesn't compile). Review only runs when
    // checks pass AND the independent review is enabled AND there is
    // something to review (diff or checks that ran).
    if !checks_ok {
        let summary = format!(
            "Project checks failed — {}:\n{}",
            checks_label.as_deref().unwrap_or("checks"),
            v::cap_chars(&checks_output, 2_000)
        );
        return finish(
            started,
            false,
            false,
            summary,
            Vec::new(),
            0,
            checks_label,
            checks_ran,
            false,
        );
    }

    let review_enabled = settings.independent_review
        && (!diff.trim().is_empty() || checks_ran);
    if !review_enabled {
        let summary = if checks_ran {
            format!(
                "Checks passed ({}). No independent review configured or nothing to review.",
                checks_label.as_deref().unwrap_or("checks")
            )
        } else {
            "No checks to run and no diff to review — nothing to verify.".to_string()
        };
        return finish(
            started,
            true,
            false,
            summary,
            Vec::new(),
            0,
            checks_label,
            checks_ran,
            false,
        );
    }

    // 3. Asymmetric review — one fresh, minimal model call.
    emit(
        app_handle,
        session_id,
        AgentEvent::VerificationStarted {
            phase: "review".into(),
        },
    );

    // Reviewer model: settings override, validated against the provider's
    // model list; fall back to the session model (fresh context is what
    // matters, not a different model).
    let reviewer_model = match &settings.reviewer_model {
        Some(m) => {
            let known = state
                .registry
                .get(provider)
                .map(|h| h.info().models.iter().any(|x| &x.id == m))
                .unwrap_or(false);
            if known { m.as_str() } else { session_model }
        }
        None => session_model,
    };
    let provider_handle = match state.registry.get(provider) {
        Some(h) => h.clone(),
        None => {
            eprintln!("[verify] unknown provider '{provider}' — skipping review");
            return finish(
                started,
                true,
                false,
                "Checks passed; review skipped (unknown provider).".into(),
                Vec::new(),
                0,
                checks_label,
                checks_ran,
                false,
            );
        }
    };
    let api_key = keys.first().map(|s| s.as_str()).unwrap_or("");
    let review_messages = v::build_review_messages(task, &diff, &checks_output);

    let review_result = {
        let deadline = cancel.changed();
        tokio::pin!(deadline);
        tokio::select! {
            res = provider_handle.call(
                &review_messages,
                reviewer_model,
                api_key,
                &[],
                ThinkingPref::Off,
                app_handle,
                session_id,
            ) => res,
            _ = &mut deadline => {
                // User pressed Esc during verification — don't loop further.
                return finish(
                    started,
                    true,
                    true,
                    "Verification stopped by user.".into(),
                    Vec::new(),
                    0,
                    checks_label,
                    checks_ran,
                    false,
                );
            }
        }
    };

    let (blocks, usage) = match review_result {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[verify] reviewer call failed: {e}");
            // Machinery failure → don't block the turn; report as passed with
            // a note (the checks already passed, which is the hard evidence).
            return finish(
                started,
                true,
                false,
                format!("Checks passed; review unavailable ({e})."),
                Vec::new(),
                0,
                checks_label,
                checks_ran,
                false,
            );
        }
    };

    // Record the reviewer's usage so its cost shows in the sidebar.
    let _ = crate::commands::stats::record_usage(
        &state.db,
        session_id,
        reviewer_model,
        provider,
        usage.input_tokens,
        usage.output_tokens,
        0,
        0,
        0,
        "verification",
    )
    .await;
    let tokens = usage.input_tokens + usage.output_tokens;

    let text: String = blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let (passed, findings) = v::parse_verdict(&text);
    let summary = if passed {
        "Independent review: change accepted.".to_string()
    } else {
        let mut s = "Independent review found issues:\n".to_string();
        for f in &findings {
            s.push_str(&format!("- {f}\n"));
        }
        s
    };

    finish(
        started,
        passed,
        false,
        summary,
        findings,
        tokens,
        checks_label,
        checks_ran,
        true,
    )
}

/// Assemble the outcome, stamping the wall-clock duration.
#[allow(clippy::too_many_arguments)]
fn finish(
    started: Instant,
    passed: bool,
    cancelled: bool,
    summary: String,
    findings: Vec<String>,
    tokens: u64,
    checks_label: Option<String>,
    checks_ran: bool,
    review_used: bool,
) -> VerificationOutcome {
    VerificationOutcome {
        passed,
        cancelled,
        summary,
        findings,
        tokens,
        duration_ms: started.elapsed().as_millis() as u64,
        checks_label,
        checks_ran,
        review_used,
    }
}
