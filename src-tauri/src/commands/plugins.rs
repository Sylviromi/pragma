use crate::AppState;
use tauri::Emitter;

#[tauri::command]
pub async fn list_plugins(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::plugins::PluginInfo>, String> {
    Ok(state.plugins.list())
}

#[tauri::command]
pub async fn set_plugin_enabled(
    id: String,
    enabled: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.plugins.set_disabled(&id, !enabled);

    // Rebinding all plugin providers keeps register/unregister idempotent.
    let api_keys = state.settings.read().unwrap().api_keys.clone();
    crate::plugins::provider::unregister_plugin_providers(&state.registry, &state.plugins);
    crate::plugins::provider::register_plugin_providers(&state.registry, &state.plugins, &api_keys, &app);

    crate::save_settings(&state);
    Ok(())
}

#[tauri::command]
pub async fn read_plugin_component(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    state.plugins.component_code(&id)
}

#[tauri::command]
pub async fn read_plugin_skill(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    state.plugins.skill_md(&id)
}

#[tauri::command]
pub async fn open_plugin_folder(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let dir = state.plugins.plugin_dir(&id);
    if !dir.exists() {
        return Err(format!("plugin folder missing: {}", dir.display()));
    }
    let path = dir.display().to_string();
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
    }
    Ok(())
}

// ── Plugin config storage ────────────────────────────────────────────────────

/// Read a plugin's config.json. Returns `{}` if no config file exists.
#[tauri::command]
pub async fn get_plugin_config(
    plugin_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let dir = state.plugins.plugin_dir(&plugin_id);
    let path = dir.join("config.json");
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("invalid config.json: {e}"))
}

/// Write a plugin's config.json (full overwrite — frontend merges before calling).
#[tauri::command]
pub async fn set_plugin_config(
    plugin_id: String,
    config: serde_json::Value,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let dir = state.plugins.plugin_dir(&plugin_id);
    if !dir.exists() {
        return Err(format!("plugin '{plugin_id}' directory not found"));
    }
    let path = dir.join("config.json");
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

// ── Plugin confirmation dialog ───────────────────────────────────────────────

/// Request user confirmation via a modal popup. Blocks until the user responds
/// or a 60-second timeout elapses (returns false on timeout).
#[tauri::command]
pub async fn request_plugin_confirmation(
    plugin_id: String,
    title: String,
    message: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();

    state
        .plugin_confirmations
        .lock()
        .await
        .insert(request_id.clone(), tx);

    let _ = app.emit(
        "plugin:confirm-request",
        serde_json::json!({
            "requestId": request_id,
            "pluginId": plugin_id,
            "title": title,
            "message": message,
        }),
    );

    // Wait for response with timeout
    match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
        Ok(Ok(accepted)) => Ok(accepted),
        _ => {
            // Timeout or channel dropped — clean up and return false
            state
                .plugin_confirmations
                .lock()
                .await
                .remove(&request_id);
            Ok(false)
        }
    }
}

/// Frontend calls this after the user clicks Accept/Dismiss on a plugin confirmation modal.
#[tauri::command]
pub async fn respond_plugin_confirmation(
    request_id: String,
    accepted: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let sender = state
        .plugin_confirmations
        .lock()
        .await
        .remove(&request_id);

    if let Some(tx) = sender {
        let _ = tx.send(accepted);
    }
    Ok(())
}