use crate::AppState;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// A managed plugin process with captured I/O.
pub struct PluginProcess {
    child: Child,
    stdin: Option<tokio::process::ChildStdin>,
    pid: u32,
}

/// Thread-safe map of plugin_id → managed process.
pub type PluginProcesses = Arc<Mutex<HashMap<String, PluginProcess>>>;

pub fn new_process_map() -> PluginProcesses {
    Arc::new(Mutex::new(HashMap::new()))
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluginProcessInfo {
    pub running: bool,
    pub pid: Option<u32>,
}

/// Spawn a managed process for a plugin. Stdout/stderr are captured line-by-line
/// and emitted as `plugin:stdout:{plugin_id}` / `plugin:stderr:{plugin_id}` events.
/// Only one process per plugin at a time.
#[tauri::command]
pub async fn spawn_plugin_process(
    plugin_id: String,
    program: String,
    args: Vec<String>,
    cwd: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<u32, String> {
    // Validate plugin exists and is enabled
    if !state.plugins.is_enabled(&plugin_id) {
        return Err(format!("plugin '{plugin_id}' is not enabled"));
    }
    if state.plugins.manifest(&plugin_id).is_none() {
        return Err(format!("plugin '{plugin_id}' not found"));
    }

    let mut procs = state.plugin_processes.lock().await;
    if let Some(existing) = procs.get(&plugin_id) {
        // Check if still running
        if existing.child.id().is_some() {
            return Err(format!("plugin '{plugin_id}' already has a running process"));
        }
    }

    let mut cmd = Command::new(&program);
    cmd.args(&args);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);
    if let Some(dir) = &cwd {
        cmd.current_dir(dir);
    }
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }

    let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let pid = child.id().ok_or("failed to get child PID")?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdin = child.stdin.take();

    // Spawn stdout reader
    if let Some(out) = stdout {
        let app_h = app.clone();
        let pid_copy = plugin_id.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(out);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let event_name = format!("plugin:stdout:{pid_copy}");
                let _ = app_h.emit(&event_name, &line);
            }
        });
    }

    // Spawn stderr reader
    if let Some(err) = stderr {
        let app_h = app.clone();
        let pid_copy = plugin_id.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(err);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let event_name = format!("plugin:stderr:{pid_copy}");
                let _ = app_h.emit(&event_name, &line);
            }
        });
    }

    // Spawn a task to clean up when the process exits
    {
        let procs_ref = state.plugin_processes.clone();
        let pid_copy = plugin_id.clone();
        let app_h = app.clone();
        tokio::spawn(async move {
            // Wait a bit then start polling — the child is owned by the map
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let mut map = procs_ref.lock().await;
                if let Some(proc) = map.get_mut(&pid_copy) {
                    match proc.child.try_wait() {
                        Ok(Some(_status)) => {
                            map.remove(&pid_copy);
                            let event_name = format!("plugin:process-exit:{pid_copy}");
                            let _ = app_h.emit(&event_name, ());
                            break;
                        }
                        Ok(None) => {} // still running
                        Err(_) => {
                            map.remove(&pid_copy);
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
        });
    }

    procs.insert(
        plugin_id,
        PluginProcess {
            child,
            stdin,
            pid,
        },
    );

    Ok(pid)
}

/// Stop a plugin's managed process. Sends SIGTERM, waits briefly, then SIGKILL.
#[tauri::command]
pub async fn stop_plugin_process(
    plugin_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut procs = state.plugin_processes.lock().await;
    let proc = procs
        .get_mut(&plugin_id)
        .ok_or(format!("no running process for plugin '{plugin_id}'"))?;

    // Try graceful kill first
    let _ = proc.child.kill().await;
    procs.remove(&plugin_id);
    Ok(())
}

/// Check whether a plugin has a running managed process.
#[tauri::command]
pub async fn plugin_process_status(
    plugin_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<PluginProcessInfo, String> {
    let mut procs = state.plugin_processes.lock().await;
    if let Some(proc) = procs.get_mut(&plugin_id) {
        match proc.child.try_wait() {
            Ok(Some(_)) => {
                let pid = proc.pid;
                procs.remove(&plugin_id);
                Ok(PluginProcessInfo {
                    running: false,
                    pid: Some(pid),
                })
            }
            Ok(None) => Ok(PluginProcessInfo {
                running: true,
                pid: Some(proc.pid),
            }),
            Err(_) => {
                procs.remove(&plugin_id);
                Ok(PluginProcessInfo {
                    running: false,
                    pid: None,
                })
            }
        }
    } else {
        Ok(PluginProcessInfo {
            running: false,
            pid: None,
        })
    }
}

/// Write a line to a plugin process's stdin (for bidirectional IPC).
#[tauri::command]
pub async fn write_plugin_stdin(
    plugin_id: String,
    data: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut procs = state.plugin_processes.lock().await;
    let proc = procs
        .get_mut(&plugin_id)
        .ok_or(format!("no running process for plugin '{plugin_id}'"))?;

    let stdin = proc
        .stdin
        .as_mut()
        .ok_or("process stdin not available")?;

    let line = if data.ends_with('\n') {
        data
    } else {
        format!("{data}\n")
    };

    stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|e| format!("write to stdin failed: {e}"))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("flush stdin failed: {e}"))?;

    Ok(())
}

/// Returns true if 127.0.0.1:port is currently accepting TCP connections.
#[tauri::command]
pub async fn probe_port(port: u16) -> bool {
    tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_ok()
}
