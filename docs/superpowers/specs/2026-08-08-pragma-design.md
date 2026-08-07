# Pragma — Design Spec
**Date:** 2026-08-08

## Overview

Pragma is a GUI AI coding harness — a Tauri 2 desktop application with a Svelte frontend. It is designed to be as customizable as PI harness but with a native windowed interface, a rich plugin system, multi-provider model support, and full session persistence with resume.

---

## Stack

- **Shell:** Tauri 2 (Rust backend, native window)
- **Frontend:** Svelte + TypeScript
- **Storage:** SQLite (`sqlx`) for conversations + flat files for agent work state
- **AI calls:** Rust (`reqwest`), streaming via SSE → Tauri events
- **Plugin UI:** Web Components (ES modules, no build step for plugin authors)
- **Skills:** Markdown SKILL.md files (PI-compatible format)

---

## Section 1: Architecture

```
┌─────────────────────────────────────────────┐
│              Tauri 2 App Window             │
│  ┌──────────────────────────────────────┐   │
│  │         Svelte Frontend              │   │
│  │                                      │   │
│  │  [Tab Bar: sessions]                 │   │
│  │  [chat-toolbar-extra slot]           │   │
│  │  [Chat Panel]                        │   │
│  │  [Input Bar + model picker]          │   │
│  │                                      │   │
│  │  Named slots:                        │   │
│  │    model-picker-extra (in dropdown)  │   │
│  │    settings-screens   (Settings nav) │   │
│  │    chat-toolbar-extra (above input)  │   │
│  │    input-toolbar      (in input bar) │   │
│  │    chat-message-extra                │   │
│  └──────────────────────────────────────┘   │
│              Tauri IPC bridge               │
│  ┌──────────────────────────────────────┐   │
│  │           Rust Backend               │   │
│  │                                      │   │
│  │  • AI HTTP calls (reqwest)           │   │
│  │  • SQLite (sqlx) — conversations     │   │
│  │  • Flat files — agent work state     │   │
│  │  • Plugin manifest loader            │   │
│  │  • Provider registry                 │   │
│  │  • Skill/extension file watcher      │   │
│  └──────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
```

**File layout:**
```
~/.config/pragma/
  plugins/          ← installed plugins (each a folder)
  sessions.db       ← SQLite (messages, sessions, metadata)
  settings.json     ← global config

~/.pi/agent/        ← skills reused as-is (backward compatible)
```

**Key decisions:**
- All AI API calls go through Rust — no direct fetch from Svelte
- Svelte never touches the filesystem directly — all storage via Tauri commands
- Plugin Web Components loaded as ES modules from plugin folder at startup
- Skills stay as markdown files, fully compatible with existing PI skills

---

## Section 2: Sessions & Persistence

**SQLite schema** (`sessions.db`):
```sql
sessions        (id, name, created_at, updated_at, model, provider, metadata_json)
messages        (id, session_id, role, content, model, provider, timestamp, tokens)
session_tags    (session_id, tag)
```

Sessions are named, searchable, and filterable by tag. Each open session appears as a browser-style tab in the title bar.

**Flat files** for agent work state — stored per session:
```
~/.config/pragma/sessions/<session-id>/
  state.json        ← current phase, current task, retry count, plugin owner, etc.
  plan.md           ← living plan (handsfree or any planning plugin)
  screenshots/      ← visual history (never deleted)
```

`state.json` includes a `pluginId` field identifying which plugin owns the state, so Pragma knows which plugin to notify on resume:
```json
{
  "pluginId": "handsfree",
  "phase": "EXECUTING",
  "currentMilestone": "Milestone 2",
  "currentTask": "Task 2.1",
  "retryCount": 0
}
```

**Resume behaviour:**
- App opens → restores previously open tabs
- Click any tab or open a session → full conversation history loaded from SQLite
- If `state.json` exists → plugin that owns that state is notified and can offer to resume
- Model + provider last used are restored per session automatically
- No session is ever auto-deleted — archive instead of delete

---

## Section 3: Provider & Model System

The provider registry lives in Rust. Built-in providers ship with the app; plugins register additional ones with identical treatment in the UI.

**Built-in providers:**
| Provider | Auth | Model discovery |
|---|---|---|
| Anthropic | API key | hardcoded list (Claude 3.5/4.x) |
| OpenAI | API key | `/v1/models` endpoint |
| DeepSeek | API key | hardcoded list |
| OpenCode | local | `/v1/models` endpoint |
| OpenAI-compatible | base URL + key | `/v1/models` endpoint |

**Model picker** — a chip in the input bar footer:
```
[● claude-sonnet-4-6 ▾]  |  [Build | Plan]  [Handsfree]  →→→  [@ skills] [/ cmds] [Send]
```

Clicking the chip opens a unified dropdown grouping all models by provider. Plugin-contributed providers appear under their own header labelled `· plugin` but are otherwise identical. Last used model is remembered per session.

**Plugin-contributed providers** (e.g. Zen WARP):
- Declared in `manifest.json`: `"contributes": { "providers": ["zen-warp"] }`
- Provider behaviour defined in `provider.json` (base URL template, auth type, model discovery)
- Models appear in the native picker under a `ZEN WARP · plugin` group
- Plugin may also inject into `input-toolbar` slot for proxy-specific controls

**Streaming:** Rust streams SSE chunks and emits Tauri events → Svelte appends tokens as they arrive.

---

## Section 4: Plugin System

**Plugin folder structure:**
```
~/.config/pragma/plugins/zen-warp/
  manifest.json       ← plugin declaration
  component.js        ← Web Component (compiled ES module)
  skill.md            ← optional SKILL.md injected into agent context
  provider.json       ← optional provider definition
```

**manifest.json:**
```json
{
  "id": "zen-warp",
  "name": "Zen WARP Proxy",
  "version": "1.0.0",
  "contributes": {
    "providers": ["zen-warp"],
    "slots": ["input-toolbar", "settings-screens"],
    "commands": [],
    "skills": ["skill.md"]
  }
}
```

**Extension slots defined by host:**
| Slot | Where |
|---|---|
| `input-toolbar` | Between model picker and Send button in input bar |
| `model-picker-extra` | Inside model picker dropdown (below provider groups) |
| `settings-screens` | Adds an entry to the Settings left nav |
| `chat-toolbar-extra` | Above the input area (handsfree status bar lives here) |
| `chat-message-extra` | Per-message action buttons |

**Hot-reload:**
- Rust file watcher (`notify` crate) on `~/.config/pragma/plugins/` and `~/.pi/agent/skills/`
- **Skills (SKILL.md):** reload on save — injected into next message's context
- **Plugin Web Components:** `plugin:reload` Tauri event → Svelte unmounts slot, re-imports ES module (cache-busted by timestamp), remounts in under a second
- **manifest.json / provider.json:** full plugin reload on change
- **App code (Svelte):** standard Vite HMR in dev mode
- Toast notification: "reloaded: zen-warp"

**Handsfree as the reference plugin:**
```
~/.config/pragma/plugins/handsfree/
  manifest.json       ← declares chat-toolbar-extra + input-toolbar slots, /handsfree command
  component.js        ← status bar (phase, task progress, screenshot viewer)
  skill.md            ← same behavioral instructions as current PI handsfree skill
```
State machine logic moves to Rust (currently TS extension in PI) — crash-safe, no Node dependency.

---

## Section 5: UI Layout

### Main Window

Sessions are browser-style tabs in the title bar. No sidebar. Multiple sessions open simultaneously.

```
┌─ PRAGMA ─┬─ my-project × ─┬─ rust-game × ─┬─ + ─┬──────────── ⚙ Settings  ⧉ Plugins ─┐
│          │  (active tab)   │               │     │                                       │
├──────────┴─────────────────┴───────────────┴─────┴───────────────────────────────────────┤
│  [chat-toolbar-extra slot — e.g. handsfree status bar]                                   │
├───────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                           │
│   User: How do I implement collision detection in Bevy?                                   │
│                                                                                           │
│   ✦  You can use Bevy's built-in Rapier physics integration...         [message actions]  │
│                                                                                           │
├───────────────────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────────────────────────────┐  │
│  │  Ask anything...                                                                    │  │
│  └─────────────────────────────────────────────────────────────────────────────────────┘  │
│  [● claude-sonnet-4-6 ▾]  │  [input-toolbar slot]  →→→  [@ skills]  [/ cmds]  [Send]    │
└───────────────────────────────────────────────────────────────────────────────────────────┘
```

### Model Picker Dropdown

Opens upward from the model chip. Unified list grouping all providers (native + plugin):

```
┌─────────────────────────────────────┐
│  🔍 Search models...                │
├─────────────────────────────────────┤
│  ANTHROPIC                          │
│    ● claude-sonnet-4-6   200k  ✓   │
│    ● claude-opus-4-6     200k       │
│  DEEPSEEK                           │
│    ● deepseek-v4-flash   64k        │
│  ZEN WARP · plugin                  │
│    ● deepseek-v4 via warp  free     │
│    ● mimo-v2.5 via warp  vision     │
│  OPENCODE · local                   │
│    ● qwen3-32b           128k       │
└─────────────────────────────────────┘
```

Plugin providers appear under their own labelled group, otherwise identical to native providers.

### Input Toolbar (plugin slot example)

```
[● claude-sonnet-4-6 ▾]  │  [Build|Plan]  [Handsfree ●]  →→→  [@ skills]  [/ cmds]  [Send]
                              ↑ Planner      ↑ Handsfree
                              plugin         plugin
                              (input-toolbar slot)
```

### Settings → Plugins Screen

Two-column layout: left nav (core settings + plugin-contributed screens below a divider), right content area.

**Plugin card (expanded):**
- Icon, name, version, author
- Toggle (enable/disable, no restart needed)
- Contribution badges: `provider`, `slot`, `command` tags
- File list
- Status: loaded / error + hot-reload indicator
- Actions: Open settings → · Reload · Remove

**Settings left nav structure:**
```
SETTINGS
  General
  Providers
  Skills
  Plugins          ← manage installed plugins
  Shortcuts
  Appearance

PLUGINS            ← plugin-contributed screens
  Zen WARP
  Handsfree
```

---

## Difficulty Assessment

**Medium project.** Well-understood technologies with no exotic requirements.

| Area | Effort |
|---|---|
| Core Tauri + Svelte shell, tab navigation | 1 week |
| Session persistence (SQLite + flat files) | 1 week |
| Provider registry + model picker + streaming | 1 week |
| Plugin loader + Web Component slot system | 1–2 weeks |
| Hot-reload (file watcher + ES module reload) | 3–4 days |
| Settings UI + plugin inspector | 3–4 days |
| Handsfree port (PI TS → Rust state machine) | 1–2 weeks |
| **Total** | **~6–8 weeks** |
