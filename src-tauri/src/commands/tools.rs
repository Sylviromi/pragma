use crate::AppState;

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