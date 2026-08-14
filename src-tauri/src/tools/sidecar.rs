//! Long-lived sidecar processes for plugin skills that ship a `handler.js`.
//!
//! Wire protocol: **JSON-RPC 2.0 over newline-delimited JSON** on stdin/stdout.
//!
//! ```text
//! host → child:  {"jsonrpc":"2.0","id":1,"method":"initialize",
//!                 "params":{"skillId":"fe/frontend-dev","workingDir":"/abs/path"}}
//! child → host:  {"jsonrpc":"2.0","id":1,
//!                 "result":{"protocolVersion":"1","tools":[{name,description,inputSchema},…]}}
//! host → child:  {"jsonrpc":"2.0","id":2,"method":"tools/call",
//!                 "params":{"name":"fe_status","arguments":{…}}}
//! child → host:  {"jsonrpc":"2.0","id":2,
//!                 "result":{"content":[{"type":"text","text":"…"}],"isError":false}}
//! child → host:  {"jsonrpc":"2.0","id":2,"error":{"code":…,"message":"…"}}
//! ```
//!
//! The **model-facing tool set always comes from the skill's `tools.json`**
//! (declarative, hot-reload-safe); `handler.js` implements those tools. The
//! initialize handshake is a liveness check plus an optional capability echo
//! (a handler may omit `tools`). Sidecars are spawned lazily on first
//! `load_skill`, reused across turns, and killed by dropping the child
//! (`kill_on_drop`) — which happens on app exit and explicit `stop_*`.

use crate::plugins::PluginManager;
use crate::providers::types::ToolDefinition;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, Mutex};

/// Snapshot of one registered skill sidecar, serialized to the frontend
/// (camelCase). `running=false` means the skill is registered for routing
/// but its handler failed to start.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SidecarStatus {
    pub skill_id: String,
    pub pid: u32,
    pub running: bool,
    pub tools: Vec<String>,
}

/// In-flight JSON-RPC calls shared by a sidecar and its call path:
/// request id → oneshot response channel.
type PendingCalls = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<serde_json::Value, String>>>>>;

/// Per-skill running sidecar (or a registered-but-failed placeholder).
pub struct SidecarState {
    #[allow(dead_code)] // kept for a future status surface (process_skill_status)
    pub pid: u32,
    /// False while a skill is registered for routing but its handler failed
    /// to spawn — calls then fail fast with a clear message instead of
    /// timing out against a dead pipe.
    pub running: bool,
    /// Advertised tools (from the skill's tools.json) — the routing index
    /// source. The handler's initialize echo is informational only.
    pub tools: Vec<ToolDefinition>,
    /// Owned handler process — dropping it (on stop / map clear / registry
    /// drop at app exit) triggers `kill_on_drop`.
    #[allow(dead_code)] // kept for diagnostics/debugging
    _child: tokio::process::Child,
    /// Outbound frame channel; an idle writer task owns the child's stdin.
    tx: mpsc::UnboundedSender<String>,
    /// In-flight requests: JSON-RPC id → response channel.
    pending: PendingCalls,
    next_id: u64,
}

type SidecarMapInner = HashMap<String, SidecarState>;

/// Thread-safe registry of skill sidecars, keyed by skill id, with a
/// tool-name → skill-id routing index rebuilt on every change.
#[derive(Clone)]
pub struct SkillSidecarRegistry {
    inner: Arc<Mutex<SidecarMapInner>>,
    // std lock: touched only briefly, never while awaiting.
    by_name: Arc<std::sync::RwLock<HashMap<String, String>>>,
}

fn parse_tools(v: Option<&serde_json::Value>) -> Vec<ToolDefinition> {
    let Some(arr) = v.and_then(|x| x.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for t in arr {
        let Some(name) = t.get("name").and_then(|x| x.as_str()) else {
            continue;
        };
        let description = t
            .get("description")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let input_schema = t
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({ "type": "object", "properties": {} }));
        out.push(ToolDefinition {
            name: name.to_string(),
            description,
            input_schema,
        });
    }
    out
}

impl SkillSidecarRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            by_name: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    fn rebuild_index(&self, map: &SidecarMapInner) {
        let mut idx = HashMap::new();
        for (skill_id, st) in map {
            for t in &st.tools {
                idx.insert(t.name.clone(), skill_id.clone());
            }
        }
        *self.by_name.write().unwrap() = idx;
    }

    /// The skill that owns a tool name, if any. Routing happens even for a
    /// skill whose sidecar failed to start — the call then errors clearly
    /// ("sidecar not running") instead of falling through to unknown-tool.
    pub fn owner(&self, name: &str) -> Option<String> {
        self.by_name.read().unwrap().get(name).cloned()
    }

    /// Which skill sidecar a tool name maps to, if running.
    pub fn running_owner(&self, name: &str) -> Option<String> {
        let owner = self.owner(name)?;
        let map = self.inner.try_lock().ok()?;
        if map.get(&owner).map(|s| s.running).unwrap_or(false) {
            Some(owner)
        } else {
            None
        }
    }

    /// Ensure a skill's sidecar is spawned and registered. Registers the
    /// skill's tools.json tool set first so routing works even when the spawn
    /// fails; a running instance is reused.
    pub async fn ensure(
        &self,
        manager: &PluginManager,
        skill_id: &str,
        working_dir: Option<&str>,
    ) -> Result<(), String> {
        {
            let map = self.inner.lock().await;
            if map.get(skill_id).map(|s| s.running).unwrap_or(false) {
                return Ok(());
            }
        }
        let handler = manager
            .skill_handler_path(skill_id)
            .ok_or_else(|| format!("skill '{skill_id}' has no handler.js"))?;
        let advertised = manager.skill_tools_json(skill_id).unwrap_or_default();

        // Spawn outside the registry lock (node start can block briefly);
        // position the route BEFORE spawn so a failed start still routes.
        let (child, pid, tx, pending) = spawn_handler(skill_id, &handler).await?;
        let st = SidecarState {
            pid,
            running: true,
            tools: advertised,
            _child: child,
            tx,
            pending: pending.clone(),
            next_id: 1,
        };
        {
            let mut map = self.inner.lock().await;
            map.insert(skill_id.to_string(), st);
        }
        let guard = self.inner.lock().await;
        self.rebuild_index(&guard);
        drop(guard);

        // Handshake: liveness + optional capability echo. A failure here
        // means the handler exists but misbehaves — mark it down.
        let params = serde_json::json!({
            "skillId": skill_id,
            "workingDir": working_dir.unwrap_or(""),
        });
        match self
            .request(skill_id, "initialize", params, Duration::from_secs(30))
            .await
        {
            Ok(res) => {
                let echoed = parse_tools(res.get("tools"));
                let mut map = self.inner.lock().await;
                if let Some(st) = map.get_mut(skill_id) {
                    // Echoed tools augment the index only when tools.json is
                    // absent (nothing declared → the handler still works).
                    if st.tools.is_empty() {
                        st.tools = echoed;
                        self.rebuild_index(&map);
                    }
                }
                Ok(())
            }
            Err(e) => {
                // Mark down and kill.
                self.mark_down(skill_id).await;
                Err(e)
            }
        }
    }

    /// Kill + unregister a skill's sidecar.
    pub async fn stop(&self, skill_id: &str) {
        let mut map = self.inner.lock().await;
        if map.remove(skill_id).is_some() {
            self.rebuild_index(&map);
        }
    }

    /// Stop every sidecar owned by a plugin (all its skills).
    pub async fn stop_plugin(&self, plugin_id: &str) {
        let prefix = format!("{plugin_id}/");
        let mut map = self.inner.lock().await;
        let mut changed = false;
        map.retain(|sid, _| {
            let keep = !sid.starts_with(&prefix);
            changed |= !keep;
            keep
        });
        if changed {
            self.rebuild_index(&map);
        }
    }

    /// Stop everything (plugin hot-reload, app shutdown).
    pub async fn stop_all(&self) {
        let mut map = self.inner.lock().await;
        if !map.is_empty() {
            map.clear();
            self.rebuild_index(&map);
        }
    }

    pub async fn running_skills(&self) -> Vec<String> {
        let map = self.inner.lock().await;
        map.iter()
            .filter(|(_, s)| s.running)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Snapshot of every registered skill sidecar (running or marked down) —
    /// the status surface for the `skill_sidecar_status` command (UI widgets
    /// like the pragma-dev status bar poll this).
    pub async fn status(&self) -> Vec<SidecarStatus> {
        let map = self.inner.lock().await;
        let mut out: Vec<SidecarStatus> = map
            .iter()
            .map(|(skill_id, st)| SidecarStatus {
                skill_id: skill_id.clone(),
                pid: st.pid,
                running: st.running,
                tools: st.tools.iter().map(|t| t.name.clone()).collect(),
            })
            .collect();
        out.sort_by(|a, b| a.skill_id.cmp(&b.skill_id));
        out
    }

    /// Mark a skill down (spawn/handshake failure) — keeps its advertised
    /// tools routable so calls report a clear error.
    async fn mark_down(&self, skill_id: &str) {
        let mut map = self.inner.lock().await;
        if let Some(st) = map.get_mut(skill_id) {
            st.running = false;
        }
        drop(map);
        let guard = self.inner.lock().await;
        self.rebuild_index(&guard);
    }

    /// JSON-RPC request/response over the sidecar's stdio with a timeout.
    async fn request(
        &self,
        skill_id: &str,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, String> {
        let (orx, tx, id) = {
            let mut map = self.inner.lock().await;
            let st = map
                .get_mut(skill_id)
                .ok_or_else(|| format!("sidecar for skill '{skill_id}' is not registered"))?;
            if !st.running {
                return Err(format!(
                    "sidecar for skill '{skill_id}' is not running — the handler failed to start; check the plugin's handler.js and that Node.js is installed"
                ));
            }
            let id = st.next_id;
            st.next_id += 1;
            let (otx, orx) = oneshot::channel();
            st.pending.lock().await.insert(id, otx);
            (orx, st.tx.clone(), id)
        };

        let frame = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        })
        .to_string();
        if tx.send(format!("{frame}\n")).is_err() {
            let mut map = self.inner.lock().await;
            if let Some(st) = map.get_mut(skill_id) {
                st.pending.lock().await.remove(&id);
            }
            return Err(format!("sidecar for skill '{skill_id}' is not responding"));
        }

        match tokio::time::timeout(timeout, orx).await {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(_)) => {
                let mut map = self.inner.lock().await;
                if let Some(st) = map.get_mut(skill_id) {
                    st.pending.lock().await.remove(&id);
                }
                Err("sidecar response channel closed (handler exited?)".into())
            }
            Err(_) => {
                let mut map = self.inner.lock().await;
                if let Some(st) = map.get_mut(skill_id) {
                    st.pending.lock().await.remove(&id);
                }
                Err(format!(
                    "sidecar tool '{method}' timed out after {}s",
                    timeout.as_secs()
                ))
            }
        }
    }

    /// Run a tool on a skill's sidecar and unwrap the result into a string.
    pub async fn call_tool(
        &self,
        skill_id: &str,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<String, String> {
        let res = self
            .request(
                skill_id,
                "tools/call",
                serde_json::json!({ "name": name, "arguments": args }),
                Duration::from_secs(180),
            )
            .await?;
        // The response envelope is {jsonrpc, id, result}; tools/call semantics
        // live under result (isError + content).
        let payload = res
            .get("result")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        if payload.is_null() {
            // A handler that replies with the envelope itself (no result key)
            // is tolerated for leniency.
            let payload = res.clone();
            if payload
                .get("isError")
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
            {
                return Err(extract_text(&payload)
                    .unwrap_or_else(|| "sidecar reported an error".to_string()));
            }
            return Ok(extract_text(&payload)
                .unwrap_or_else(|| serde_json::to_string(&payload).unwrap_or_default()));
        }
        if payload
            .get("isError")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
        {
            return Err(
                extract_text(&payload).unwrap_or_else(|| "sidecar reported an error".to_string())
            );
        }
        Ok(extract_text(&payload)
            .unwrap_or_else(|| serde_json::to_string(&payload).unwrap_or_default()))
    }

    /// Shutdown hook: kill all sidecars (task side of Tauri's on-exit).
    pub fn shutdown(&self) {
        // kill_on_drop handles cleanup when processes exit; nothing to do
        // synchronously here — kept as a stable hook for future use.
    }
}

/// Convert a tools/call result into its text content: either
/// `content: [{type:"text", text:…}]`, a bare string, or the raw JSON.
fn extract_text(res: &serde_json::Value) -> Option<String> {
    match res.get("content") {
        Some(serde_json::Value::Array(arr)) => {
            let mut texts = Vec::new();
            for c in arr {
                match c.get("text").and_then(|t| t.as_str()) {
                    Some(t) => texts.push(t.to_string()),
                    None => texts.push(serde_json::to_string(c).unwrap_or_default()),
                }
            }
            Some(texts.join("\n"))
        }
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Spawn `node <handler>` with piped stdio, a writer task owning stdin, and a
/// reader task that routes stdout responses to the pending map by JSON-RPC id.
/// The returned Child MUST be kept alive by the caller — `kill_on_drop` fires
/// on drop.
async fn spawn_handler(
    skill_id: &str,
    handler: &std::path::Path,
) -> Result<
    (
        tokio::process::Child,
        u32,
        mpsc::UnboundedSender<String>,
        PendingCalls,
    ),
    String,
> {
    let mut cmd = Command::new("node");
    cmd.arg(handler);
    cmd.current_dir(handler.parent().unwrap_or(std::path::Path::new(".")));
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(|e| {
        format!("failed to start handler for skill '{skill_id}' (Node.js required): {e}")
    })?;
    let pid = child.id().ok_or("failed to get handler pid")?;

    let mut stdin = child.stdin.take().ok_or("handler stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("handler stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("handler stderr unavailable")?;

    let pending: PendingCalls = Arc::new(Mutex::new(HashMap::new()));

    // Writer: serializes outbound frames onto the child's stdin.
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            if stdin.write_all(frame.as_bytes()).await.is_err() {
                break;
            }
            let _ = stdin.flush().await;
        }
    });

    // Reader: stdout lines → JSON-RPC responses routed by id.
    let pending_r = pending.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) else {
                continue;
            };
            let Some(id) = v.get("id").and_then(|x| x.as_u64()) else {
                continue; // notification (no id) — not handled
            };
            let tx = pending_r.lock().await.remove(&id);
            if let Some(tx) = tx {
                let is_err = v.get("error").is_some();
                let _ = tx.send(if is_err {
                    Err(v["error"]["message"]
                        .as_str()
                        .unwrap_or("sidecar error")
                        .to_string())
                } else {
                    Ok(v)
                });
            }
        }
    });

    // stderr → dev log (handlers often print server logs here).
    let skill_e = skill_id.to_string();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            eprintln!("[sidecar:{skill_e}] {line}");
        }
    });

    Ok((child, pid, tx, pending))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Test-fixture snapshot of the frontend-dev plugin (the live copy
    /// lives OUTSIDE the repo at /mnt/Projects/pragma-plugins; tests must
    /// be self-contained so the plugin content is pinned here).
    #[allow(dead_code)]
    const FRONTEND_DEV_MANIFEST_JSON: &str = r#"
{
  "id": "frontend-dev",
  "name": "Frontend Developer",
  "version": "1.0.0",
  "description": "Frontend Developer skill: serves the working directory on a live localhost server with tool control",
  "contributes": {
    "providers": [],
    "slots": [],
    "commands": [],
    "skills": []
  }
}
"#;
    const FRONTEND_DEV_SKILL_MD: &str = r#"
# Frontend Developer skill

You work on frontend web code in the session's working directory. This skill
ships a live localhost static server whose tools let you control it.

## Workflow

1. **Serve**: call `fe_serve_start` to serve the session's working directory
   (pass `dir` to serve something else). The handler IS the server — one
   process, both the tool API and the thing the tools control. It's idempotent:
   a second start reports `alreadyRunning` with the existing port.
2. **Iterate**: edit files, then use `fe_serve_status` (port, state) and
   `fe_serve_logs` (recent requests) to verify the server is picking up your
   changes. You cannot browse the page yourself — verify by reading files and
   checking the log.
3. **Stop**: `fe_serve_stop` when the server is no longer needed.

## Rules

- Ports are ephemeral and reported by the tools — read them, never assume
  a fixed port.
- If no working directory is set, run `change_directory` first: the server
  serves the session's working directory.
- Localhost only, static files only — this is a preview server, not a
  production deployment.
"#;
    const FRONTEND_DEV_TOOLS_JSON: &str = r#"
[
  {
    "name": "fe_serve_start",
    "description": "Start a static file server for the session working directory (or `dir`) on an ephemeral localhost port. Returns the port and serving directory. Use this when the task involves previewing or iterating on frontend code — the handler IS the server.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "dir": {
          "type": "string",
          "description": "Directory to serve (defaults to the session working directory)"
        }
      }
    }
  },
  {
    "name": "fe_serve_status",
    "description": "List running servers: directory, port, started time. Also reports the session working directory.",
    "inputSchema": { "type": "object", "properties": {} }
  },
  {
    "name": "fe_serve_stop",
    "description": "Stop the server serving `dir` (defaults to the session working directory).",
    "inputSchema": {
      "type": "object",
      "properties": {
        "dir": {
          "type": "string",
          "description": "Directory whose server to stop (defaults to the session working directory)"
        }
      }
    }
  },
  {
    "name": "fe_serve_logs",
    "description": "Return the last `limit` log lines of the static server (requests served, errors).",
    "inputSchema": {
      "type": "object",
      "properties": {
        "limit": {
          "type": "integer",
          "description": "Number of recent log lines, default 50, max 200."
        }
      }
    }
  }
]
"#;
    const FRONTEND_DEV_HANDLER_JS: &str = r#"
// Frontend Developer skill sidecar — a tiny static file server that also
// answers the skill's tool calls over JSON-RPC 2.0 (newline-delimited JSON on
// stdin/stdout). One process, two roles: the tool API AND the thing the tools
// control. Depends only on Node's built-ins.
'use strict'

const http = require('http')
const fs = require('fs')
const path = require('path')
const readline = require('readline')

const servers = new Map() // abs dir -> { server, port, startedAt }
const logs = []           // ring buffer of recent access/status lines
const LOG_CAP = 200

function log(line) {
  logs.push(`[${new Date().toISOString()}] ${line}`)
  if (logs.length > LOG_CAP) logs.shift()
  console.error('[fe] ' + line) // host stderr — keeps stdout JSON-clean
}

const MIME = {
  '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript',
  '.css': 'text/css', '.json': 'application/json', '.svg': 'image/svg+xml',
  '.png': 'image/png', '.jpg': 'image/jpeg', '.gif': 'image/gif',
  '.webp': 'image/webp', '.ico': 'image/x-icon', '.txt': 'text/plain',
  '.md': 'text/markdown', '.map': 'application/json', '.wasm': 'application/wasm',
}

// Set by the initialize handshake — the session working directory.
let sessionDir = null

function startServer(dir) {
  const abs = path.resolve(dir)
  if (!fs.existsSync(abs)) {
    return Promise.resolve({ ok: false, error: `no such directory: ${abs}` })
  }
  if (servers.has(abs)) {
    const s = servers.get(abs)
    log(`reused server for ${abs}:${s.port}`)
    return Promise.resolve({ ok: true, alreadyRunning: true, port: s.port, dir: abs })
  }
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      const urlPath = decodeURIComponent((req.url || '/').split('?')[0])
      let file = path.join(abs, urlPath)
      if (!file.startsWith(abs)) {
        res.writeHead(403); res.end('forbidden'); return
      }
      fs.stat(file, (err, st) => {
        if (!err && st.isDirectory()) file = path.join(file, 'index.html')
        fs.readFile(file, (e2, data) => {
          if (e2) {
            res.writeHead(404); res.end('not found')
            log(`404 ${req.method} ${req.url}`)
            return
          }
          const ext = path.extname(file).toLowerCase()
          res.writeHead(200, { 'Content-Type': MIME[ext] || 'application/octet-stream' })
          res.end(data)
          log(`200 ${req.method} ${req.url} (${data.length}b)`)
        })
      })
    })
    server.on('error', (e) => {
      log(`server error: ${e.message}`)
      resolve({ ok: false, error: e.message })
    })
    server.listen(0, '127.0.0.1', () => {
      const port = server.address().port
      servers.set(abs, { server, port, startedAt: new Date().toISOString() })
      log(`serving ${abs} on http://127.0.0.1:${port}`)
      resolve({ ok: true, port, dir: abs })
    })
  })
}

function stopServer(dir) {
  const abs = path.resolve(dir)
  const entry = servers.get(abs)
  if (!entry) return { ok: true, alreadyStopped: true, dir: abs }
  entry.server.close()
  servers.delete(abs)
  log(`stopped ${abs}`)
  return { ok: true, dir: abs }
}

function status() {
  return {
    ok: true,
    sessionDir,
    servers: [...servers.entries()].map(([dir, s]) => ({
      dir, port: s.port, startedAt: s.startedAt,
    })),
  }
}

// ── JSON-RPC plumbing ──────────────────────────────────────────────────────
function reply(id, payload, isError) {
  const result = {
    content: [
      { type: 'text', text: typeof payload === 'string' ? payload : JSON.stringify(payload, null, 2) },
    ],
    isError: !!isError,
  }
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\n')
}

function replyErr(id, message) {
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, error: { code: -1, message } }) + '\n')
}

const rl = readline.createInterface({ input: process.stdin })
rl.on('line', (line) => {
  let msg
  try { msg = JSON.parse(line) } catch { return }
  const { id } = msg
  if (msg.method === 'initialize') {
    sessionDir = (msg.params && msg.params.workingDir) || null
    log(`initialized (workingDir=${sessionDir || 'none'})`)
    reply(id, { protocolVersion: 1, tools: require('./tools.json') }, false)
  } else if (msg.method === 'tools/call') {
    const { name, arguments: args } = msg.params
    const a = args || {}
    try {
      if (name === 'fe_serve_start') {
        const dir = a.dir || sessionDir
        if (!dir) {
          reply(id, { ok: false, error: 'no working directory — call change_directory first, or pass "dir"' }, true)
          return
        }
        startServer(dir).then((r) => reply(id, r, !r.ok))
      } else if (name === 'fe_serve_status') {
        reply(id, status(), false)
      } else if (name === 'fe_serve_stop') {
        const dir = a.dir || sessionDir
        if (!dir) {
          reply(id, { ok: false, error: 'specify "dir" or set a working directory first' }, true)
          return
        }
        reply(id, stopServer(dir), false)
      } else if (name === 'fe_serve_logs') {
        const n = Math.max(1, Math.min(LOG_CAP, (a.limit || 50) | 0))
        reply(id, { ok: true, logs: logs.slice(-n) }, false)
      } else {
        replyErr(id, `unknown tool: ${name}`)
      }
    } catch (e) {
      replyErr(id, `handler error: ${e.message}`)
    }
  }
})

process.on('exit', () => {
  for (const [, s] of servers) s.server.close()
})
"#;

    const ECHO_HANDLER: &str = r#"
const readline = require('readline');
const rl = readline.createInterface({ input: process.stdin });
function reply(id, result) {
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\n');
}
function replyErr(id, message) {
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, error: { code: -1, message } }) + '\n');
}
rl.on('line', (line) => {
  const msg = JSON.parse(line);
  if (msg.method === 'initialize') {
    reply(msg.id, { protocolVersion: 1, tools: [{ name: 'echo', description: 'Echo args', inputSchema: { type: 'object', properties: { s: { type: 'string' } } } }] });
  } else if (msg.method === 'tools/call') {
    const args = msg.params.arguments || {};
    if (msg.params.name === 'echo') {
      reply(msg.id, { content: [{ type: 'text', text: 'echo:' + (args.s || '') }], isError: false });
    } else if (msg.params.name === 'boom') {
      reply(msg.id, { content: [{ type: 'text', text: 'kaboom' }], isError: true });
    } else {
      replyErr(msg.id, 'no such tool: ' + msg.params.name);
    }
  }
});
"#;

    fn node_available() -> bool {
        std::process::Command::new("node")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn make_plugin(handler: &str, tools: &str) -> (PluginManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let sk = Path::new(dir.path()).join("tp/skills/echo");
        std::fs::create_dir_all(&sk).unwrap();
        std::fs::write(
            dir.path().join("tp/manifest.json"),
            r#"{"id":"tp","name":"TP","version":"1","description":"test plugin"}"#,
        )
        .unwrap();
        std::fs::write(sk.join("skill.md"), "doc").unwrap();
        std::fs::write(sk.join("handler.js"), handler).unwrap();
        std::fs::write(sk.join("tools.json"), tools).unwrap();
        (PluginManager::new(dir.path().to_path_buf(), vec![], vec![]), dir)
    }

    #[tokio::test]
    async fn sidecar_round_trip_via_real_node() {
        if !node_available() {
            eprintln!("node not found — skipping sidecar integration test");
            return;
        }
        let (mgr, _dir) = make_plugin(
            ECHO_HANDLER,
            r#"[
                {"name":"echo","description":"Echo","inputSchema":{"type":"object","properties":{"s":{"type":"string"}}}},
                {"name":"boom","description":"Always errors","inputSchema":{"type":"object","properties":{}}}
            ]"#,
        );
        let reg = SkillSidecarRegistry::new();
        reg.ensure(&mgr, "tp/echo", Some("/tmp/work"))
            .await
            .unwrap();

        // routing index built from tools.json
        assert_eq!(reg.owner("echo").as_deref(), Some("tp/echo"));
        assert_eq!(reg.owner("boom").as_deref(), Some("tp/echo"));
        assert_eq!(reg.owner("read_file"), None);

        // happy path
        let out = reg
            .call_tool("tp/echo", "echo", &serde_json::json!({"s": "hi"}))
            .await
            .unwrap();
        assert_eq!(out, "echo:hi");

        // isError → Err with the message text
        let err = reg
            .call_tool("tp/echo", "boom", &serde_json::json!({}))
            .await;
        assert!(
            err.as_ref().is_err_and(|e| e.contains("kaboom")),
            "err: {err:?}"
        );

        // unknown tool → handler error surfaced
        let err = reg
            .call_tool("tp/echo", "nope", &serde_json::json!({}))
            .await;
        assert!(
            err.as_ref().is_err_and(|e| e.contains("no such tool")),
            "err: {err:?}"
        );

        // stop → calls fail fast, not "unknown tool"
        reg.stop("tp/echo").await;
        assert_eq!(reg.owner("echo"), None);
        let err = reg
            .call_tool("tp/echo", "echo", &serde_json::json!({}))
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn missing_handler_routes_no_shell() {
        let (mgr, _dir) = make_plugin("", r#"[]"#);
        // no handler.js — ensure errors, `sk/echo` tools stay unregistered
        std::fs::remove_file(mgr.skill_handler_path("tp/echo").unwrap()).unwrap();
        let reg = SkillSidecarRegistry::new();
        let err = reg.ensure(&mgr, "tp/echo", None).await;
        assert!(err.is_err(), "expected error, got {err:?}");
        assert_eq!(reg.owner("echo"), None);
    }

    #[test]
    fn parse_tools_handles_all_shapes() {
        let v = serde_json::json!([
            {"name": "a", "description": "A", "inputSchema": {"type":"object","properties":{}}},
            {"name": "b"} // no schema → default
        ]);
        let tools = parse_tools(Some(&v));
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "a");
        assert_eq!(tools[1].input_schema["type"], "object");
        assert!(parse_tools(None).is_empty());
    }

    /// Copy the shipped frontend-dev demo plugin into a temp dir.
    fn write_frontend_dev_plugin(dir: &Path) {
        let plugin = dir.join("frontend-dev");
        let sk = plugin.join("skills/frontend-dev");
        std::fs::create_dir_all(&sk).unwrap();
        std::fs::write(plugin.join("manifest.json"), FRONTEND_DEV_MANIFEST_JSON).unwrap();
        std::fs::write(sk.join("skill.md"), FRONTEND_DEV_SKILL_MD).unwrap();
        std::fs::write(sk.join("tools.json"), FRONTEND_DEV_TOOLS_JSON).unwrap();
        std::fs::write(sk.join("handler.js"), FRONTEND_DEV_HANDLER_JS).unwrap();
    }

    #[tokio::test]
    async fn frontend_dev_skill_end_to_end() {
        if !node_available() {
            eprintln!("node not found — skipping frontend-dev e2e test");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        write_frontend_dev_plugin(dir.path());
        let mgr = PluginManager::new(dir.path().to_path_buf(), vec![], vec![]);

        let skill = mgr
            .skill("frontend-dev/frontend-dev")
            .expect("skill discovered");
        assert!(skill.has_handler && skill.has_tools);

        // A work dir with one file to serve.
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("index.html"), "<h1>hello</h1>").unwrap();

        let reg = SkillSidecarRegistry::new();
        reg.ensure(
            &mgr,
            "frontend-dev/frontend-dev",
            Some(work.path().to_str().unwrap()),
        )
        .await
        .unwrap();

        // Routing index exposes the skill's tools.
        assert_eq!(
            reg.owner("fe_serve_start").as_deref(),
            Some("frontend-dev/frontend-dev")
        );

        // Start the server (sessions dir comes from initialize).
        let out = reg
            .call_tool(
                "frontend-dev/frontend-dev",
                "fe_serve_start",
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).expect("json result");
        let port = v["port"].as_u64().expect("port in start result");

        // Status reports the session dir + running server.
        let st = reg
            .call_tool(
                "frontend-dev/frontend-dev",
                "fe_serve_status",
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        assert!(
            st.contains("sessionDir") && st.contains(&port.to_string()),
            "status: {st}"
        );

        // Fetch through the LIVE server.
        let body = tokio::task::spawn_blocking(move || {
            use std::io::{Read, Write};
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port as u16)).unwrap();
            s.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .unwrap();
            let mut buf = String::new();
            s.read_to_string(&mut buf).unwrap();
            buf
        })
        .await
        .unwrap();
        assert!(body.contains("<h1>hello</h1>"), "served content: {body}");

        // The GET landed in the handler's log ring.
        let lg = reg
            .call_tool(
                "frontend-dev/frontend-dev",
                "fe_serve_logs",
                &serde_json::json!({ "limit": 5 }),
            )
            .await
            .unwrap();
        assert!(lg.contains("200 GET /"), "logs: {lg}");

        // Stop idempotently.
        let stp = reg
            .call_tool(
                "frontend-dev/frontend-dev",
                "fe_serve_stop",
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        assert!(!stp.contains("error"), "stop: {stp}");

        // Unknown tool → handler error.
        let bad = reg
            .call_tool("frontend-dev/frontend-dev", "nope", &serde_json::json!({}))
            .await;
        assert!(bad.is_err() && bad.as_ref().unwrap_err().contains("unknown tool"));
    }

    /// pragma-dev is a first-party plugin committed INSIDE the repo
    /// (plugins/pragma-dev), so the e2e test exercises the real tree rather
    /// than a pinned fixture — the whole point of the plugin is tracking the
    /// code.
    fn pragma_dev_plugin_dir() -> Option<std::path::PathBuf> {
        // CARGO_MANIFEST_DIR = <repo>/src-tauri; parent = repo root.
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent()?;
        let dir = repo.join("plugins/pragma-dev");
        dir.join("manifest.json").is_file().then_some(dir)
    }

    #[tokio::test]
    async fn pragma_dev_skill_end_to_end() {
        if !node_available() {
            eprintln!("node not found — skipping pragma-dev e2e");
            return;
        }
        let Some(dir) = pragma_dev_plugin_dir() else {
            eprintln!("plugins/pragma-dev not present — skipping pragma-dev e2e");
            return;
        };
        // dir = <repo>/plugins/pragma-dev → plugins dir = dir.parent(), repo = grandparent.
        let plugins_dir = dir.parent().unwrap();
        let repo = plugins_dir.parent().unwrap();
        let mgr = PluginManager::new(plugins_dir.to_path_buf(), vec![], vec![]);

        let skill = mgr
            .skill("pragma-dev/pragma-dev")
            .expect("skill discovered");
        assert!(skill.has_handler && skill.has_tools);

        let reg = SkillSidecarRegistry::new();
        reg.ensure(&mgr, "pragma-dev/pragma-dev", Some(repo.to_str().unwrap()))
            .await
            .unwrap();

        // status() snapshot reflects the running sidecar (new bridge surface).
        let st = reg.status().await;
        assert!(st
            .iter()
            .any(|s| s.skill_id == "pragma-dev/pragma-dev" && s.running));

        // Convention verifier against the real tree: a fully-wired command
        // passes every check.
        let out = reg
            .call_tool(
                "pragma-dev/pragma-dev",
                "pd_verify_command",
                &serde_json::json!({"name": "list_providers"}),
            )
            .await
            .unwrap();
        assert!(out.contains("\"ok\": true"), "verify: {out}");
        assert!(
            out.contains("tauri.ts wrapper"),
            "verify should check all wiring: {out}"
        );

        // Verify_tool on a real tool (the ledger-aware edit_file, dispatched
        // inline in agentic.rs).
        let vt = reg
            .call_tool(
                "pragma-dev/pragma-dev",
                "pd_verify_tool",
                &serde_json::json!({"name": "edit_file"}),
            )
            .await
            .unwrap();
        assert!(vt.contains("\"ok\": true"), "verify_tool: {vt}");

        // A command that is NOT wired fails loudly on the missing points.
        let missing = reg
            .call_tool(
                "pragma-dev/pragma-dev",
                "pd_verify_command",
                &serde_json::json!({"name": "no_such_command_xyz"}),
            )
            .await
            .unwrap();
        assert!(
            missing.contains("\"ok\": false"),
            "missing command should fail: {missing}"
        );

        // dev_status works with zero dev server started (sidecar just up).
        let st = reg
            .call_tool(
                "pragma-dev/pragma-dev",
                "pd_dev_status",
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        assert!(st.contains("\"running\": false"), "status: {st}");

        // pd_todo scans the real tree and returns the file-grouped summary.
        let todo = reg
            .call_tool(
                "pragma-dev/pragma-dev",
                "pd_todo",
                &serde_json::json!({"limit": 5}),
            )
            .await
            .unwrap();
        assert!(todo.contains("\"total\""), "todo: {todo}");

        // pd_reload_app on a dir WITHOUT a Cargo manifest fails fast (no cargo
        // invocation) and never sets the relaunch marker.
        let work = tempfile::tempdir().unwrap();
        let rl = reg
            .call_tool(
                "pragma-dev/pragma-dev",
                "pd_reload_app",
                &serde_json::json!({"dir": work.path()}),
            )
            .await
            .unwrap();
        assert!(rl.contains("\"ok\": false"), "reload fast-fail: {rl}");
        assert!(
            rl.contains("\"reloadApp\": false"),
            "no relaunch marker on failure: {rl}"
        );

        // Unknown tool → handler error.
        let bad = reg
            .call_tool("pragma-dev/pragma-dev", "nope", &serde_json::json!({}))
            .await;
        assert!(bad.is_err() && bad.as_ref().unwrap_err().contains("unknown tool"));

        reg.stop("pragma-dev/pragma-dev").await;
        assert!(reg.status().await.is_empty());
    }
}
