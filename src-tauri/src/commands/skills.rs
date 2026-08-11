//! Bridge commands that expose plugin-skill sidecars to the frontend UI.
//!
//! Skill sidecars are model-driven (spawned on first `load_skill`, tools
//! routed from the agentic loop). These commands let UI plugins — e.g. the
//! pragma-dev status-bar widget — see the same state and drive the same
//! processes: a widget can poll `skill_sidecar_status` for the dev server's
//! state and call its tools (`dev_start` / `dev_stop`) directly.

use crate::AppState;
use crate::tools::sidecar::SidecarStatus;

/// List every registered skill sidecar (skill id, pid, running?, tool names).
#[tauri::command]
pub async fn skill_sidecar_status(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SidecarStatus>, String> {
    Ok(state.skill_sidecars.status().await)
}

/// Ensure a skill's sidecar is up, then call one of its tools.
///
/// `ensure` spawns the handler on demand, so UI widgets work even before the
/// model has loaded the skill. The sidecar is initialized with no working
/// directory; tools that need one take an explicit `dir` argument.
#[tauri::command]
pub async fn call_skill_tool(
    skill_id: String,
    name: String,
    arguments: serde_json::Value,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    state
        .skill_sidecars
        .ensure(&state.plugins, &skill_id, None)
        .await?;
    state
        .skill_sidecars
        .call_tool(&skill_id, &name, &arguments)
        .await
}