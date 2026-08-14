use crate::AppState;

/// Returns the tool_use_ids that are currently blocked waiting for frontend approval.
/// Used by the frontend after HMR reloads to reconcile stale tool card states.
#[tauri::command]
pub async fn get_pending_approvals(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    Ok(state
        .pending_approvals
        .lock()
        .await
        .keys()
        .cloned()
        .collect())
}

#[tauri::command]
pub async fn approve_tool(
    tool_use_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut pending = state.pending_approvals.lock().await;
    if let Some(tx) = pending.remove(&tool_use_id) {
        tx.send(true).ok();
    } else {
        // The approval arrived before the loop registered the pending entry
        // (the frontend auto-approves SAFE tools the moment it sees the
        // tool_call event). Remember it so the loop's later wait resolves
        // immediately instead of hanging forever.
        drop(pending);
        state.pre_approved.lock().await.insert(tool_use_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn deny_tool(
    tool_use_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut pending = state.pending_approvals.lock().await;
    if let Some(tx) = pending.remove(&tool_use_id) {
        tx.send(false).ok();
    } else {
        drop(pending);
        state.pre_denied.lock().await.insert(tool_use_id);
    }
    Ok(())
}

/// Resolve a pending `ask_question` from the frontend. `answer` is the chosen
/// option text (or None to let the loop continue without an answer).
#[tauri::command]
pub async fn answer_question(
    question_id: String,
    answer: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut channels = state.question_channels.lock().await;
    if let Some(tx) = channels.remove(&question_id) {
        tx.send(answer).ok();
    }
    Ok(())
}
