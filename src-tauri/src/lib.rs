use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tauri::{Emitter, Manager};

mod commands;
mod db;
mod notify;
mod plugins;
mod providers;
mod tools;

pub struct Settings {
    /// API keys per provider, in rotation order — the first is the primary
    /// (used for discovery); the harness rotates through the rest when a call
    /// comes back rate-limited (HTTP 429/402/529).
    pub api_keys: HashMap<String, Vec<String>>,
    pub disabled_plugins: Vec<String>,
    /// Plugin ids the app has loaded at least once — anything NOT in this
    /// list is a first-time plugin and starts DISABLED.
    pub seen_plugins: Vec<String>,
    pub notifications_enabled: bool,
    pub verification: VerificationSettings,
}

/// Granular toggles for the harness verification layer (runs at loop-done:
/// deterministic project checks + one small asymmetric LLM review).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationSettings {
    /// Master switch — verification phase entirely on/off.
    pub enabled: bool,
    /// Run the project's checks (cargo test / npm test …) — deterministic, no LLM.
    pub run_checks: bool,
    /// Independent LLM review pass (task + diff + check output only, no conversation).
    pub independent_review: bool,
    /// Max fix rounds fed back to the agent before surfacing the report (0 = report only).
    pub max_rounds: u32,
    /// Override model for the reviewer call (defaults to the session model).
    pub reviewer_model: Option<String>,
    /// Override test command (defaults to auto-detection from the project).
    pub test_command: Option<String>,
}

impl Default for VerificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            run_checks: true,
            independent_review: true,
            max_rounds: 2,
            reviewer_model: None,
            test_command: None,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_keys: HashMap::new(),
            disabled_plugins: Vec::new(),
            seen_plugins: Vec::new(),
            notifications_enabled: true,
            verification: VerificationSettings::default(),
        }
    }
}

pub struct AppState {
    pub db: SqlitePool,
    pub registry: providers::ProviderRegistry,
    pub settings: RwLock<Settings>,
    pub settings_path: std::path::PathBuf,
    pub plugins: plugins::PluginManager,
    // pending tool approvals: tool_use_id → oneshot sender
    pub pending_approvals:
        Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    // approvals/denials that arrived BEFORE the loop registered a pending
    // entry (the frontend auto-approves SAFE tools the moment it sees the
    // tool_call event — those signals must not be lost).
    pub pre_approved: Arc<tokio::sync::Mutex<std::collections::HashSet<String>>>,
    pub pre_denied: Arc<tokio::sync::Mutex<std::collections::HashSet<String>>>,
    // pending ask_question answers: question_id → oneshot sender (Option = answer,
    // None = timeout/no answer). Mirrors pending_approvals so the loop can pause
    // on a question and resume from answer_question.
    pub question_channels:
        Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<Option<String>>>>>,
    // session IDs with an active agentic loop — used by frontend to reconnect after HMR
    pub running_sessions: Arc<tokio::sync::Mutex<std::collections::HashSet<String>>>,
    // per-session "what the model has SEEN" of each file: write gate + dedup
    pub seen_ledger: tools::SeenLedger,
    // one-shot cancel signal per session — interrupt_session() flips it; the
    // agentic loop selects on it while waiting on the provider / approvals
    pub loop_cancels: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::watch::Sender<bool>>>>,
    // per-provider API-key rotation offset (index into settings.api_keys[provider]) —
    // persists across runs while the app is open so key #0 isn't hammered every run
    pub api_key_rotation: Arc<tokio::sync::Mutex<HashMap<String, usize>>>,
    // managed plugin sidecar processes (plugin_id → process handle)
    pub plugin_processes: commands::process::PluginProcesses,
    // skill sidecars (skill_id → JSON-RPC handler process) for plugin tool execution
    pub skill_sidecars: tools::sidecar::SkillSidecarRegistry,
    // pending plugin confirmation dialogs: request_id → oneshot sender
    pub plugin_confirmations:
        Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
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
                    if commands::agentic::loop_row_is_stale(&pool, &sid)
                        .await
                        .unwrap_or(false)
                    {
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
            registry.register(providers::openai_compat::opencode_go());

            let plugin_manager = plugins::PluginManager::new(
                plugins_dir.clone(),
                settings.disabled_plugins.clone(),
                settings.seen_plugins.clone(),
            );
            // First-time plugins start DISABLED: anything on disk that was
            // never seen before (installed while the app was closed) is
            // disabled before the UI loads. Existing plugins were seeded as
            // seen at construction and keep their state.
            let new_plugins_seen = plugin_manager.register_new_plugins();
            // Symlinked plugin entries (dev workflow: plugin lives in an
            // untracked folder, linked in with `ln -s`) — watch their real
            // targets explicitly since notify doesn't follow symlinks.
            let symlink_targets = plugin_manager.symlink_targets();
            plugins::provider::register_plugin_providers(
                &registry,
                &plugin_manager,
                &settings.api_keys,
                app.handle(),
            );

            app.manage(AppState {
                db: pool,
                registry,
                settings: RwLock::new(settings),
                settings_path,
                pending_approvals: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                pre_approved: Arc::new(tokio::sync::Mutex::new(std::collections::HashSet::new())),
                pre_denied: Arc::new(tokio::sync::Mutex::new(std::collections::HashSet::new())),
                question_channels: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                running_sessions: Arc::new(tokio::sync::Mutex::new(
                    std::collections::HashSet::new(),
                )),
                seen_ledger: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
                loop_cancels: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                api_key_rotation: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                plugin_processes: commands::process::new_process_map(),
                skill_sidecars: tools::sidecar::SkillSidecarRegistry::new(),
                plugin_confirmations: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                plugins: plugin_manager,
            });

            // Persist the seen/disabled state from the startup sweep (a
            // first-time plugin found on disk was just disabled).
            if new_plugins_seen {
                let state = app.handle().state::<AppState>();
                crate::save_settings(&state);
            }

            // Model discovery for the built-in providers (Anthropic /v1/models,
            // DeepSeek and local /v1/models). Runs detached; results replace the
            // static fallbacks and emit providers:changed so the UI refreshes.
            providers::spawn_builtin_discovery(app.handle());

            // Hot-reload watcher: rebind providers + tell the frontend to re-scan.
            let app_handle = app.handle().clone();
            let symlink_targets2 = symlink_targets.clone();
            plugins::watcher::spawn_plugin_watcher(&plugins_dir, symlink_targets2, move || {
                let handle = app_handle.clone();
                std::thread::spawn(move || {
                    let state = handle.state::<AppState>();
                    // Kill skill sidecars — a plugin/skill change may have
                    // replaced handler.js; they respawn at the next run start.
                    let sidecars = state.skill_sidecars.clone();
                    let _ =
                        tokio::runtime::Runtime::new().map(|rt| rt.block_on(sidecars.stop_all()));
                    let api_keys = state.settings.read().unwrap().api_keys.clone();
                    plugins::provider::unregister_plugin_providers(&state.registry, &state.plugins);
                    plugins::provider::register_plugin_providers(
                        &state.registry,
                        &state.plugins,
                        &api_keys,
                        &handle,
                    );
                    // A plugin folder added at runtime is first-time: disable
                    // it before the frontend re-scans, and persist the state.
                    if state.plugins.register_new_plugins() {
                        crate::save_settings(&state);
                    }
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
            notify::set_notifications_enabled,
            notify::get_notifications_enabled,
            commands::messages::add_message,
            commands::messages::list_messages,
            commands::providers::list_providers,
            commands::agentic::run_agentic_loop,
            commands::agentic::get_running_sessions,
            commands::agentic::interrupt_session,
            commands::tools::approve_tool,
            commands::tools::deny_tool,
            commands::tools::answer_question,
            commands::tools::get_pending_approvals,
            commands::revert::revert_session,
            commands::revert::get_revert_info,
            commands::stats::get_session_stats,
            commands::providers::save_api_keys,
            commands::providers::get_api_keys,
            commands::providers::get_verification_settings,
            commands::providers::set_verification_settings,
            commands::plugins::list_plugins,
            commands::plugins::set_plugin_enabled,
            commands::plugins::install_plugin,
            commands::plugins::uninstall_plugin,
            commands::plugins::read_plugin_component,
            commands::plugins::read_plugin_skill,
            commands::plugins::list_skills,
            commands::plugins::open_plugin_folder,
            commands::process::spawn_plugin_process,
            commands::process::stop_plugin_process,
            commands::process::plugin_process_status,
            commands::process::write_plugin_stdin,
            commands::process::probe_port,
            commands::plugins::get_plugin_config,
            commands::plugins::set_plugin_config,
            commands::plugins::request_plugin_confirmation,
            commands::plugins::respond_plugin_confirmation,
            commands::skills::skill_sidecar_status,
            commands::skills::call_skill_tool,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Persist settings.json (api_keys + disabled/seen plugins + verification).
pub fn save_settings(state: &AppState) {
    let settings = state.settings.read().unwrap();
    write_settings(
        &state.settings_path,
        &settings.api_keys,
        settings.notifications_enabled,
        &settings.verification,
        // PluginManager is the single source of truth for plugin on/off state:
        // toggles mutate its internal list (set_plugin_enabled), so persisting
        // the live list — not the startup-time Settings snapshot — is what
        // makes plugins remember their state across restarts.
        &state.plugins.disabled_ids(),
        &state.plugins.seen_ids(),
    );
}

/// Serialize + write settings.json.
fn write_settings(
    path: &std::path::Path,
    api_keys: &HashMap<String, Vec<String>>,
    notifications_enabled: bool,
    verification: &VerificationSettings,
    disabled_plugins: &[String],
    seen_plugins: &[String],
) {
    let json = serde_json::json!({
        "api_keys": api_keys,
        "disabled_plugins": disabled_plugins,
        "seen_plugins": seen_plugins,
        "notifications_enabled": notifications_enabled,
        "verification": {
            "enabled": verification.enabled,
            "run_checks": verification.run_checks,
            "independent_review": verification.independent_review,
            "max_rounds": verification.max_rounds,
            "reviewer_model": verification.reviewer_model,
            "test_command": verification.test_command,
        },
    });
    let _ = std::fs::write(path, serde_json::to_string_pretty(&json).unwrap());
}

fn load_settings(path: &std::path::Path) -> Settings {
    if !path.exists() {
        return Settings::default();
    }
    let text = std::fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .map(|v| {
            // api_keys: each provider holds an ordered list (rotation order).
            // Legacy files stored a single string per provider — migrate.
            let keys = v["api_keys"]
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| {
                            let list: Vec<String> = if let Some(arr) = v.as_array() {
                                arr.iter()
                                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                    .filter(|s| !s.is_empty())
                                    .collect()
                            } else {
                                v.as_str()
                                    .map(|s| s.to_string())
                                    .filter(|s| !s.is_empty())
                                    .map(|s| vec![s])
                                    .unwrap_or_default()
                            };
                            (!list.is_empty()).then_some((k.clone(), list))
                        })
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
            // Plugin ids the app has loaded before. Missing/empty on the
            // upgrade run — PluginManager seeds on-disk plugins there.
            let seen_plugins = v["seen_plugins"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let notifications_enabled = v["notifications_enabled"].as_bool().unwrap_or(true);
            // Verification settings: each field falls back to its default when
            // missing (older settings.json files won't have the object at all).
            let def = VerificationSettings::default();
            let vv = &v["verification"];
            let verification = VerificationSettings {
                enabled: vv["enabled"].as_bool().unwrap_or(def.enabled),
                run_checks: vv["run_checks"].as_bool().unwrap_or(def.run_checks),
                independent_review: vv["independent_review"]
                    .as_bool()
                    .unwrap_or(def.independent_review),
                max_rounds: vv["max_rounds"].as_u64().unwrap_or(def.max_rounds as u64) as u32,
                reviewer_model: vv["reviewer_model"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
                test_command: vv["test_command"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
            };
            Settings {
                api_keys: keys,
                disabled_plugins,
                seen_plugins,
                notifications_enabled,
                verification,
            }
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
    fn settings_round_trip_preserves_disabled_plugins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut keys = HashMap::new();
        keys.insert(
            "anthropic".to_string(),
            vec!["sk-test".to_string(), "sk-test-2".to_string()],
        );
        // A disable toggle persisted the PluginManager's live list:
        write_settings(
            &path,
            &keys,
            false,
            &VerificationSettings::default(),
            &["demo".to_string()],
            &["demo".to_string(), "zen".to_string()],
        );
        // Restart: load_settings must restore the exact disabled/seen sets + keys.
        let loaded = load_settings(&path);
        assert_eq!(loaded.disabled_plugins, vec!["demo"]);
        assert_eq!(loaded.seen_plugins, vec!["demo", "zen"]);
        assert_eq!(
            loaded.api_keys.get("anthropic").unwrap(),
            &vec!["sk-test".to_string(), "sk-test-2".to_string()]
        );
        assert!(!loaded.notifications_enabled);
        // Defaults when the file is absent.
        let loaded_default = load_settings(&dir.path().join("missing.json"));
        assert!(loaded_default.disabled_plugins.is_empty());
        assert!(loaded_default.notifications_enabled);
    }

    #[test]
    fn load_settings_migrates_legacy_string_api_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Old format: a plain string per provider.
        std::fs::write(
            &path,
            r#"{"api_keys":{"anthropic":"sk-old","deepseek":""},"disabled_plugins":[]}"#,
        )
        .unwrap();
        let loaded = load_settings(&path);
        // String migrated to a one-element list; empty strings dropped.
        assert_eq!(
            loaded.api_keys.get("anthropic").unwrap(),
            &vec!["sk-old".to_string()]
        );
        assert!(!loaded.api_keys.contains_key("deepseek"));
        // The array format round-trips as-is.
        write_settings(
            &path,
            &loaded.api_keys,
            true,
            &VerificationSettings::default(),
            &[],
            &[],
        );
        let again = load_settings(&path);
        assert_eq!(
            again.api_keys.get("anthropic").unwrap(),
            &vec!["sk-old".to_string()]
        );
        // Verification settings round-trip through the file.
        assert!(again.verification.enabled);
        assert_eq!(again.verification.max_rounds, 2);
    }

    #[test]
    fn wayland_detection() {
        // XDG_SESSION_TYPE=wayland (Hyprland/Sway) → no decorations
        assert!(detects_wayland(Some("wayland".into()), None));
        assert!(detects_wayland(
            Some("WAYLAND".into()),
            Some("wayland-0".into())
        ));
        // WAYLAND_DISPLAY set (some sessions only set the socket)
        assert!(detects_wayland(None, Some("wayland-1".into())));
        // X11 / GNOME on Xorg → keep decorations
        assert!(!detects_wayland(Some("x11".into()), None));
        assert!(!detects_wayland(Some("tty".into()), None));
        assert!(!detects_wayland(None, None));
    }
}
