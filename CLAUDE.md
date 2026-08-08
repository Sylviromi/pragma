# Pragma — Claude Code Guide

Pragma is a Tauri 2 desktop AI coding harness. Rust backend + Svelte 5 frontend. The AI runs agentic loops with 16 built-in tools (file ops, shell, git, web) and supports plugins contributed by the user.

## Run / Build

```bash
npm run dev          # frontend only (Vite)
npm run tauri        # full app (Tauri + Vite dev)
npm run check        # svelte-check (TypeScript)
npm run test         # vitest
cd src-tauri && cargo check   # Rust
cd src-tauri && cargo test    # Rust tests
```

App config dir on Linux: `~/.config/dev.pragma.app/`
Plugins live at: `~/.config/dev.pragma.app/plugins/<id>/`

## Architecture

```
src/                        Svelte 5 frontend (SvelteKit)
  components/               UI components
  stores/                   Svelte 5 $state stores
  lib/tauri.ts              All Tauri invoke wrappers
  lib/types.ts              Shared TypeScript types
  routes/                   SvelteKit pages (+page.svelte, +layout.svelte)

src-tauri/src/
  lib.rs                    App setup, AppState, invoke_handler registration
  commands/                 Tauri commands (invoke from frontend)
  providers/                AI provider implementations
    anthropic.rs            Claude via Anthropic API
    openai_compat.rs        OpenAI-compatible (DeepSeek, local, plugins)
    types.rs                Shared types: ContentBlock, AgentEvent, ModelInfo, ProviderInfo
    mod.rs                  ProviderRegistry (Arc<RwLock<HashMap>>)
  tools/
    definitions.rs          JSON schema definitions for all 16 tools
    mod.rs                  ToolExecutor::execute() dispatch
    file_ops.rs             read_file, write_file, patch_file, create_directory, delete_file, move_file, list_files, search_files
    shell.rs                run_command
    git.rs                  git_status, git_diff, git_log, git_add, git_commit
    web.rs                  web_search (DuckDuckGo), fetch_url
  plugins/
    manifest.rs             PluginManifest + parse_manifest()
    mod.rs                  PluginManager, PluginInfo
    provider.rs             register_plugin_providers() — reads provider.json, registers OpenAICompatProvider
    watcher.rs              notify watcher → hot-reload on plugin file changes
  db/
    mod.rs                  sqlx SQLite init + migrations
    migrations/001_init.sql sessions + messages tables
  screenshot.rs             capture_app_screenshot
```

## Key Conventions

### Rust ↔ TypeScript serialization
All structs serialized to frontend use `#[serde(rename_all = "camelCase")]`.
Enum **tag** values use `snake_case` (`rename_all = "snake_case"` on the enum).
Enum **field** names use `camelCase` (per-variant `rename_all = "camelCase"`).

```rust
// Correct:
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AgentEvent {
    Token { content: String },
    #[serde(rename_all = "camelCase")]
    ToolCall { run_id: String, tool_use_id: String, ... },
}
```

### Adding a Tauri command
1. Write `pub async fn my_command(arg: String, state: tauri::State<'_, AppState>) -> Result<T, String>` in `src-tauri/src/commands/`
2. Add `pub mod my_module;` to `commands/mod.rs`
3. Add `commands::my_module::my_command` to `generate_handler![]` in `lib.rs`
4. Add `export const myCommand = (arg: string) => tauriInvoke<T>('my_command', { arg })` to `src/lib/tauri.ts`

### Adding a tool
1. Add the `ToolDefinition` with JSON schema to `tools/definitions.rs` → `standard_tools()`
2. Implement the logic in the appropriate `tools/*.rs` file
3. Add the match arm in `tools/mod.rs` → `ToolExecutor::execute()`
4. Update the count assertion in `definitions.rs` tests
5. Add the tool name to `SAFE_TOOLS` in `src/stores/chat.svelte.ts` if it's read-only

### Adding a provider
**Built-in**: Add a constructor function in `providers/openai_compat.rs` (or a new file), register in `lib.rs` with `registry.register(...)`.
**Plugin-contributed**: handled automatically via `provider.json` in the plugin folder.

### Frontend patterns (Svelte 5 runes)
```svelte
let x = $state(0)           // reactive state
const y = $derived(x * 2)   // computed
$effect(() => { ... })       // side effect (runs after render)
let { prop } = $props()      // component props
```
All stores are in `src/stores/*.svelte.ts` using top-level `$state` objects (no writable/readable).

### SSE streaming
Both `anthropic.rs` and `openai_compat.rs` buffer across HTTP chunk boundaries:
```rust
let mut buf = String::new();
while let Some(chunk) = stream.next().await {
    buf.push_str(&String::from_utf8_lossy(&bytes));
    loop {
        let Some(nl) = buf.find('\n') else { break };
        let line = buf[..nl].trim_end_matches('\r').to_string();
        buf.drain(..=nl);
        // process line
    }
}
```

## Plugin System

A plugin is a folder at `~/.config/dev.pragma.app/plugins/<id>/` containing:

```
manifest.json   required — declares id, name, version, contributes
provider.json   optional — registers an OpenAI-compat provider
component.js    optional — ES module Web Component for UI slots
skill.md        optional — injected as system context in every conversation
```

### manifest.json
```json
{
  "id": "my-plugin",
  "name": "My Plugin",
  "version": "1.0.0",
  "description": "What it does",
  "contributes": {
    "providers": ["my-plugin"],
    "slots": ["input-toolbar"],
    "commands": [],
    "skills": ["skill.md"]
  }
}
```
- `id` must be lowercase, no spaces or slashes
- Only list `"skills": ["skill.md"]` if you actually have a skill.md that should be injected

### provider.json
```json
{
  "id": "my-plugin",
  "name": "My Provider",
  "base_url": "http://localhost:1234/v1",
  "auth": "none",
  "discover": false,
  "models": [
    { "id": "model-id", "name": "Display Name", "contextWindow": 32768, "isFree": true, "isVision": false }
  ]
}
```
- `auth`: `"api-key"` (reads key from Settings → Providers) or `"none"`
- `discover`: if true, fetches `{base_url}/models` on load to populate model list

### component.js
Plain ES module, no build step. Default export is an HTMLElement subclass:
```javascript
export default class MyPlugin extends HTMLElement {
  connectedCallback() {
    const slot = this.getAttribute('data-slot') // 'input-toolbar', etc.
    this.attachShadow({ mode: 'open' })
    // Use window.__TAURI__.core.invoke('command_name', {arg}) to call Tauri
  }
}
```
Available slots: `input-toolbar`, `chat-toolbar-extra`, `model-picker-extra`

### skill.md
Injected verbatim as a system message prefix in every conversation.
Keep it short and non-prescriptive — it affects all AI interactions.
Don't use first-person "You are..." constructs; prefer concise reference material.

### Hot-reload
The plugin watcher (`plugins/watcher.rs`) detects any file change under the plugins dir and re-registers providers + emits `plugin:reload` to the frontend. No restart needed during development.

## Database

SQLite at `~/.config/dev.pragma.app/sessions.db`. Schema in `db/migrations/001_init.sql`.
- `sessions` — id, name, working_dir, model, provider, archived, metadata_json, timestamps
- `messages` — id, session_id, role, content (JSON), model, provider, timestamp, tokens

Message `content` is stored as JSON: a plain string for simple text, or a `ContentBlock[]` array for tool use/results.

## Settings

Persisted as JSON at `~/.config/dev.pragma.app/settings.json`:
```json
{ "api_keys": { "anthropic": "sk-..." }, "disabled_plugins": ["my-plugin"] }
```
