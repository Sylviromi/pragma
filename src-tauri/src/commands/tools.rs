use crate::AppState;

/// Returns the tool_use_ids that are currently blocked waiting for frontend approval.
/// Used by the frontend after HMR reloads to reconcile stale tool card states.
#[tauri::command]
pub async fn get_pending_approvals(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(state.pending_approvals.lock().await.keys().cloned().collect())
}

#[tauri::command]
pub async fn approve_tool(
    tool_use_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut pending = state.pending_approvals.lock().await;
    if let Some(tx) = pending.remove(&tool_use_id) {
        tx.send(true).ok();
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
    }
    Ok(())
}