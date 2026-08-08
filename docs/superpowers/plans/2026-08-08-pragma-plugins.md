# Pragma Plugin System — Implementation Plan (Plan 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Pragma extensible with plugins. Plugins live as plain folders in `~/.config/pragma/plugins/<id>/` (or the app's real config dir) with **no build step**: a `manifest.json`, an optional ES-module `component.js` that defines a Web Component, an optional `skill.md`, and an optional `provider.json` that registers OpenAI-compatible providers into the native model picker.

**Architecture:**
- Rust owns plugin discovery, enable/disable state, provider registration, and a **file watcher** (`notify`) that hot-reloads plugin files and emits a `plugin:reload` Tauri event.
- Svelte frontend mounts plugin Web Components into extension **slots** (`input-toolbar`, `chat-toolbar-extra`, `settings-screens`, `model-picker-extra`).
- ES modules are transferred to the webview as strings over IPC, imported via Blob URLs (cache-busted by timestamp), and wrapped in a `pragma-plugin-<id>` custom element. No Vite involvement — plugin authors write plain browser ESM.
- Plugin providers flow through the existing `ProviderRegistry`/`ProviderHandle`, tagged `ProviderSource::Plugin`, so they show up in the model picker under their own `· plugin` header automatically.

**Existing surfaces we build on:**
- `ProviderSource::Plugin` already exists in `providers/types.rs`; `ProviderHandle::OpenAICompat` carries base_url/models — plugin providers need no new provider machinery.
- `StatusBar.svelte` already has the `slot-chat-toolbar-extra` container id.
- `src/routes/settings/plugins/+page.svelte` is a placeholder to be replaced.

---

## File Map (new/changed)

```
src-tauri/src/
  plugins/
    mod.rs            PluginManager, PluginInfo, enable/disable, ensure_demo_plugin
    manifest.rs       PluginManifest/Contributes parse + tests
    provider.rs       PluginProviderDef parse + registration into ProviderRegistry + tests
    watcher.rs        notify watcher thread → Emitter: plugin:reload; provider rebind
      loader_tests.rs         (or inline #[cfg(test)] blocks)
  commands/
    plugins.rs        list_plugins, set_plugin_enabled, read_plugin_component, read_plugin_skill, open_plugin_folder
  lib.rs              AppState.plugins, setup wiring, watcher spawn, settings gains disabled_plugins
  providers/
    mod.rs            + unregister(), upsert_models(), ProviderHandle stays as-is
  settings.json       gains "disabled_plugins": ["id", ...]
src/
  lib/tauri.ts        + plugin IPC wrappers
  stores/plugins.svelte.ts   list, cacheKey bump, plugin:reload listener
  components/
    PluginSlot.svelte        loads + mounts one plugin's component into a host
    SlotStack.svelte         renders all plugins claiming a slot
    Toast.svelte + Toast.svelte.ts (module-level $state)
  routes/
    settings/plugins/+page.svelte   (replace placeholder with real management UI)
```

---

## Task A1: Plugin manifest loader

**Files:** `src-tauri/src/plugins/mod.rs`, `src-tauri/src/plugins/manifest.rs`

- [ ] **Step 1: Write test first** — parse full manifest, defaults for missing fields, reject bad JSON.

`src-tauri/src/plugins/manifest.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub contributes: Contributes,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Contributes {
    #[serde(default)]
    pub providers: Vec<String>,
    #[serde(default)]
    pub slots: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
}

pub fn parse_manifest(text: &str) -> Result<PluginManifest, String> {
    let m: PluginManifest = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if m.id.is_empty() || m.name.is_empty() || m.version.is_empty() {
        return Err("manifest needs id, name, version".into());
    }
    if m.id != m.id.trim().to_lowercase() || m.id.contains([' ', '/', '\\']) {
        return Err(format!("invalid plugin id: {}", m.id));
    }
    Ok(m)
}

pub fn load_manifest_from_dir(dir: &Path) -> Result<PluginManifest, String> {
    let p = dir.join("manifest.json");
    if !p.exists() { return Err(format!("no manifest.json in {}", dir.display())); }
    let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    load_manifest(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_manifest() { ... }
    #[test]
    fn defaults_for_missing_contributes() { ... }
    #[test]
    fn rejects_bad_id() { ... }
    #[test]
    fn rejects_bad_json() { ... }
}
```

- [ ] **Step 2: Run** `cd src-tauri && cargo test plugins::manifest::tests` → RED then PASS.

- [ ] **Step 3: Commit.**

## Task 2: PluginManager + AppState wiring + commands

**Files:** `src-tauri/src/plugins/mod.rs`, `src-tauri/src/commands/plugins.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: PluginManager**

```rust
pub struct PluginManager {
    pub dir: PathBuf,
    pub disabled: RwLock<Vec<String>>,      // ids
}

impl PluginManager {
    pub fn new(dir: PathBuf, disabled: Vec<String>) -> Self
    pub fn plugin_dir(&self, id: &str) -> PathBuf
    pub fn is_enabled(&self, id: &str) -> bool            // !disabled && manifest exists
    pub fn list(&self) -> Vec<PluginInfo>                 // skips missing manifest
    pub fn manifest(&self, id: &str) -> Option<PluginManifest>
    pub fn set_disabled(&self, id: &str, disabled: bool) -> bool  // persists via save_settings
    pub fn has_component(&self, id: &str) -> bool         // component.js exists
    pub fn has_skill(&self, id: &str) -> bool
    pub fn component_code(&self, id: &str) -> Result<String, String>
    pub fn skill_md(&self, id: &str) -> Result<String, String>
}

pub struct PluginInfo { id, name, version, description, enabled, contributes, has_component, has_skill, folder }
```

- [ ] **Step 2: Demo plugin bootstrapping** — `ensure_demo_plugin()` writes `demo/` folder (manifest, component.js, skill.md, provider.json) into the plugins dir **only when the dir has no subfolders at all**, so the UI demos itself on first launch. Repo carries the demo plugin sources under `src-tauri/assets/demo-plugin/*` and they are copied via `include_str!`.

- [ ] **Step 3: commands/plugins.rs** — `list_plugins`, `set_plugin_enabled(id, enabled)` (persists to `settings.json` `disabled_plugins`, also un/registers provider), `read_plugin_component(id)` → code string, `read_plugin_skill(id)` → markdown, `open_plugin_folder(id)` → `xdg-open`.

- [ ] **Step 4: lib.rs** — Settings gains `disabled_plugins: Vec<String>` (parsed, default). AppState gains `plugins: plugins::PluginManager`. In setup: create dir, ensure demo, load disabled from settings, build manager with dir, `register_plugin_providers(&registry, &plugins)`.

- [ ] **Step 5: providers/mod.rs** — add `unregister(&self, id)` and `upsert_models(&self, id, models)`.

- [ ] **Step 6: Verify** `cargo check` + `cargo test`, run app, confirm `~/.config/pragma/plugins/demo/` created.

- [ ] **Step 7: Commit.**

## Task 3: Plugin provider registration (provider.json)

**Files:** `src-tauri/src/plugins/provider.rs`, `src-tauri/src/plugins/mod.rs`

- [ ] **Step 1: provider.rs**

```rust
pub struct ProviderDef {
    pub id: String,        // must be in contributes.providers
    pub name: String,
    pub base_url: String,    // e.g. https://api.zen.example/v1
    #[serde(default)] pub auth: String,          // "api-key" | "none"
    #[serde(default)] pub discover: bool,        // fetch /v1/models at load
    #[serde(default)] pub models: Vec<ModelInfo>,
}
```
Parsing of `provider.json` inside plugin folder; validation that `contributes.providers` includes the def id.

- [ ] **Step 2: `register_plugin_providers(registry, manager)`** — for each enabled plugin with provider.json, build `OpenAICompatProvider` with `ProviderSource::Plugin { plugin_id }` and `registry.register`. If `discover: true` → spawn a tokio task that GETs `{base}/models` with bearer api-key from settings (best-effort; 5s timeout; upsert models, ignore failures).

- [ ] **Step 3: tests** — parse valid/invalid provider.json; verify registration produces correct ProviderInfo (`source == Plugin`) and that unregister removes it.

- [ ] **Step 4: Verify** — launch app; demo provider "demo" shows in picker with `· plugin` group. Manual: `curl http://localhost:4000/v1/models` sanity.

- [ ] **Step 5: Commit.**

## Task 4: File watcher + hot reload

**Files:** `src-tauri/src/plugins/watcher.rs`, `src-tauri/src/lib.rs`

- [ ] **Step 1: watcher.rs** — `pub fn spawn_plugin_watcher(app: AppHandle, plugins_dir: PathBuf, on_changed: Box<dyn Fn() + Send + 'static>)`; std-thread poll loop:
  - `notify::RecommendedWatcher` on the dir (recursive), `debounce 350ms`;
  - on event → callback (the real work happens on the Tauri main event loop via a spawned task to avoid blocking):
    - (re)load all enabled plugin providers (unregisterAllPlugin then register)
    - settings write-back not needed
    - `app.emit("plugin:reload", PluginReloadPayload { version: now_ts })`.

- [ ] **Step 2: lib.rs wiring** — after setup registers plugins, spawn watcher if plugins dir exists; pass a callback that uses `app_handle.state` to access registry+plugins and rebind.

- [ ] **Step 3: tests** — watcher itself untested (filesystem+thread), but `register_plugin_providers` idempotency + unregister tested in Task 3; rebind test: register demo provider, `unregister`, `register` again → registry still coherent. Verify app output: edit `demo/component.js` while app runs → toast "reloaded: demo" (frontend part in Task 6).

- [ ] **Step 4: Commit.**

## Task 5: Frontend — IPC + plugins store + PluginSlot

**Files:** `src/lib/tauri.ts`, `src/stores/plugins.svelte.ts`, `src/components/PluginSlot.svelte`, `src/components/SlotStack.svelte`

- [ ] **Step 1: tauri.ts** — `listPlugins()`, `setPluginEnabled(id, enabled)`, `readPluginComponent(id)`, `readPluginSkill(id)`, `openPluginFolder(id)`.

- [ ] **Step 2: stores/plugins.svelte.ts**

```ts
import { listen } from '@tauri-apps/api/event'
export interface PluginInfo { id, name, description, version, enabled, hasComponent, hasSkill, contributes: {providers, slots, commands, skills} }
export const plugins = $state({ list: [] as PluginInfo[], cacheKey: {} as Record<string, number> })
export async function loadPlugins()
export function bumpPlugin(id: string)   // cacheKey[id] = Date.now()
export async function initPluginEvents() // listen('plugin:reload') → bump('*')
```

- [ ] **Step 3: PluginSlot.svelte** — props `{ pluginId, slot, context?, taint = '0s' }`. Effect:
  1. if no plugin / no component → shows up empty.
  2. `const code = await readPluginComponent(pluginId)` (once per (id, taint); cache in module map)
  3. `const blobUrl = URL.createObjectURL(new Blob([code], { type: 'text/javascript' }))`
  4. `const mod = await import(/* @vite-ignore */ blobUrl)` — module default must be a class extending HTMLElement.
  5. registry: `customElements.define('pragma-plugin-<sanitized-id>', mod.default)`
  6. create element, set attrs `data-slot` + `data-context="json"`, append to bound host div; cleanup on destroy/re-taint: remove child, revoke URL.

- [ ] **Step 4: SlotStack.svelte** — `let { slot, context }`, `$derived(apps = plugins.list.filter(p => p.hasComponent && p.enabled && p.contributes.slots.includes(slot)))`, renders `<PluginSlot>` per app in its own inline wrapper; empty → `slot="mask"` nothing & parent `:empty` hide.

- [ ] **Step 5: tests** — `stores/plugins.svelte.ts` logic test (filter/hasComponent) with vitest; slot mount test skipped (needs webview IPC).

- [ ] **Step 6: Commit.**

## Task 6: Wire slots into UI + Toast

**Files:** `src/components/StatusBar.svelte`, `src/components/InputArea.svelte`, `src/components/ModelPicker.svelte`, `src/routes/settings/+layout.svelte`, `src/components/Toast.svelte`, `src/stores/toast.svelte.ts`, `src/routes/+layout.svelte`

- [ ] **Step 1: toast store+component** — `toast.svelte.ts` exposes `push(msg)`, holds list; `Toast.svelte` renders fixed bottom-right stack, auto-dismiss 3s, aria-live polite.

- [ ] **Step 2: StatusBar** → render `<SlotStack slot="chat-toolbar-extra" context={sessionId}>` inside the container; keep `:empty` hide.

- [ ] **Step 3: InputArea** → `<SlotStack slot="input-toolbar" />` between the divider and the hint buttons.

- [ ] **Step 4: ModelPicker** → `<SlotStack slot="model-picker-extra" />` at the bottom of the dropdown.

- [ ] **Step 5: settings nav** → after core items, a `PLUGINS` divider listing each enabled plugin with `settings` slot entries; simplest: render enabled plugins that declare `slots.includes("settings-screens")` as nav rows linking to `/settings/plugins/<id>` (simple param route in plugins page with `?p=` query). Also `initPlugins()` called on app start; `plugin:reload` toast "reloaded: <id>".

- [ ] **Step 6: Verify visually** — demo plugin status bar visible (phase + progress), clicking demo "turbo" button toggles text; editing `component.js` in `~/.config/pragma/plugins/demo/` shows toast + update within ~1s. Screenshot.

- [ ] **Step 7: Commit.**

## Task 7: Settings → Plugins management screen

**Files:** `src/routes/settings/plugins/+page.svelte` (replace placeholder), `src-tauri/src/commands/plugins.rs` (open folder already)

- [ ] **Step 1: Card list** — for each `plugins.list`: name/version/description, chips for contributed providers/slots/skills, toggle button (Switch) → `setPluginEnabled` + `loadPlugins()`; "Open Folder" button; reload button per plugin. Empty state when list empty.
- [ ] **Step 2: Styling** — existing tokens; grid of cards; toggle reuses the `.toggle` pattern from Settings/General.
- [ ] **Step 3: Verify** — disable demo plugin hides its slots everywhere (status bar empty, picker group gone); re-enable brings back; refresh persists.
- [ ] **Step 4: Commit.**

## Task 8: Skills injection

**Files:** `src-tauri/src/commands/providers.rs` (run_agentic_loop), `src-tauri/src/plugins/mod.rs`

- [ ] **Step 1: `collect_skill_markdown(manager) -> String`** — concatenates `skill.md` of all enabled plugins with `# <id>` headers prefixed.
- [ ] **Step 2: Agentic loop** — before building history, load skills once per run; if any, prepend a `system` message containing them (only first run per loop; not re-appended on tool-round trips).
- [ ] **Step 3: list_skills command** — for the `@ skills` popover (frontend uses it, plus plugin skill count on card).
- [ ] **Step 4: tests** — skills formatting unit test; loop-prepend via a small unit test on a pure helper.
- [ ] **Step 5: Commit.**

## Test commands

```bash
cd src-tauri && cargo test            # all Rust
npx vitest run                        # frontend
npm run tauri dev                     # smoke test
```

## Plan 3 Preview

**Handsfree port into Pragma** — the PI handsfree skill as a first-class Pragma plugin:
1. `handsfree` plugin: `manifest.json`, `component.js` (status bar with phase/task/retry/screenshot), `skill.md`
2. State machine moves to Rust: `src-tauri/src/agents/handsfree.rs` reads/writes session `state.json` + `plan.md` (AgentState per session in `~/.config/pragma/sessions/<id>/`)
3. `/handsfree` command slot: launches the loop with tool approval auto-granted after user confirms start
4. Session resumption — if state.json exists, plugin offers "resume" on tab open
5. Screenshot viewer in status bar (plugin UI) + window capture command in Rust