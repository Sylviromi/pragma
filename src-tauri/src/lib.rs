use sqlx::SqlitePool;
use tauri::{Emitter, Manager};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

mod db;
mod commands;
mod plugins;
mod providers;
mod tools;

#[derive(Default)]
pub struct Settings {
    pub api_keys: HashMap<String, String>,
    pub disabled_plugins: Vec<String>,
}

pub struct AppState {
    pub db: SqlitePool,
    pub registry: providers::ProviderRegistry,
    pub settings: RwLock<Settings>,
    pub settings_path: std::path::PathBuf,
    pub plugins: plugins::PluginManager,
    // pending tool approvals: tool_use_id → oneshot sender
    pub pending_approvals: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir().expect("no config dir");
            std::fs::create_dir_all(&config_dir).ok();
            let plugins_dir = config_dir.join("plugins");
            std::fs::create_dir_all(&plugins_dir).ok();

            let db_path = format!("sqlite:{}", config_dir.join("sessions.db").display());
            let rt = tokio::runtime::Runtime::new().unwrap();
            let pool = rt.block_on(db::init_db(&db_path)).expect("db init failed");

            let settings_path = config_dir.join("settings.json");
            let settings = load_settings(&settings_path);

            let registry = providers::ProviderRegistry::default();
            registry.register(providers::anthropic::AnthropicProvider);
            registry.register(providers::openai_compat::deepseek());
            registry.register(providers::openai_compat::opencode_local());

            let plugin_manager =
                plugins::PluginManager::new(plugins_dir.clone(), settings.disabled_plugins.clone());
            let wrote_demo = plugin_manager.ensure_demo_plugin();
            if wrote_demo {
                eprintln!("[plugins] wrote demo plugin into {}", plugins_dir.display());
            }
            plugins::provider::register_plugin_providers(&registry, &plugin_manager, &settings.api_keys);

            app.manage(AppState {
                db: pool,
                registry,
                settings: RwLock::new(settings),
                settings_path,
                pending_approvals: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                plugins: plugin_manager,
            });

            // Hot-reload watcher: rebind providers + tell the frontend to re-import.
            let app_handle = app.handle().clone();
            plugins::watcher::spawn_plugin_watcher(&plugins_dir, move || {
                let handle = app_handle.clone();
                std::thread::spawn(move || {
                    let state = handle.state::<AppState>();
                    let api_keys = state.settings.read().unwrap().api_keys.clone();
                    plugins::provider::unregister_plugin_providers(&state.registry, &state.plugins);
                    plugins::provider::register_plugin_providers(&state.registry, &state.plugins, &api_keys);
                    let _ = handle.emit(
                        "plugin:reload",
                        serde_json::json!({ "version": chrono::Utc::now().timestamp_millis() }),
                    );
                });
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::sessions::create_session,
            commands::sessions::list_sessions,
            commands::sessions::get_session,
            commands::sessions::update_session,
            commands::sessions::archive_session,
            commands::messages::add_message,
            commands::messages::list_messages,
            commands::providers::list_providers,
            commands::providers::run_agentic_loop,
            commands::tools::approve_tool,
            commands::tools::deny_tool,
            commands::providers::save_api_key,
            commands::providers::get_api_keys,
            commands::plugins::list_plugins,
            commands::plugins::set_plugin_enabled,
            commands::plugins::read_plugin_component,
            commands::plugins::read_plugin_skill,
            commands::plugins::open_plugin_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Persist settings.json (api_keys + disabled_plugins).
pub fn save_settings(state: &AppState) {
    let settings = state.settings.read().unwrap();
    let json = serde_json::json!({
        "api_keys": settings.api_keys,
        "disabled_plugins": settings.disabled_plugins,
    });
    let _ = std::fs::write(
        &state.settings_path,
        serde_json::to_string_pretty(&json).unwrap(),
    );
}

fn load_settings(path: &std::path::Path) -> Settings {
    if !path.exists() {
        return Settings::default();
    }
    let text = std::fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| {
            let keys = v["api_keys"]
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                        .collect()
                })
                .unwrap_or_default();
            let disabled_plugins = v["disabled_plugins"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            Some(Settings {
                api_keys: keys,
                disabled_plugins,
            })
        })
        .unwrap_or_default()
}