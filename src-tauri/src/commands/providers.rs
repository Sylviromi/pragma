use crate::AppState;

#[tauri::command]
pub async fn list_providers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::providers::types::ProviderInfo>, String> {
    Ok(state.registry.list_infos())
}

/// Replace a provider's API key list (rotation order — first is primary).
/// Empty keys are dropped; an empty list removes the provider's entry.
#[tauri::command]
pub async fn save_api_keys(
    provider: String,
    keys: Vec<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let cleaned: Vec<String> = keys
        .into_iter()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect();
    {
        let mut settings = state.settings.write().unwrap();
        if cleaned.is_empty() {
            settings.api_keys.remove(&provider);
        } else {
            settings.api_keys.insert(provider.clone(), cleaned);
        }
    }
    crate::save_settings(&state);
    // Fetch fresh model lists for this provider now that the key changed.
    state.registry.trigger_discovery(&provider, &app);
    Ok(())
}

#[tauri::command]
pub async fn get_api_keys(
    state: tauri::State<'_, AppState>,
) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    Ok(state.settings.read().unwrap().api_keys.clone())
}
