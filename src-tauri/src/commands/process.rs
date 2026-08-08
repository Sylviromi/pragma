/// Spawn a detached background process. Returns the PID.
/// Stdin/stdout/stderr are redirected to /dev/null (or equivalent).
/// On Unix the child is placed in its own process group so it survives if Pragma exits.
#[tauri::command]
pub async fn spawn_process(
    program: String,
    args: Vec<String>,
    cwd: Option<String>,
) -> Result<u32, String> {
    let mut cmd = std::process::Command::new(&program);
    cmd.args(&args);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    if let Some(dir) = &cwd {
        cmd.current_dir(dir);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    Ok(child.id())
}

/// Send SIGTERM to the given PID. Silently succeeds if the process is already gone.
#[tauri::command]
pub async fn kill_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        // Ignore non-zero exit — process may have already exited.
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Returns true if 127.0.0.1:port is currently accepting TCP connections.
#[tauri::command]
pub async fn probe_port(port: u16) -> bool {
    tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_ok()
}
