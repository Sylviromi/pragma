use crate::AppState;

#[tauri::command]
pub async fn list_providers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::providers::types::ProviderInfo>, String> {
    Ok(state.registry.list_infos())
}

#[tauri::command]
pub async fn save_api_key(
    provider: String,
    key: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    {
        state.settings.write().unwrap().api_keys.insert(provider.clone(), key);
    }
    crate::save_settings(&state);
    // Fetch fresh model lists for this provider now that the key changed.
    state.registry.trigger_discovery(&provider, &app);
    Ok(())
}

#[tauri::command]
pub async fn get_api_keys(
    state: tauri::State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    Ok(state.settings.read().unwrap().api_keys.clone())
}
