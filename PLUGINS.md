# Pragma Plugin Reference

This document describes the Pragma plugin system for AI models and developers
building plugins. A plugin is a folder at `~/.config/dev.pragma.app/plugins/<id>/`.

> **Developing a plugin:** keep your plugin folder *outside* any git repo (e.g.
> `/mnt/Projects/pragma-plugins/`) and symlink it into the plugins dir —
> `ln -s /path/to/your-plugin ~/.config/dev.pragma.app/plugins/your-plugin`.
> The app resolves through the symlink and the file watcher follows it, so edits
> hot-reload without any copy step. The Rust backend never creates or copies
> plugin folders; what's in the config dir is the source of truth. The example
> plugins (`demo`, `frontend-dev`) live this way today.
>
> **First-party exception — `pragma-dev`:** the pragma-dev plugin is committed
> *in* the pragma repo at `plugins/pragma-dev/` (symlinked into the config dir).
> Its skill.md is an architecture reference for the codebase, so it must travel
> with code changes; the e2e test loads it from the repo tree rather than a
> pinned fixture. Follow this pattern only for plugins whose content must track
> the repo.

## The Plugin Lifecycle

### Where plugins live

Every plugin is a **folder** under the app's plugin directory:
`~/.config/dev.pragma.app/plugins/<id>/`. The folder name must match the `id`
in `manifest.json`. What's on disk is the source of truth — the app never
creates or copies plugin folders, it reads them (and hot-reloads on change).

### Installing

Install by **source**, not by archive — the same model pi and OpenCode use.
Settings → Plugins → **Install…** accepts:

- `git:https://github.com/user/repo` — shallow-cloned with git. A tag is
  supported: `git:github.com/user/repo@v1.0.0` (bare `https://…`/`ssh://…`
  URLs work too).
- `npm:my-plugin` — fetched from the npm registry
  (`npm:@scope/pkg`, `npm:pkg@1.2.3`).
- a **local folder** path — installed as a symlink, so edits to the source
  folder hot-reload live (the dev workflow).
- a local **`.zip`** — still accepted for file drops.

The backend fetches the source, validates the manifest, and swaps the folder
in atomically — a bad package can never leave a half-installed plugin. The
plugin is **enabled** on install, and its source spec is recorded in
`plugin_sources.json` (app config dir) so a future update checker knows where
each plugin came from. Re-installing an existing id replaces the old folder —
which is how updates land.

Alternatively, **drop a folder** into `plugins/` manually — the file watcher
picks it up and hot-reloads. A dropped-in folder starts *disabled* (first-time
plugins need an explicit enable in Settings → Plugins).

### Publishing

Publishing a plugin = putting it **somewhere fetchable** and telling people
where. Two first-class options, matching how pi/OpenCode plugins are shipped:

1. **A git repository** — the plugin folder (with `manifest.json` at the root,
   or under a `plugins/<id>/` subfolder) in any repo. Users install it with
   `git:<url>`; tag releases with `vX.Y.Z` for version pinning.
2. **An npm package** — publish the plugin folder as a package containing
   `manifest.json`. Users install it with `npm:<name>`.

The manifest's optional metadata makes a plugin findable and attributable:

```json
{
  "id": "my-plugin",
  "name": "My Plugin",
  "version": "1.0.0",
  "author": "You",
  "publisher": "your-handle",
  "repository": "https://github.com/you/my-plugin",
  "homepage": "https://you.dev/my-plugin",
  "description": "…"
}
```

- `author` / `repository` / `homepage` are shown in Settings → Plugins.
- **`publisher` namespaces plugins** without touching the flat id scheme:
  first-party plugins use `"publisher": "pragma"` and get a **Pragma** badge
  in Settings → Plugins; third parties use their own handle. Ecosystem-level
  namespacing comes from the **source** itself — an npm scope
  (`npm:@you/my-plugin`), a git owner (`git:github.com/you/repo`) — exactly
  like pi packages and npm scopes; the id stays a clean identifier because it
  maps 1:1 to folder names and skill ids (`<plugin>/<skill>`).

### Updates

Pragma **checks for plugin updates automatically at startup** (at most every
10 minutes, silently — it never interrupts). Settings → Plugins → **Check for
updates** re-checks on demand. When newer versions exist you see them in two
places:

- a **count badge** next to **Plugins** in the Settings sidebar
- per-plugin **`vX → vY`** badges with an **Update** button on the plugins page

How versions are found:

- **git sources**: `git ls-remote --tags` picks the highest `vX.Y.Z` tag.
- **npm sources**: the registry's `latest` dist-tag.
- **local folders** are live symlinks — always current, no polling.
- **version-pinned sources** (`git:…@tag`, `npm:pkg@ver`) are deliberately
  skipped, matching pi's pinned-ref semantics: unpin (drop the `@tag`/
  `@version`) to receive updates.

Manual/dropped-in folders have no recorded source and are skipped.

### The plugin registry (single-repo marketplace)

Every Pragma plugin can be served from **one GitHub repo** — e.g.
`Sylviromi/plugins` — which doubles as both the storefront and the archive:

```
Sylviromi/plugins
  plugins/<id>/            one folder per plugin (the source of truth)
    manifest.json
    …
  index.json               generated registry — what the app fetches
  scripts/generate-plugin-index.js
```

**How it works:**

1. Plugin authors add a folder under `plugins/<id>/` and tag releases
   **per plugin** — `git tag <id>-v1.2.3` (tags are repo-wide, so they're
   prefixed to stay unambiguous).
2. A tiny script scans the folder and generates `index.json`:
   `node scripts/generate-plugin-index.js plugins Sylviromi/plugins index.json`
   (wire it into CI so the index is always fresh).
3. The app's Settings → Plugins → **Browse community plugins** fetches
   `https://raw.githubusercontent.com/Sylviromi/plugins/main/index.json` and
   lists every plugin with an Install button.
4. Each entry's `source` encodes the whole install recipe — the
   `#plugins/<id>` subpath makes the installer **sparse-checkout** only that
   folder from the shared repo, so a monorepo can serve individual plugins:

   ```
   git:https://github.com/Sylviromi/plugins@<id>-v1.2.3#plugins/<id>
   ```

5. Update checks work the same as any git source: tags are filtered to the
   plugin's own prefix (`<id>-v…`), so updating one plugin never crosses
   into another's versions.

**Install by hand** without the browse screen, from the same repo:
`git:https://github.com/Sylviromi/plugins#plugins/<id>` (unpinned = latest
release tag) or with an explicit tag. The subpath syntax works for any
git source, not just the official registry.

A future **plugin registry** — a static JSON index hosted anywhere (a GitHub
repo, GitHub Pages, any CDN) listing `{ id, name, version, description,
  author, downloadUrl }` — will let the app show a “Browse plugins” screen:
installing from the registry reuses the exact same validated install path as
manual sources. Roadmap item; the installer and source records are already in
place for it.

## Plugin Directory Structure

```
~/.config/dev.pragma.app/plugins/<id>/
  manifest.json       REQUIRED — declares id, name, version, and what the plugin contributes
  provider.json       optional — registers an OpenAI-compatible AI provider
  component.js        optional — ES module Web Component for UI slots
  skill.md            optional — legacy: a single on-demand skill (skill id = plugin id)
  skills/<skill>/     optional — the modern layout: one folder per skill
    skill.md          the skill's reference material (loaded on demand by the model)
    tools.json        optional — tool schemas the skill contributes
    handler.js        optional — Node sidecar implementing those tools (JSON-RPC over stdio)
  config.json         optional — plugin-scoped persistent configuration (managed via commands)
```

## manifest.json (required)

Every plugin must have a `manifest.json`. The folder name must match the `id` field exactly.

```json
{
  "id": "my-plugin",
  "name": "My Plugin",
  "version": "1.0.0",
  "description": "What this plugin does",
  "contributes": {
    "providers": ["my-plugin"],
    "slots": ["status-bar", "settings-page"],
    "commands": [],
    "skills": ["skill.md"]
  }
}
```

### Validation Rules

- `id`: lowercase, no spaces, no `/` or `\`. Must match the folder name.
- `name`, `version`: required, non-empty.
- `contributes.providers`: list provider IDs that match `provider.json`.
- `contributes.slots`: list UI slot names this plugin renders into.
- `contributes.skills`: legacy field, kept for compatibility. Skills are now
  discovered from the plugin's `skills/` folder automatically — you do not need
  to list them in the manifest (see [Skills & Tools](#skills--tools)).

## provider.json (optional)

Registers an OpenAI-compatible provider so the plugin's models appear in the
model picker.

```json
{
  "id": "my-plugin",
  "name": "My Provider",
  "base_url": "http://localhost:1234/v1",
  "auth": "none",
  "discover": false,
  "reasoningEffort": "medium",
  "models": [
    {
      "id": "model-id",
      "name": "Display Name",
      "contextWindow": 32768,
      "isFree": true,
      "isVision": false,
      "maxTokens": 32000,
      "reasoningEffort": "high",
      "supportsThinking": true,
      "supportsThinkingLevels": true,
      "thinkingBudget": 8192
    }
  ]
}
```

- `id`: must be listed in `manifest.json`'s `contributes.providers`.
- `base_url`: must start with `http://` or `https://`.
- `auth`: `"api-key"` (reads key from Settings > Providers) or `"none"`.
- `discover`: if `true`, fetches `{base_url}/models` at startup to populate the model list.
- `reasoningEffort` (optional): provider-level reasoning effort — `"low"` / `"medium"` /
  `"high"`. Sent as the OpenAI-compat `reasoning_effort` request field on every call.
  Use for reasoning-capable endpoints (gpt-5, Qwen3, GLM-z1, local relays). Omit when
  the upstream rejects unknown params or reasons by default (e.g. `deepseek-reasoner`).
- `models`: static fallback model list. Per model (all optional):
  - `maxTokens`: output-token ceiling sent as `max_tokens` (default 16384).
  - `reasoningEffort`: overrides the provider-level `reasoningEffort` for this model.
  - `supportsThinking`: declare that the model supports extended thinking / reasoning.
    This is how **custom providers opt in to thinking** — Anthropic-family endpoints
    get the `thinking` request block (with `thinkingBudget`), OpenAI-compat endpoints
    get `reasoning_effort`. When omitted, known families are detected automatically
    (claude-opus-4 / sonnet-4-5 / 4-6 / 3-7, gpt-5, o-series, deepseek-reasoner,
    qwen3, glm-z1) and everything else defaults to off.
  - `thinkingBudget`: Anthropic `budget_tokens` for this model (default 8192). Only
    meaningful when `supportsThinking` is true.
  - `supportsThinkingLevels`: declare that the model supports *graduated* thinking
    levels (Anthropic `thinking_level` on Sonnet 4.5+ / Opus 4.x; `reasoning_effort`
    on OpenAI-compat reasoning families). Omit and the harness detects known
    families automatically; thinking-capable models without levels (e.g. Claude
    3.7, budget-only) ignore explicit levels.

The user-facing **Thinking** selector in the model picker (Off · Auto · Low · Med ·
High) never forces thinking onto a model that doesn't support it. `Off` disables
thinking entirely (cost control); `Auto` sends no level (API default; OpenAI-compat
ends up using the configured `reasoningEffort`); `Low`/`Med`/`High` are sent only
when the model supports graduated levels.

## component.js (optional)

A plain ES module (no build step) whose default export is an `HTMLElement` subclass.
The component is instantiated once per slot it claims in the manifest.

```javascript
export default class MyPlugin extends HTMLElement {
  connectedCallback() {
    const slot = this.getAttribute('data-slot')  // e.g. 'status-bar'
    this.attachShadow({ mode: 'open' })
    this.shadowRoot.innerHTML = '<div>Hello from my plugin</div>'
  }

  disconnectedCallback() {
    // Clean up timers, listeners, etc.
  }
}
```

### Available Slots

Slots fall into two kinds:

- **Anchored slots** — the app places these in fixed layout positions. The
  table below lists the built-in ones.
- **Free slots** — any slot name NOT in the table below. A plugin claiming
  one is mounted in a global free-floating layer instead; the component
  positions itself anywhere on screen with `position: fixed` (remember to set
  `pointer-events: auto` on your `:host` — the layer is transparent to
  clicks). This is how a plugin takes arbitrary UI space without any vanilla
  change: claim a new slot name (e.g. `"pets"` for a pet that walks across
  the bottom of the screen) and render whatever you want.

| Slot | Location | Use Case |
|------|----------|----------|
| `chat-view` | Replaces the whole message list | Full custom conversation surface (owns the composer too — when a plugin claims it, the default `InputArea` is hidden) |
| `input-toolbar` | Next to the message input | Quick action buttons |
| `chat-toolbar-extra` | Right side of chat controls | Extra chat actions |
| `model-picker-extra` | Below model selector | Provider info, status |
| `status-bar` | Strip below the tab bar | Live status indicators |
| `composer-center` | Middle of the composer footer | Mode toggles |
| `sidebar-section` | Bottom of the telemetry sidebar | Stats, logs |
| `settings-page` | Plugin's own page at `/settings/plugin/<id>` | Configuration UI |

### Calling Tauri Commands

Plugins can call any registered Tauri command:

```javascript
const { invoke } = window.__TAURI__.core
const result = await invoke('command_name', { arg1: 'value' })
```

### Rendering Markdown Like the App

The host exposes its own markdown renderer (the `marked`-based one from
`src/lib/markdown.ts`) on `window.__pragmaMarkdown` so plugin bubbles can render
model text identically to the built-in chat — no need to vendor a parser:

```javascript
const md = window.__pragmaMarkdown  // { renderMarkdown, segmentsOf, withCaret }
const segs = md.segmentsOf(text)     // split fenced code blocks from prose
for (const seg of segs) {
  if (seg.kind === 'code')  /* render <pre> with escaped text */
  else                      html += md.renderMarkdown(seg.text)  // safe HTML
}
```

`renderMarkdown` HTML-escapes input and sanitizes links/images (http/https/mailto
only) exactly like the built-in chat. The bridge is installed once at app mount;
check `typeof md.renderMarkdown === 'function'` before using it and fall back to
plain text when absent (e.g. very old builds).

### Listening to Events

```javascript
const { listen } = window.__TAURI__.event
const unlisten = await listen('event-name', (e) => {
  console.log(e.payload)
})
// Call unlisten() in disconnectedCallback
```

## skill.md (optional)

Skills are **loaded on demand**: the model sees a skill index in its system
prompt and calls the `load_skill` tool when a skill seems relevant. Loading a
skill injects its reference material AND makes any tools it ships available to
the model for the rest of the session — so a git skill's tools only appear when
the model actually needs git.

The legacy layout (a single top-level `skill.md`) still works; its skill id is
the plugin id. The modern layout puts each skill in its own folder:

```
~/.config/dev.pragma.app/plugins/<id>/skills/<skill-id>/
  skill.md      required — the reference material, injected on load_skill
  tools.json    optional — tool schemas this skill contributes
  handler.js    optional — Node sidecar implementing those tools
```

A skill id is `<plugin-id>/<skill-id>` (e.g. `fe/frontend-dev`); a legacy
plugin-level skill uses just the plugin id. Load any skill id with
`load_skill`.

Keep `skill.md` short and non-prescriptive. It should read as reference
material, not as a persona.

```markdown
# Frontend Dev Context

Work on frontend code. The dev server is started with `frontend_serve_start`
and exposes: frontend_serve_status, frontend_serve_logs.
```

## Skills & Tools

A skill can contribute tools through its `tools.json`. The model sees these
schemas only after the skill is loaded, so the model never carries tool
surface it hasn't opted into.

### tools.json (optional)

A JSON array of tool definitions:

```json
[
  {
    "name": "frontend_serve_status",
    "description": "Is the dev server running? Returns the port and process state.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "detail": { "type": "boolean", "description": "Include recent log lines" }
      }
    }
  }
]
```

- `name` is the tool name the model calls (unique across all tools).
- `description` tells the model when/how to use it.
- `inputSchema` is a JSON Schema for arguments (defaults to an empty object
  schema when omitted).

### handler.js (optional) — the sidecar

If the skill ships `handler.js`, the host spawns it once with `node` (the
skill folder is the working directory) when the skill is first loaded, and
keeps it running for the app's lifetime (killed on app exit, plugin hot-reload,
or when the plugin is disabled). Tools are **executed by the handler** over
JSON-RPC 2.0 with newline-delimited JSON on stdin/stdout.

The sidecar can BE your service — e.g. a dev server running inside the same
Node process that answers tool calls. That is the supported pattern: one
process, both the tool API and the thing the tools control.

#### Wire protocol

```text
host → child:  {"jsonrpc":"2.0","id":1,"method":"initialize",
                "params":{"skillId":"fe/frontend-dev","workingDir":"/abs/path"}}
child → host:  {"jsonrpc":"2.0","id":1,
                "result":{"protocolVersion":"1","tools":[{name,description,inputSchema},…]}}
host → child:  {"jsonrpc":"2.0","id":2,"method":"tools/call",
                "params":{"name":"frontend_serve_status","arguments":{…}}}
child → host:  {"jsonrpc":"2.0","id":2,
                "result":{"content":[{"type":"text","text":"…"}],"isError":false}}
child → host:  {"jsonrpc":"2.0","id":2,"error":{"code":-1,"message":"…"}}
```

- `initialize` is sent once when the sidecar starts. Reply promptly with
  `protocolVersion` and (optionally) your actual tool list — the host uses
  `tools.json` for the model-facing schemas and the echo only as a
  cross-check.
- `tools/call` carries `name` + `arguments`. Reply with either
  `{content: [{type: "text", text}]}` (multiple text blocks are joined) or an
  `error` object.
- Set `"isError": true` in the result when the call failed semantically —
  the text becomes the tool error the model sees.
- Every request has an `id` you must echo back. Ignore messages without one.
- Keep stdout clean: only newline-delimited JSON responses. Use `console.error`
  for logs (the host forwards them to its own stderr).

Minimal handler sketch:

```javascript
const readline = require('readline')
const rl = readline.createInterface({ input: process.stdin })

function reply(id, result) {
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\n')
}

rl.on('line', (line) => {
  const msg = JSON.parse(line)
  if (msg.method === 'initialize') {
    reply(msg.id, { protocolVersion: 1, tools: require('./tools.json') })
  } else if (msg.method === 'tools/call') {
    const { name, arguments: args } = msg.params
    if (name === 'frontend_serve_status') {
      reply(msg.id, { content: [{ type: 'text', text: 'running on :5173' }], isError: false })
    } else {
      reply(msg.id, { content: [{ type: 'text', text: 'unknown tool: ' + name }], isError: true })
    }
  }
})
```

## config.json (plugin configuration)

Plugins can persist their own configuration using these Tauri commands:

### Reading Config

```javascript
const config = await invoke('get_plugin_config', { pluginId: 'my-plugin' })
// Returns {} if no config.json exists
```

### Writing Config

```javascript
await invoke('set_plugin_config', {
  pluginId: 'my-plugin',
  config: { setting1: 'value', setting2: true }
})
// Full overwrite — merge before calling if you want partial updates
```

Config is stored as `config.json` in the plugin's directory.

## Process Management (Sidecar Plugins)

Plugins that need to run a background process (proxy server, language server,
etc.) can use the managed process commands. The host captures stdout/stderr and
streams them as events.

### Spawning a Process

```javascript
const pid = await invoke('spawn_plugin_process', {
  pluginId: 'my-plugin',
  program: 'node',
  args: ['server.js'],
  cwd: '/path/to/working/dir'  // or null
})
```

- Only one process per plugin at a time.
- Plugin must be enabled.
- Process is killed when the app exits (`kill_on_drop`).

### Stopping a Process

```javascript
await invoke('stop_plugin_process', { pluginId: 'my-plugin' })
```

### Checking Status

```javascript
const info = await invoke('plugin_process_status', { pluginId: 'my-plugin' })
// { running: true, pid: 12345 }
```

### Receiving Stdout/Stderr

The host reads stdout/stderr line-by-line and emits events:

```javascript
const { listen } = window.__TAURI__.event

// Each line from the process's stdout
await listen('plugin:stdout:my-plugin', (e) => {
  const line = e.payload  // raw string
  const data = JSON.parse(line)  // if your process outputs JSON
})

// Each line from stderr
await listen('plugin:stderr:my-plugin', (e) => {
  console.warn(e.payload)
})

// Fired when the process exits
await listen('plugin:process-exit:my-plugin', () => {
  console.log('process stopped')
})
```

### Writing to Stdin

For bidirectional IPC, write to the process's stdin:

```javascript
await invoke('write_plugin_stdin', {
  pluginId: 'my-plugin',
  data: JSON.stringify({ command: 'reload' })
})
```

A newline is appended automatically if not present.

## Confirmation Dialogs

Plugins can request user confirmation via a modal popup:

```javascript
const accepted = await invoke('request_plugin_confirmation', {
  pluginId: 'my-plugin',
  title: 'Confirm Action',
  message: 'Are you sure you want to do this?'
})
// accepted: true (user clicked Accept) or false (Dismiss / timeout after 60s)
```

This is an async command — it blocks until the user responds. Useful for sidecar
plugins that need user consent before taking an action (e.g., cycling a VPN
identity).

### Sidecar Confirmation Pattern

For processes that need confirmation mid-operation:

1. Process emits a JSON line to stdout: `{"event":"confirm_action",...}`
2. Component.js receives via `plugin:stdout:my-plugin`
3. Component.js calls `request_plugin_confirmation`
4. On result, writes back via `write_plugin_stdin`
5. Process reads stdin and proceeds or aborts

## Design Tokens

All UI (including plugin Shadow DOM) can theme against CSS custom properties set
on `<html>`. Core tokens:

```
--bg-base, --bg-elevated, --bg-overlay
--surface-hover
--text-primary, --text-secondary, --text-muted
--border-default, --border-subtle, --border-focus
--accent, --accent-soft, --accent-on, --accent-hover, --accent-active
--success, --warning, --danger
--shadow-sm, --shadow-md, --shadow-lg
--radius-sm, --radius-md, --radius-lg
--font-heading, --font-body, --font-mono
```

Theme families: `steel`, `forest`, `amber`, `violet`, `high-contrast`.
Modes: `dark`, `light`.

Legacy `--color-*` aliases (e.g., `--color-bg-base`, `--color-accent-brand`)
still resolve to the same values for backwards compatibility.

## Hot-Reload

A file watcher monitors the plugins directory. Any file change triggers:

1. All plugin providers are unregistered and re-registered
2. A `plugin:reload` event is emitted to the frontend
3. Plugin components are re-imported and re-mounted

No app restart needed during development.

## Example: Minimal Plugin

A plugin that just adds a status indicator:

```
my-status/
  manifest.json
  component.js
```

**manifest.json:**
```json
{
  "id": "my-status",
  "name": "My Status",
  "version": "1.0.0",
  "contributes": { "slots": ["status-bar"] }
}
```

**component.js:**
```javascript
export default class extends HTMLElement {
  connectedCallback() {
    this.attachShadow({ mode: 'open' })
    this.shadowRoot.innerHTML = `
      <style>:host { font: 11px monospace; color: var(--text-muted); }</style>
      <span>My Status: OK</span>
    `
  }
}
```

## Example: Sidecar Plugin

A plugin that runs a background server and shows its status:

```
my-server/
  manifest.json
  provider.json
  component.js
  server/
    index.ts
```

The component spawns the server via `spawn_plugin_process`, listens for JSON
lines on `plugin:stdout:my-server`, and renders status in the `status-bar` slot.
Configuration is managed via `get_plugin_config` / `set_plugin_config` and
rendered in the `settings-page` slot.

See `~/.config/dev.pragma.app/plugins/zen-proxy/` for a complete working example.
