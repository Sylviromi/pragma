//! OS-level notifications via tauri-plugin-notification. Fired when the agent
//! needs attention (question, approval), finishes its work, or errors — but
//! only when the main window is NOT focused, so focused work is never
//! interrupted. Global on/off lives in AppState.settings (notify:enabled).

use crate::AppState;
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

/// Send an OS notification if the user is not currently looking at the app
/// and notifications are enabled. Best-effort: failures are logged, never
/// fatal.
pub fn notify(app_handle: &tauri::AppHandle, title: &str, body: &str) {
    // Respect the user's notification preference.
    let enabled = app_handle
        .try_state::<AppState>()
        .map(|st| st.settings.read().unwrap().notifications_enabled)
        .unwrap_or(true);
    if !enabled {
        return;
    }
    // Only notify when the window is unfocused (minimized/background/other app).
    let focused = app_handle
        .get_webview_window("main")
        .map(|w| w.is_focused().unwrap_or(false))
        .unwrap_or(false);
    if focused {
        return;
    }
    let res = app_handle
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
    if let Err(e) = res {
        eprintln!("[notify] failed to show notification: {e}");
    }
}

/// Set whether OS notifications are enabled (persisted to settings.json).
/// Lives here (module path) rather than the crate root — Tauri's
/// generate_handler! collides on bare-name commands defined in the root module.
#[tauri::command]
pub async fn set_notifications_enabled(
    enabled: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.settings.write().unwrap().notifications_enabled = enabled;
    crate::save_settings(&state);
    Ok(())
}

/// Current notification preference (settings page initial state).
#[tauri::command]
pub async fn get_notifications_enabled(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    Ok(state.settings.read().unwrap().notifications_enabled)
}