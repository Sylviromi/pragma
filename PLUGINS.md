# Pragma Plugin Reference

This document describes the Pragma plugin system for AI models and developers
building plugins. A plugin is a folder at `~/.config/dev.pragma.app/plugins/<id>/`.

## Plugin Directory Structure

```
~/.config/dev.pragma.app/plugins/<id>/
  manifest.json       REQUIRED — declares id, name, version, and what the plugin contributes
  provider.json       optional — registers an OpenAI-compatible AI provider
  component.js        optional — ES module Web Component for UI slots
  skill.md            optional — injected as system context in every AI conversation
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
- `contributes.skills`: list skill files (usually just `["skill.md"]`).

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
      "reasoningEffort": "high"
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

| Slot | Location | Use Case |
|------|----------|----------|
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

### Listening to Events

```javascript
const { listen } = window.__TAURI__.event
const unlisten = await listen('event-name', (e) => {
  console.log(e.payload)
})
// Call unlisten() in disconnectedCallback
```

## skill.md (optional)

Injected verbatim as a system message prefix in every AI conversation when the
plugin is enabled. Keep it short and non-prescriptive.

```markdown
# My Plugin Context

Reference material the AI should know about when this plugin is active.
Available tools: ...
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
