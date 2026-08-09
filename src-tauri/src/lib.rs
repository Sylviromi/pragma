use sqlx::SqlitePool;
use tauri::{Emitter, Manager};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

mod db;
mod commands;
mod plugins;
mod providers;
mod screenshot;
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
    // session IDs with an active agentic loop — used by frontend to reconnect after HMR
    pub running_sessions: Arc<tokio::sync::Mutex<std::collections::HashSet<String>>>,
    // one-shot cancel signal per session — interrupt_session() flips it; the
    // agentic loop selects on it while waiting on the provider / approvals
    pub loop_cancels: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::watch::Sender<bool>>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Wayland compositors (Hyprland, Sway, …) draw their own window
            // frame, so the GTK client-side header bar should not render there.
            // X11 / GNOME keep the normal decorations.
            if is_wayland_session() {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.set_decorations(false);
                }
            }

            let config_dir = app.path().app_config_dir().expect("no config dir");
            std::fs::create_dir_all(&config_dir).ok();
            let plugins_dir = config_dir.join("plugins");
            std::fs::create_dir_all(&plugins_dir).ok();

            let db_path = format!("sqlite:{}", config_dir.join("sessions.db").display());
            let rt = tokio::runtime::Runtime::new().unwrap();
            let pool = rt.block_on(db::init_db(&db_path)).expect("db init failed");

            // Sweep stale cross-process loop locks (rows whose owning process
            // died) so a fresh instance never inherits a phantom "already
            // running" state for a session.
            rt.block_on(async {
                let stale: Vec<String> = sqlx::query_scalar("SELECT session_id FROM active_loops")
                    .fetch_all(&pool)
                    .await
                    .unwrap_or_default();
                for sid in stale {
                    if commands::agentic::loop_row_is_stale(&pool, &sid).await.unwrap_or(false) {
                        let _ = sqlx::query("DELETE FROM active_loops WHERE session_id=?")
                            .bind(&sid)
                            .execute(&pool)
                            .await;
                    }
                }
            });

            let settings_path = config_dir.join("settings.json");
            let settings = load_settings(&settings_path);

            let registry = providers::ProviderRegistry::default();
            registry.register(providers::anthropic::AnthropicProvider::default());
            registry.register(providers::openai_compat::deepseek());
            registry.register(providers::openai_compat::opencode_local());

            let plugin_manager =
                plugins::PluginManager::new(plugins_dir.clone(), settings.disabled_plugins.clone());
            let wrote_demo = plugin_manager.ensure_demo_plugin();
            if wrote_demo {
                eprintln!("[plugins] wrote demo plugin into {}", plugins_dir.display());
            }
            plugins::provider::register_plugin_providers(&registry, &plugin_manager, &settings.api_keys, &app.handle());

            app.manage(AppState {
                db: pool,
                registry,
                settings: RwLock::new(settings),
                settings_path,
                pending_approvals: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                running_sessions: Arc::new(tokio::sync::Mutex::new(std::collections::HashSet::new())),
                loop_cancels: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                plugins: plugin_manager,
            });

            // Model discovery for the built-in providers (Anthropic /v1/models,
            // DeepSeek and local /v1/models). Runs detached; results replace the
            // static fallbacks and emit providers:changed so the UI refreshes.
            providers::spawn_builtin_discovery(&app.handle());

            // Hot-reload watcher: rebind providers + tell the frontend to re-scan.
            let app_handle = app.handle().clone();
            let watched_plugins_dir = plugins_dir.clone();
            plugins::watcher::spawn_plugin_watcher(&plugins_dir, move || {
                let handle = app_handle.clone();
                let plugin_watch = watched_plugins_dir.clone();
                std::thread::spawn(move || {
                    let state = handle.state::<AppState>();
                    let api_keys = state.settings.read().unwrap().api_keys.clone();
                    plugins::provider::unregister_plugin_providers(&state.registry, &state.plugins);
                    plugins::provider::register_plugin_providers(&state.registry, &state.plugins, &api_keys, &handle);
                    let _ = handle.emit(
                        "plugin:reload",
                        serde_json::json!({ "version": chrono::Utc::now().timestamp_millis() }),
                    );

                    // Dev convenience: dropping a file named ".shot" inside the
                    // plugins dir makes the app capture its own window to the
                    // app-data screenshots folder, then deletes the marker.
                    let marker = plugin_watch.join(".shot");
                    if marker.exists() {
                        let _ = std::fs::remove_file(&marker);
                        match screenshot::capture_core(&handle) {
                            Ok(path) => eprintln!("[shot] saved {path}"),
                            Err(e) => eprintln!("[shot] failed: {e}"),
                        }
                    }
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
            commands::agentic::run_agentic_loop,
            commands::agentic::get_running_sessions,
            commands::agentic::interrupt_session,
            commands::tools::approve_tool,
            commands::tools::deny_tool,
            commands::tools::get_pending_approvals,
            commands::revert::revert_session,
            commands::revert::get_revert_info,
            commands::providers::save_api_key,
            commands::providers::get_api_keys,
            commands::plugins::list_plugins,
            commands::plugins::set_plugin_enabled,
            commands::plugins::read_plugin_component,
            commands::plugins::read_plugin_skill,
            commands::plugins::open_plugin_folder,
            commands::process::spawn_process,
            commands::process::kill_process,
            commands::process::probe_port,
            screenshot::capture_app_screenshot,
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

/// True when running under a Wayland compositor (Hyprland, Sway, KWin wayland,
/// …) rather than X11 — used to suppress the GTK client-side window header.
#[cfg(target_os = "linux")]
fn is_wayland_session() -> bool {
    detects_wayland(
        std::env::var("XDG_SESSION_TYPE").ok(),
        std::env::var("WAYLAND_DISPLAY").ok(),
    )
}

#[cfg(not(target_os = "linux"))]
fn is_wayland_session() -> bool {
    false
}

/// Pure detector so it's unit-testable without touching process env.
fn detects_wayland(xdg_session_type: Option<String>, wayland_display: Option<String>) -> bool {
    xdg_session_type
        .map(|v| v.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
        || wayland_display.map(|s| !s.is_empty()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_detection() {
        // XDG_SESSION_TYPE=wayland (Hyprland/Sway) → no decorations
        assert!(detects_wayland(Some("wayland".into()), None));
        assert!(detects_wayland(Some("WAYLAND".into()), Some("wayland-0".into())));
        // WAYLAND_DISPLAY set (some sessions only set the socket)
        assert!(detects_wayland(None, Some("wayland-1".into())));
        // X11 / GNOME on Xorg → keep decorations
        assert!(!detects_wayland(Some("x11".into()), None));
        assert!(!detects_wayland(Some("tty".into()), None));
        assert!(!detects_wayland(None, None));
    }
}