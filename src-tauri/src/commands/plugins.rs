use crate::AppState;

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
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.plugins.set_disabled(&id, !enabled);

    // Rebinding all plugin providers keeps register/unregister idempotent.
    let api_keys = state.settings.read().unwrap().api_keys.clone();
    crate::plugins::provider::unregister_plugin_providers(&state.registry, &state.plugins);
    crate::plugins::provider::register_plugin_providers(&state.registry, &state.plugins, &api_keys);

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