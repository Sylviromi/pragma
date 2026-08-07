# Pragma — Design Spec
**Date:** 2026-08-08
**Status:** In progress (UI Layout section pending)

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
│  │  [Session Sidebar] [Chat Panel]      │   │
│  │  [Model Picker   ] [Toolbar  ]       │   │
│  │                                      │   │
│  │  <slot name="model-picker-extra">    │   │
│  │  <slot name="settings-screens">      │   │
│  │  <slot name="chat-toolbar-extra">    │   │
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

Sessions are named, searchable, and filterable by tag. Sidebar lists them sorted by last active with fuzzy search.

**Flat files** for agent work state — stored per session:
```
~/.config/pragma/sessions/<session-id>/
  state.json        ← current phase, current task, retry count, etc.
  plan.md           ← living plan (handsfree or any planning plugin)
  screenshots/      ← visual history (never deleted)
```

**Resume behaviour:**
- App opens → sidebar shows all sessions
- Click any session → full conversation history loaded from SQLite, rendered in chat panel
- If `state.json` exists → plugin that owns that state is notified and can offer to resume
- Model + provider last used are restored per session automatically
- No session is ever auto-deleted — archive instead of delete

---

## Section 3: Provider & Model System

The provider registry lives in Rust. Built-in providers ship with the app; plugins can register additional ones.

**Built-in providers:**
| Provider | Auth | Model discovery |
|---|---|---|
| Anthropic | API key | hardcoded list (Claude 3.5/4.x) |
| OpenAI | API key | `/v1/models` endpoint |
| DeepSeek | API key | hardcoded list |
| OpenCode | local | `/v1/models` endpoint |
| OpenAI-compatible | base URL + key | `/v1/models` endpoint |

**Per-message model picker** — sits in the chat input bar:
```
[Session: my-project ▾] ... [claude-sonnet-4-6 (Anthropic) ▾] [Send]
```

Last used model is remembered per session. Models grouped by provider in the dropdown.

**Plugin-contributed providers** (e.g. Zen WARP):
- Plugin manifest declares: `"contributes": { "providers": ["zen-warp"] }`
- Provider defined via JSON file (base URL template, auth type, model discovery method)
- Web Component injects into `<slot name="model-picker-extra">` for proxy-specific controls
- Indistinguishable from built-in providers in the chat UI

**Streaming:** Rust streams SSE chunks from the provider and emits Tauri events → Svelte subscribes and appends tokens as they arrive.

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
    "slots": ["model-picker-extra", "settings-screens"],
    "commands": [],
    "skills": ["skill.md"]
  }
}
```

**Extension slots defined by host:**
| Slot | Description |
|---|---|
| `model-picker-extra` | Below the model dropdown |
| `settings-screens` | New tab in Settings |
| `chat-toolbar-extra` | Above the input bar (handsfree status lives here) |
| `session-sidebar-extra` | Below session list |
| `chat-message-extra` | Per-message action buttons |

**Hot-reload:**
- Rust file watcher (`notify` crate) on `~/.config/pragma/plugins/` and `~/.pi/agent/skills/`
- **Skills (SKILL.md):** reload instantly on save — injected into next message's context
- **Plugin Web Components:** `plugin:reload` Tauri event → Svelte unmounts slot, re-imports ES module (cache-busted by timestamp), remounts
- **manifest.json / provider.json:** full plugin reload on change
- **App code (Svelte):** standard Vite HMR in dev mode
- Toast notification: "reloaded: zen-warp"

**GUI plugin inspector** — Settings → Plugins screen:
- Lists all installed plugins with their slots, skills, and provider contributions
- Toggle per plugin (enable/disable without deleting)
- No file editing required for on/off

**Handsfree as the reference plugin:**
```
~/.config/pragma/plugins/handsfree/
  manifest.json       ← declares chat-toolbar-extra slot, commands: ["/handsfree"]
  component.js        ← status bar (phase, task progress, screenshot viewer)
  skill.md            ← same behavioral instructions as current PI handsfree skill
```
State machine logic moves to Rust (currently TS extension in PI) — crash-safe, no Node dependency.

---

## Section 5: UI Layout

*Pending*
