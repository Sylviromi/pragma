use sqlx::SqlitePool;
use tauri::Manager;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

mod db;
mod commands;
mod providers;
mod tools;

#[derive(Default)]
pub struct Settings {
    pub api_keys: HashMap<String, String>,
}

pub struct AppState {
    pub db: SqlitePool,
    pub registry: providers::ProviderRegistry,
    pub settings: RwLock<Settings>,
    pub settings_path: std::path::PathBuf,
    // pending tool approvals: tool_use_id → oneshot sender
    pub pending_approvals: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir().expect("no config dir");
            std::fs::create_dir_all(&config_dir).ok();

            let db_path = format!("sqlite:{}", config_dir.join("sessions.db").display());
            let rt = tokio::runtime::Runtime::new().unwrap();
            let pool = rt.block_on(db::init_db(&db_path)).expect("db init failed");

            let settings_path = config_dir.join("settings.json");
            let settings = load_settings(&settings_path);

            let registry = providers::ProviderRegistry::default();
            registry.register(providers::anthropic::AnthropicProvider);
            registry.register(providers::openai_compat::deepseek());
            registry.register(providers::openai_compat::opencode_local());

            app.manage(AppState {
                db: pool,
                registry,
                settings: RwLock::new(settings),
                settings_path,
                pending_approvals: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn load_settings(path: &std::path::Path) -> Settings {
    if !path.exists() {
        return Settings::default();
    }
    let text = std::fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| {
            let keys = v["api_keys"].as_object()?;
            Some(Settings {
                api_keys: keys
                    .iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect(),
            })
        })
        .unwrap_or_default()
}