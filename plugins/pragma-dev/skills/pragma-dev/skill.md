# Pragma Dev — architecture reference + dev tooling

Working on **Pragma** — a Tauri 2 + Svelte 5 desktop AI coding harness.
Source: `/mnt/Projects/pragma` (this repo). Set the session working
directory to that path before starting.

This skill's tools (`pd_*`, see tools.json) drive the dev loop from inside
the agent: typecheck/cargo-check/test runners, a managed dev-app child, and
pragma-convention verifiers. You cannot see the app window — the dev-app logs
are your eyes on the build. This file is the architecture reference; consult
it before cross-module work.

## Dev workflow

1. **Verify as you edit**
   - `pd_check` — `npm run check` (svelte-kit sync + svelte-check), parsed errors/warnings.
   - `pd_cargo_check` — `cargo check --message-format=json` in `src-tauri/`, clean grouped errors/warnings (not the 96k-char wall).
   - `pd_test` / `pd_cargo_test` — vitest / cargo test with failure extraction.
   - `pd_verify_command <name>` / `pd_verify_tool <name>` — after adding a command/tool, confirms every wiring point in one call (catching a forgotten `generate_handler!` entry or a missing `tauri.ts` wrapper).
2. **Watch the app build**: `pd_dev_start` (mode `app` = `npm run dev:app`, the project's canonical launcher; `tauri` = full tauri dev with auto-reload; `vite` = frontend-only). Poll `pd_dev_status` / `pd_dev_logs` (filter `errors`). Stop with `pd_dev_stop`. Only one dev-app instance at a time — if the user's main app was itself launched via `dev:app`, prefer `pd_reload_app` for Rust changes instead of a second instance.
3. **Apply changes**
   - **Frontend / plugin-only changes**: hot-reload automatically via Vite — no tool needed. If HMR didn't pick something up cleanly (route/layout edits, stale plugin components) call `pd_reload_frontend` LAST — it reloads ONLY the webview at the end of the run; the Rust backend and the loop keep going. The frontend re-attaches to any still-running loop on the next page load (`reconnectRunningSessions` in `src/stores/chat.svelte.ts`): it restores the live turn from sessionStorage and resumes live updates; the session never appears idle while the loop works.
   - **Rust changes**: call `pd_reload_app` LAST — it runs `cargo build` in `src-tauri/` and, on success, requests a full app relaunch (exit code 23 → the `dev:app` launcher rebuilds and restarts; other launchers exit cleanly). A failed build never restarts. This is the ONLY path that restarts the whole app — never use it for frontend-only or plugin-only changes.

## Stack

- **Backend**: Rust + Tauri 2, sqlx/SQLite, reqwest, serde, tauri-plugin-dialog, tauri-plugin-notification
- **Frontend**: Svelte 5 runes ($state/$derived/$effect/$props), TypeScript, SvelteKit
- **Build**: `npm run tauri` (full dev), `npm run dev` (vite only), `npm run dev:app` (scripts/dev-app.sh — no Rust auto-restart), `npm run check` (TS), `npm run test` (vitest run), `cd src-tauri && cargo check` / `cargo test`

## Source map

```
src/lib/tauri.ts                 All Tauri invoke() wrappers (mock<T>() falls back in browser mock mode)
src/lib/types.ts                 Shared TS types (Session, Message, AgentEvent, ProviderInfo, SkillInfo…)
src/lib/markdown.ts              safe markdown for model output (marked: escape-then-sanitize);
                                 installMarkdownPluginBridge() exposes {renderMarkdown, segmentsOf, withCaret}
                                 on window.__pragmaMarkdown (installed in +layout.svelte) so plugins render
                                 model text with the exact same renderer — plugins never vendor a parser
src/stores/chat.svelte.ts        Chat state — streaming, tool items, approval, SAFE_TOOLS set, working_dir sync
src/stores/sessions.svelte.ts    Sessions list + tab (open-tab set) management — sessions render in SessionRail
src/stores/agents.svelte.ts      Agents pane store (sub-agents + detached processes) — EMPTY until backend events land
src/components/                  ChatPanel (root per-tab: MessageList + InputArea), ChatMessage, SessionRail
                                 (left rail, sessions grouped by working dir), DirBar (working-dir strip),
                                 ThoughtCard, QuestionBlock (choice/scale/input), ToolCard, SubagentCard +
                                 ProcessCard (upcoming — inert renderer branches), ModelPicker, AgentMark
                                 (per-session tint avatar), InputArea (inline effort chips), TabBar (slim
                                 chrome: logo + rail toggle + active session + actions), SidebarPanel (pinned
                                 context ring + Plan/Agents/Changes tabs), SettingsHeader, StatusBar,
                                 PluginSlot/SlotStack (plugin UI slots), PluginConfirmModal, ReturnModal,
                                 Toast, Toggle, Icon, ContextMenu
                                 Shared primitives (componentized audit): Collapsible (chevron header +
                                 grid-rows collapse — ThoughtCard/ToolCard/SubagentCard/ProcessCard shells),
                                 Button (primary/ghost/danger/warning/quiet), Modal (overlay + ESC/outside
                                 close — ReturnModal/PluginConfirmModal/UpdateDialog), Logo, TurnRow,
                                 Card/SettingRow/SectionHeader/SearchInput/Segmented/EmptyState (settings),
                                 StatusDot, InlineAlert, OutputLog (shared mono log pane), Changelog
src/routes/                      +page.svelte (rail + chat + sidebar), +layout.svelte, settings/{general,appearance,
                                 providers,plugins,plugin/[id],skills,shortcuts}

src-tauri/src/lib.rs             App setup, AppState, plugin registration, invoke_handler
src-tauri/src/commands/agentic.rs  run_agentic_loop (agentic loop + tool dispatch + change_directory special
                                 case + sidecar reloadApp marker/exit-23), get_running_sessions, interrupt_session
src-tauri/src/commands/sessions.rs  CRUD for sessions (create/list/get/update/archive)
src-tauri/src/commands/messages.rs  add_message, list_messages
src-tauri/src/commands/providers.rs list_providers, save_api_keys (multi-key rotation per provider), get_api_keys
src-tauri/src/commands/tools.rs     approve_tool / deny_tool / answer_question / get_pending_approvals
src-tauri/src/commands/plugins.rs   list/enable/disable plugins, read component/skill, plugin config get/set,
                                 request/respond plugin confirmation, open_plugin_folder, list_skills
src-tauri/src/commands/process.rs   spawn_plugin_process / stop / status / write_plugin_stdin / probe_port
                                 (managed plugin processes, plugin_id → process, events plugin:stdout:{id})
src-tauri/src/commands/revert.rs    file-op snapshot journal + revert_session ("return to this point")
src-tauri/src/commands/stats.rs     session usage tracking, cost, build_session_stats (context ring: used = last
                                 provider counted input; segments = recorded prompt breakdown)
src-tauri/src/notify.rs             OS notifications (only when window unfocused) via tauri-plugin-notification
src-tauri/src/providers/types.rs    ContentBlock (Text/ToolUse/ToolResult/Thinking), AgentEvent, ModelInfo…
src-tauri/src/providers/anthropic.rs  Anthropic SSE streaming (thinking/signature blocks)
src-tauri/src/providers/openai_compat.rs  OpenAI-compat SSE + to_openai_messages() history converter
src-tauri/src/providers/sse.rs     shared SSE helper (streams `data:` payloads line by line)
src-tauri/src/tools/definitions.rs core_tools() (14) + skill_tools() (git 5 + web 2) = standard_tools() (21);
                                 count assertions in tests
src-tauri/src/tools/mod.rs         ToolExecutor::execute() dispatch; SeenLedger (per-file read/serve state)
src-tauri/src/tools/file_ops.rs    read/write/patch/create/delete/move/list/search files
src-tauri/src/tools/shell.rs       run_command (sh -c, 60s default timeout)
src-tauri/src/tools/git.rs         git_status/diff/log/add/commit
src-tauri/src/tools/web.rs         web_search (DuckDuckGo), fetch_url
src-tauri/src/tools/sidecar.rs     SkillSidecarRegistry — plugin skill handlers.js JSON-RPC sidecars
src-tauri/src/plugins/             manifest.rs, mod.rs (PluginManager/SkillInfo), provider.rs, watcher.rs
src-tauri/src/db/migrations/       001_init → 006_usage_breakdown (see DB schema)
src-tauri/capabilities/default.json  Tauri permissions
plugins/pragma-dev/                this plugin (first-party, symlinked into the config plugins dir)
```

## Key data flows

**Agentic loop** (`commands/agentic.rs::run_agentic_loop`):
1. Load DB history → strip denied tool exchanges → prepend system prompt + loaded-skill docs
2. Loop: call provider → emit Token/ToolCall/Thinking events → plain tools queue for batch flush; inline tools (load_skill/change_directory/ask_question/sidecar) resolve approval and execute immediately. `ask_question` pauses for a user answer (notification).
3. `change_directory` is special-cased (updates DB + emits `WorkingDirChanged`) before `executor.execute()`.
4. Mutating file tools snapshot pre-op state to `file_snapshots` (revert.rs) before executing.
5. `run_command`/`git_commit` are approval-gated; `SAFE_TOOLS` (read-only) run without approval. The flush resolves approvals in two phases — read-only batch runs first (parallel) so SAFE tools complete while a mutating tool's approval prompt is still open; approvals are bounded by a 120s timeout (`resolve_tool_approval`, `APPROVAL_TIMEOUT`) so a missed prompt can't hang the loop. Approvals arriving before the pending entry is registered are honored via `pre_approved`/`pre_denied`.
6. On `pd_reload_app` success (sidecar returns `reloadApp: true`), the loop exits with code 23; the `dev:app` launcher relaunches.
7. On `done`/`error`: frontend calls `loadMessages()` to sync from DB.

**Working directory**: stored in `sessions.working_dir`. Set by the user (DirBar folder picker) or the model (`change_directory`). `InputArea` reads the current dir from the store on send; file/shell tools resolve relative to it.

**Extended thinking (Anthropic)**: SSE emits `thinking_delta` + `signature_delta` → captured into `ContentBlock::Thinking`; persisted and passed back in history (API requirement). `to_openai_messages()` strips thinking blocks for OpenAI-compat providers.

**Streaming**: both providers buffer raw bytes → emit complete SSE lines only; `accept-encoding: identity` disables compression. `providers/sse.rs` is the shared helper.

**Plugin skill sidecars** (`tools/sidecar.rs`): a skill's model-facing tools always come from its `tools.json` (declarative, hot-reload-safe). On first `load_skill`, `node skills/<id>/handler.js` is spawned (JSON-RPC 2.0 over newline-delimited stdio); tools named in `tools.json` route to it via the `by_name` index. Handlers are reused across turns (`ensure`), killed on app exit / `stop_*`. A failed spawn marks the skill down — calls then fail fast with a clear message.

**Plugin processes** (`commands/process.rs`): plugins can spawn one managed process each (frontend-driven) with captured stdout/stderr emitted as `plugin:stdout:{id}` events — distinct from skill sidecars (model-driven).

## Adding a Tauri command

1. `src-tauri/src/commands/my_mod.rs` — `pub async fn my_cmd(state: tauri::State<'_, AppState>) -> Result<T, String>`
2. `commands/mod.rs` — `pub mod my_mod;`
3. `lib.rs` `generate_handler![]` — `commands::my_mod::my_cmd`
4. `src/lib/tauri.ts` — `export const myCmd = (…) => tauriInvoke<T>('my_cmd', {…})` (use `mock<T>(fallback, 'my_cmd')` when browser mock mode should also work)
5. Run `pd_verify_command my_cmd` — it checks all four wiring points.

## Adding a tool

1. `tools/definitions.rs` — ToolDefinition in `core_tools()` (or `skill_tools()` for native git/web); assert counts in `test_expected_tool_count`.
2. Implement in `tools/<module>.rs`, or inline in `agentic.rs` when it needs session/DB access (e.g. `edit_file` uses the seen-ledger + whole-file hash anchor, `change_directory`/skill reload markers are session-aware).
3. `tools/mod.rs` `ToolExecutor::execute()` — match arm (or special-case in the agentic loop before/after `execute()`).
4. Read-only tools: add to `SAFE_TOOLS` in `src/stores/chat.svelte.ts` (skips approval).
5. Run `pd_verify_tool my_tool` — checks definition, dispatch arm, and SAFE_TOOLS classification.

## Serialization rules

- Rust structs → TS: `#[serde(rename_all = "camelCase")]` on the struct.
- Enum tag values: `#[serde(tag = "event", rename_all = "snake_case")]`.
- Enum field names: per-variant `#[serde(rename_all = "camelCase")]` (NOT on the enum itself).

## DB schema (`~/.config/dev.pragma.app/sessions.db`, migrations in src-tauri/src/db/migrations/)

```sql
sessions(id, name, working_dir, created_at, updated_at, model, provider, archived, metadata_json)
messages(id, session_id→sessions, role, content — JSON string | ContentBlock[], model, provider, timestamp, tokens)
session_tags(session_id, tag)                       -- 001
active_loops(session_id PK, started_at, heartbeat_at) -- 002/004: cross-process loop lock
file_snapshots(id, session_id, message_rowid, tool_use_id, tool_name, path,
               content_before, existed_before, extra_before)  -- 003: revert journal
session_usage(id, session_id, model, provider, input_tokens, output_tokens, cost_usd,
              system_tokens, conversation_tokens, files_tokens) -- 005/006: real
              per-call prompt breakdown (chars/4 of what the loop actually sent)
```

## Context ring semantics (sidebar telemetry)

- `used` = the newest `session_usage.input_tokens` (provider-counted — exactly
  what the next call would send, the basis for auto-compaction). Segments are
  scaled onto `used` so the ring's filled fraction always equals percent.
- Segments come from the recorded breakdown (`006`: system prompt + tool
  schemas / conversation / tool_result payloads, measured at request-build
  time in agentic.rs — never a hardcoded system estimate). Sessions without a
  breakdown row (legacy) fall back to char-estimates from `messages`.
- The sidebar shows the LAST pushed/loaded payload (store-first): it does not
  re-fetch on every panel open — `session:stats` pushes from the run loop keep
  it live. Only sessions with no payload yet are fetched once.

## Plugin authoring (folders in `~/.config/dev.pragma.app/plugins/<id>/`)

- Theming: plugin UI reads the same design tokens as the app (`--bg-base`, `--accent`, `--border-default`, …) — including the interface scale var `--ui-scale` (default 1.1 = 110%, adjustable in Settings → Appearance) and the font knob `--font-ui` (the primary UI face; Settings → Appearance → UI font remaps the whole trio — `--font-body` derives from it, `--font-heading`/`--font-mono` keep the display/mono roles). The scale is applied as a `transform: scale` on `#app`, so plugin components inherit it automatically and should NOT set their own font-size root.
- Positioning: because of that transform, `position: fixed` inside a plugin anchors to the app (which fills the window), not the raw viewport — use `inset`/`top-right-bottom-left` anchors (they render correctly at any scale), and if you need exact cursor coordinates, divide them by `--ui-scale`.

- `manifest.json` — id (lowercase, = folder name), name, version, contributes: {providers, slots, commands, skills}. Legacy `contributes.skills` kept for compatibility; modern skills live in `skills/<id>/` and are auto-discovered.
- `skills/<id>/` — `skill.md` (reference, loaded on demand via load_skill), `tools.json` (model-facing schemas — authoritative), `handler.js` (Node sidecar implementing them, JSON-RPC over stdio).
- `provider.json` — optional OpenAI-compat model registration (auth "api-key"|"none", discover, reasoningEffort, per-model supportsThinking/thinkingBudget/maxTokens).
- `component.js` — ES module Web Component for UI slots: `chat-view` (replaces the
  whole message list AND owns the composer — the default InputArea is hidden when a
  plugin claims it), `input-toolbar`, `chat-toolbar-extra`, `model-picker-extra`,
  `status-bar`, `composer-center`, `sidebar-section`, `settings-page`.
- `config.json` — plugin-scoped persistent config via `get/set_plugin_config` commands.
- Enable/disable is persisted app-wide in `settings.json` (`disabled_plugins`), toggled from
  Settings → Plugins. The PluginManager's live list is the source of truth;
  `save_settings` persists `manager.disabled_ids()` so toggles survive restarts.
- Hot-reload: the watcher watches symlink targets; any file change re-registers providers + emits `plugin:reload`.

**This plugin is first-party** (lives at `plugins/pragma-dev/` in the repo, symlinked into the config dir) — a deliberate exception to the "plugins live outside the repo" convention, so its reference stays in sync with the code it documents.