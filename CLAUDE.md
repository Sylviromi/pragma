# Pragma — Claude Code Guide

Pragma is a Tauri 2 desktop AI coding harness. Rust backend + Svelte 5 frontend. The AI runs agentic loops with 13 core tools (file ops, shell, control flow); the git/web tools arrive with their native skills, and plugins contribute on-demand skills (each with its own tools + optional Node sidecar).

## Run / Build

```bash
npm run dev          # frontend only (Vite)
npm run tauri        # full app (Tauri + Vite dev)
npm run check        # svelte-check (TypeScript)
npm run test         # vitest
cd src-tauri && cargo check   # Rust
cd src-tauri && cargo test    # Rust tests
```

**After adding a new route (`src/routes/...`): restart the dev server.**
Vite's watcher ignores `.svelte-kit/**` (to avoid HMR storms from agent
edits), so the running server keeps a stale client route manifest — navigating
to a brand-new route hard-reloads and can throw `hydration_mismatch`.

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
    definitions.rs          JSON schema definitions for all 17 tools
    mod.rs                  ToolExecutor::execute() dispatch
    file_ops.rs             read_file, write_file, edit_file, delete_file, list_files, search_files
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
A tool belongs to exactly one surface:
1. **Core tool** — schema in `tools/definitions.rs` → `core_tools()`; logic in the matching `tools/*.rs`; match arm in `tools/mod.rs` → `ToolExecutor::execute()`.
2. **Native skill tool** (git/web) — schema in `tools/definitions.rs` → `skill_tools()`; doc lives in `src-tauri/assets/skills/<skill>/skill.md` (embedded in the binary).
3. **Plugin skill tool** — declared in the plugin's `skills/<id>/tools.json` (model-facing schema) and implemented by a Node sidecar `handler.js` (JSON-RPC over stdio, see `tools/sidecar.rs`).
Read-only tools may be added to `SAFE_TOOLS` in `src/stores/chat.svelte.ts`.

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
`providers/sse.rs` (`SseStream`) is the single shared SSE helper — both
`anthropic.rs` and `openai_compat.rs` consume it. It handles the fiddly parts
(multi-byte UTF-8 split across HTTP chunks, `\r\n` trimming, 60s idle timeout,
clean-vs-broken stream end) and yields `data:` payloads one line at a time:
```rust
let mut sse = SseStream::new(resp);
while let Some(data) = sse.next_data().await { /* parse payload */ }
```
`[DONE]` / `message_stop` are interpreted by each provider (they recognize
their own end-of-stream markers).

## Plugin System

A plugin is a folder at `~/.config/dev.pragma.app/plugins/<id>/` containing:

```
manifest.json   required — declares id, name, version, contributes
provider.json   optional — registers an OpenAI-compat provider
component.js    optional — ES module Web Component for UI slots
skills/<id>/    optional — modern layout: one folder per skill
  skill.md      the skill's reference material (loaded on demand)
  tools.json    optional — tool schemas the skill contributes
  handler.js    optional — Node sidecar implementing the tools
```

### Developing plugins outside the repo
Plugin code is **not** part of this source tree. Plugins are **npm packages**
(one plugin per package, `pragma-plugin-<id>`); the source folders live at
`/mnt/Projects/pragma-plugins/` (its own git repo) and are linked into the
config dir for development:

```bash
ln -s /mnt/Projects/pragma-plugins/frontend-dev ~/.config/dev.pragma.app/plugins/frontend-dev
```

Publishing = `node scripts/publish.mjs` in pragma-plugins (syncs the version
and runs `npm publish`); installing = `npm:pragma-plugin-<id>` or the
Marketplace (npm search on `keywords:pragma-plugin`). The app resolves
plugins through the symlink (manifests, skills, sidecars all work) and the
watcher explicitly watches symlink targets, so edits to the untracked folder
hot-reload like any other plugin. The Rust side never creates or copies
plugin folders — what's in the plugin dir is the source of truth.

**First-party exception — `pragma-dev`:** this plugin is committed IN the repo at
`plugins/pragma-dev/` and linked into the config dir, because its content is
an architecture reference for this codebase and must track it (a stale
reference is worse than none). It is **dev-only**: its manifest declares
`"dev": true`, and the plugin manager skips dev plugins in release builds
(`tauri build`) — only debug builds (`tauri dev` / `cargo run`) see it. Link it
into your config dir once per checkout with the portable script (computes the
repo and config paths itself — no hardcoded dirs):

```bash
bash scripts/link-dev-plugin.sh
```

The Rust e2e test `pragma_dev_skill_end_to_end` loads it straight from the
repo tree (debug build, unaffected by the release gating). When you refactor
pragma internals (commands, tools, providers, DB), update
`plugins/pragma-dev/skills/pragma-dev/skill.md` in the same change.

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
- `"dev": true` marks a development-only plugin — loaded only in debug builds
  (`tauri dev` / `cargo run`), skipped entirely in release builds. Use it for
  first-party tooling that must never ship to users (e.g. pragma-dev).

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
Anchored slots: `input-toolbar`, `chat-toolbar-extra`, `model-picker-extra`,
`composer-center`, `status-bar`, `sidebar-section`, `chat-view`, `settings-page`.
**Free slots**: any slot name not in that list is mounted in the global free
plugin layer (`PluginFreeLayer`) — the component positions itself anywhere
with `position: fixed` (set `pointer-events: auto` on `:host`). Plugins can
claim brand-new slot names for arbitrary UI surfaces with no vanilla change.

### skill.md
Loaded on demand (model calls `load_skill`); loading also adds the skill's
tools to the session. Keep it short and non-prescriptive — it's injected as
reference material, not a persona. Don't use first-person "You are..."
constructs; prefer concise reference material.

### Hot-reload
The plugin watcher (`plugins/watcher.rs`) detects any file change under the plugins dir — including inside symlinked plugin folders, whose real targets are watched explicitly — and re-registers providers + emits `plugin:reload` to the frontend. No restart needed during development.

## Database

SQLite at `~/.config/dev.pragma.app/sessions.db`. Schema in `db/migrations/` (001 → 006).
- `sessions` — id, name, working_dir, model, provider, archived, metadata_json, timestamps
- `messages` — id, session_id, role, content (JSON), model, provider, timestamp, tokens
- `session_usage` — per-call telemetry: input/output tokens (provider-counted), cost, plus the
  real per-part prompt breakdown the loop recorded when the request was built
  (`system_tokens` / `conversation_tokens` / `files_tokens`, chars/4 — 006)

Message `content` is stored as JSON: a plain string for simple text, or a `ContentBlock[]` array for tool use/results.

### Context ring semantics (`commands/stats.rs`)
`get_session_stats` / `session:stats` pushes share `build_session_stats`:
- `used` = newest `session_usage.input_tokens` (what the next call would send — the honest basis
  for the future auto-compaction); percent = `used` / provider context window (may exceed 100).
- Segments: the last usage row's recorded breakdown (system prompt + tool schemas, conversation,
  `tool_result` payloads), scaled onto `used` so the ring's fill == percent. Sessions with no
  breakdown row (legacy) fall back to char-estimates from `messages`.
- The sidebar shows the last pushed/loaded payload (store-first, no refetch on open);
  `session:stats` events from the run loop keep it live.

## Settings

Persisted as JSON at `~/.config/dev.pragma.app/settings.json`:
```json
{ "api_keys": { "anthropic": "sk-..." }, "disabled_plugins": ["my-plugin"] }
```
