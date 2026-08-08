# Pragma Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a working Tauri 2 AI coding harness with agentic tool use, session persistence, multi-provider streaming, and browser-style tab navigation.

**Architecture:** Rust backend owns all I/O — SQLite, HTTP AI calls, file system tools, shell execution. Agentic loop runs entirely in Rust: sends messages → parses tool_use → executes tools → feeds results back → loops until done. Svelte 5 frontend renders the conversation and tool cards, calls Rust via Tauri IPC only.

**Tech Stack:** Tauri 2, Svelte 5 (runes), TypeScript, Rust, sqlx 0.8 (SQLite), reqwest 0.12, tokio 1, serde_json 1, uuid 1, chrono 0.4

**UI Reference:** `prototype_design/` contains an interactive HTML prototype of the full UI (tab bar, chat, tool cards, model picker, settings, plugin management). All Svelte components must match the visual design there — colors, layout, spacing, typography.

---

## File Map

```
pragma/
  src-tauri/
    src/
      lib.rs                        app setup, state, command registration
      main.rs                       thin entry point
      db/
        mod.rs                      SqlitePool init + run_migrations
        migrations/001_init.sql     sessions, messages, session_tags tables
      commands/
        mod.rs
        sessions.rs                 CRUD + working_dir
        messages.rs                 add, list, content stored as JSON
        providers.rs                list_providers, run_agentic_loop
        tools.rs                    approve_tool_call (frontend → Rust signal)
        agent_state.rs              read/write ~/.config/pragma/sessions/<id>/state.json
      providers/
        mod.rs                      ProviderRegistry
        types.rs                    ContentBlock, ToolDefinition, ProviderInfo, StreamEvent
        anthropic.rs                Anthropic — tool_use aware
        openai_compat.rs            OpenAI-compatible — tool_use aware
      tools/
        mod.rs                      ToolRegistry, execute_tool() dispatch
        definitions.rs              all tool definitions (JSON schema for AI)
        file_ops.rs                 read_file, write_file, create_directory, delete_file, move_file, patch_file, list_files, glob, search_files
        shell.rs                    run_command
        git.rs                      git_status, git_diff, git_log, git_add, git_commit
        web.rs                      web_search (curl → DDG Lite), fetch_url (curl + HTML strip)
    Cargo.toml
    tauri.conf.json
  src/
    app.svelte
    main.ts
    lib/
      tauri.ts                      typed invoke() wrappers
      types.ts                      Session, Message, ContentBlock, ToolCall TS types
    stores/
      sessions.svelte.ts
      providers.svelte.ts
      chat.svelte.ts                agentic conversation state
    components/
      TabBar.svelte
      ChatPanel.svelte
      ChatMessage.svelte            handles text + tool_use + tool_result blocks
      ToolCard.svelte               pending/running/done tool call display + approve/deny
      MessageList.svelte
      InputArea.svelte
      ModelPicker.svelte
      StatusBar.svelte
      Toast.svelte
    routes/
      settings/
        +layout.svelte
        providers/+page.svelte
        appearance/+page.svelte
    styles/
      tokens.css, dark.css, light.css, high-contrast.css, base.css
```

---

## Task 1: Bootstrap project

**Files:** project root via create-tauri-app, `src-tauri/Cargo.toml`, `vite.config.ts`

- [x] **Step 1: Scaffold**
```bash
cd /mnt/Projects/pragma
npm create tauri-app@latest . -- --template svelte-ts --manager npm
npm install
```

- [x] **Step 2: Verify blank window**
```bash
npm run tauri dev
```
Expected: native window opens. Close it.

- [x] **Step 3: Add Rust dependencies to `src-tauri/Cargo.toml`**
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio", "chrono", "uuid", "macros"] }
reqwest = { version = "0.12", features = ["stream", "json"] }
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
futures-util = "0.3"
thiserror = "1"
notify = "6"
glob = "0.3"
```

- [x] **Step 4: Add test tooling**
```bash
npm install -D vitest @testing-library/svelte @testing-library/jest-dom jsdom
```

Add to `vite.config.ts`:
```typescript
import { defineConfig } from 'vite'
import { sveltekit } from '@sveltejs/kit/vite'

export default defineConfig({
  plugins: [sveltekit()],
  test: { environment: 'jsdom', setupFiles: ['src/test-setup.ts'], globals: true },
})
```

Create `src/test-setup.ts`:
```typescript
import '@testing-library/jest-dom'
```

- [x] **Step 5: Set app identity in `src-tauri/tauri.conf.json`**
```json
{
  "productName": "Pragma",
  "identifier": "dev.pragma.app",
  "app": {
    "windows": [{ "title": "Pragma", "width": 1200, "height": 800, "minWidth": 800, "minHeight": 600 }]
  }
}
```

- [x] **Step 6: Verify compilation**
```bash
cd src-tauri && cargo check
```
Expected: no errors.

- [x] **Step 7: Commit**
```bash
git add . && git commit -m "feat: bootstrap Tauri 2 + Svelte project"
```

---

## Task 2: CSS token system + themes

**Files:** `src/styles/tokens.css`, `dark.css`, `light.css`, `high-contrast.css`, `base.css`, `src/app.svelte`

- [x] **Step 1: Write token test**

Create `src/styles/tokens.test.ts`:
```typescript
import { describe, it, expect } from 'vitest'
import { readFileSync } from 'fs'

const dark = readFileSync('src/styles/dark.css', 'utf-8')
const required = [
  '--color-bg-base', '--color-bg-elevated', '--color-bg-overlay',
  '--color-text-primary', '--color-text-secondary', '--color-text-muted', '--color-text-accent',
  '--color-border-default', '--color-border-subtle', '--color-border-focus',
  '--color-accent-brand', '--color-accent-success', '--color-accent-warning', '--color-accent-danger',
  '--radius-sm', '--radius-md', '--radius-lg',
  '--spacing-1', '--spacing-2', '--spacing-3', '--spacing-4', '--spacing-6',
  '--font-sans', '--font-mono',
]

describe('CSS token system', () => {
  it('dark.css defines all required tokens', () => {
    for (const t of required) expect(dark).toContain(t)
  })
})
```

- [x] **Step 2: Run — verify FAIL**
```bash
npx vitest run src/styles/tokens.test.ts
```

- [x] **Step 3: Create `src/styles/dark.css`**
```css
:root[data-theme="dark"], :root {
  --color-bg-base: #0d1117;
  --color-bg-elevated: #161b22;
  --color-bg-overlay: #21262d;
  --color-text-primary: #e6edf3;
  --color-text-secondary: #c9d1d9;
  --color-text-muted: #8b949e;
  --color-text-accent: #58a6ff;
  --color-border-default: #30363d;
  --color-border-subtle: #21262d;
  --color-border-focus: #1f6feb;
  --color-accent-brand: #1f6feb;
  --color-accent-success: #3fb950;
  --color-accent-warning: #d29922;
  --color-accent-danger: #f78166;
  --radius-sm: 4px; --radius-md: 6px; --radius-lg: 10px;
  --spacing-1: 4px; --spacing-2: 8px; --spacing-3: 12px; --spacing-4: 16px; --spacing-6: 24px;
  --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  --font-mono: 'JetBrains Mono', 'Cascadia Code', monospace;
  --font-size-xs: 10px; --font-size-sm: 11px; --font-size-md: 13px; --font-size-lg: 15px;
}
```

- [x] **Step 4: Create `src/styles/light.css`**
```css
:root[data-theme="light"] {
  --color-bg-base: #ffffff; --color-bg-elevated: #f6f8fa; --color-bg-overlay: #eaeef2;
  --color-text-primary: #1f2328; --color-text-secondary: #656d76;
  --color-text-muted: #9198a1; --color-text-accent: #0969da;
  --color-border-default: #d0d7de; --color-border-subtle: #eaeef2; --color-border-focus: #0969da;
  --color-accent-brand: #0969da; --color-accent-success: #1a7f37;
  --color-accent-warning: #9a6700; --color-accent-danger: #cf222e;
  --radius-sm: 4px; --radius-md: 6px; --radius-lg: 10px;
  --spacing-1: 4px; --spacing-2: 8px; --spacing-3: 12px; --spacing-4: 16px; --spacing-6: 24px;
  --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  --font-mono: 'JetBrains Mono', 'Cascadia Code', monospace;
  --font-size-xs: 10px; --font-size-sm: 11px; --font-size-md: 13px; --font-size-lg: 15px;
}
```

- [x] **Step 5: Create `src/styles/high-contrast.css`**
```css
:root[data-theme="high-contrast"] {
  --color-bg-base: #000000; --color-bg-elevated: #0a0a0a; --color-bg-overlay: #1a1a1a;
  --color-text-primary: #ffffff; --color-text-secondary: #eeeeee;
  --color-text-muted: #aaaaaa; --color-text-accent: #66b3ff;
  --color-border-default: #ffffff; --color-border-subtle: #666666; --color-border-focus: #66b3ff;
  --color-accent-brand: #4d94ff; --color-accent-success: #00ff66;
  --color-accent-warning: #ffcc00; --color-accent-danger: #ff4444;
  --radius-sm: 4px; --radius-md: 6px; --radius-lg: 10px;
  --spacing-1: 4px; --spacing-2: 8px; --spacing-3: 12px; --spacing-4: 16px; --spacing-6: 24px;
  --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  --font-mono: 'JetBrains Mono', 'Cascadia Code', monospace;
  --font-size-xs: 10px; --font-size-sm: 11px; --font-size-md: 13px; --font-size-lg: 15px;
}
```

- [x] **Step 6: Create `src/styles/base.css`**
```css
*, *::before, *::after { box-sizing: border-box; }
body {
  margin: 0; font-family: var(--font-sans); font-size: var(--font-size-md);
  color: var(--color-text-primary); background: var(--color-bg-base);
  -webkit-font-smoothing: antialiased;
}
:focus-visible { outline: 2px solid var(--color-border-focus); outline-offset: 2px; }
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { animation-duration: 0.01ms !important; transition-duration: 0.01ms !important; }
}
code, pre, .mono { font-family: var(--font-mono); }
button { cursor: pointer; font-family: inherit; font-size: inherit; }
.sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0,0,0,0); border: 0; }
```

- [x] **Step 7: Wire styles into `src/app.svelte`**
```svelte
<script lang="ts">
  import './styles/dark.css'
  import './styles/light.css'
  import './styles/high-contrast.css'
  import './styles/base.css'
  let theme = $state<'dark'|'light'|'high-contrast'>('dark')
  $effect(() => document.documentElement.setAttribute('data-theme', theme))
</script>
<div id="app"><p style="color:var(--color-text-primary)">Pragma loading…</p></div>
```

- [x] **Step 8: Run — verify PASS**
```bash
npx vitest run src/styles/tokens.test.ts
```

- [x] **Step 9: Commit**
```bash
git add src/styles/ src/app.svelte src/test-setup.ts vite.config.ts
git commit -m "feat: CSS token system with dark/light/high-contrast themes"
```

---

## Task 3: Database setup

**Files:** `src-tauri/src/db/mod.rs`, `src-tauri/src/db/migrations/001_init.sql`

- [x] **Step 1: Write test**

Create `src-tauri/src/db/mod.rs`:
```rust
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;

pub async fn init_db(db_path: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(db_path)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await?;
    run_migrations(&pool).await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(include_str!("migrations/001_init.sql")).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrations_create_sessions_table() {
        let pool = init_db(":memory:").await.unwrap();
        let (name,): (String,) = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='sessions'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(name, "sessions");
    }

    #[tokio::test]
    async fn test_migrations_idempotent() {
        let pool = init_db(":memory:").await.unwrap();
        run_migrations(&pool).await.unwrap(); // second run should not error
    }
}
```

- [x] **Step 2: Run — verify FAIL**
```bash
cd src-tauri && cargo test db::tests
```

- [x] **Step 3: Create migration**

Create `src-tauri/src/db/migrations/001_init.sql`:
```sql
CREATE TABLE IF NOT EXISTS sessions (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    working_dir   TEXT NOT NULL DEFAULT '',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    model         TEXT NOT NULL DEFAULT '',
    provider      TEXT NOT NULL DEFAULT '',
    archived      INTEGER NOT NULL DEFAULT 0,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS messages (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES sessions(id),
    role        TEXT NOT NULL CHECK(role IN ('user','assistant','system')),
    content     TEXT NOT NULL,   -- JSON: string | ContentBlock[]
    model       TEXT NOT NULL DEFAULT '',
    provider    TEXT NOT NULL DEFAULT '',
    timestamp   TEXT NOT NULL,
    tokens      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, timestamp);

CREATE TABLE IF NOT EXISTS session_tags (
    session_id TEXT NOT NULL REFERENCES sessions(id),
    tag        TEXT NOT NULL,
    PRIMARY KEY (session_id, tag)
);
```

- [x] **Step 4: Run — verify PASS**
```bash
cd src-tauri && cargo test db::tests
```

- [x] **Step 5: Commit**
```bash
git add src-tauri/src/db/
git commit -m "feat: SQLite database with sessions/messages schema"
```

---

## Task 4: Rust types + AppState

**Files:** `src-tauri/src/providers/types.rs`, `src-tauri/src/tools/definitions.rs`, `src-tauri/src/lib.rs`

- [x] **Step 1: Create `src-tauri/src/providers/types.rs`**
```rust
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String, #[serde(default)] is_error: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context_window: Option<u32>,
    pub is_free: bool,
    pub is_vision: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSource { Builtin, Plugin { plugin_id: String } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub source: ProviderSource,
    pub models: Vec<ModelInfo>,
}

// Events emitted over the Channel during an agentic loop
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AgentEvent {
    Token { content: String },
    ToolCall { run_id: String, tool_use_id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, output: String, is_error: bool },
    Done,
    Error { message: String },
}
```

- [x] **Step 2: Create tool definitions**

Create `src-tauri/src/tools/definitions.rs`:
```rust
use crate::providers::types::ToolDefinition;

pub fn standard_tools() -> Vec<ToolDefinition> {
    vec![
        // ── File operations ──────────────────────────────────────────────
        ToolDefinition {
            name: "read_file".into(),
            description: "Read the contents of a file. Path is relative to the session working directory.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to working directory" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "write_file".into(),
            description: "Write content to a file, creating it and any parent directories if needed. Overwrites existing files.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDefinition {
            name: "patch_file".into(),
            description: "Apply a unified diff to a file. Use this to make targeted edits without rewriting the whole file.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to working directory" },
                    "diff": { "type": "string", "description": "Unified diff string (--- a/file ... +++ b/file ... @@ ... @@)" }
                },
                "required": ["path", "diff"]
            }),
        },
        ToolDefinition {
            name: "create_directory".into(),
            description: "Create a directory and any missing parent directories.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path relative to working directory" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "delete_file".into(),
            description: "Delete a file. Does not delete directories. Fails if the path does not exist.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to working directory" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "move_file".into(),
            description: "Move or rename a file or directory.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "src": { "type": "string", "description": "Source path relative to working directory" },
                    "dest": { "type": "string", "description": "Destination path relative to working directory" }
                },
                "required": ["src", "dest"]
            }),
        },
        ToolDefinition {
            name: "list_files".into(),
            description: "List files in a directory recursively up to depth 6. Returns relative paths.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path relative to working directory. Defaults to '.'." },
                    "pattern": { "type": "string", "description": "Optional glob pattern, e.g. '**/*.rs'" }
                }
            }),
        },
        ToolDefinition {
            name: "search_files".into(),
            description: "Search for a regex pattern in files. Returns matching lines with file path and line number.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern to search for" },
                    "path": { "type": "string", "description": "Directory to search in, defaults to '.'" },
                    "glob": { "type": "string", "description": "File glob filter, e.g. '*.rs'" }
                },
                "required": ["pattern"]
            }),
        },
        // ── Shell ─────────────────────────────────────────────────────────
        ToolDefinition {
            name: "run_command".into(),
            description: "Run a shell command in the session working directory. Returns combined stdout + stderr. Timeout: 60s.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to run (via sh -c)" }
                },
                "required": ["command"]
            }),
        },
        // ── Git ───────────────────────────────────────────────────────────
        ToolDefinition {
            name: "git_status".into(),
            description: "Return `git status --short` output for the working directory.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo_path": { "type": "string", "description": "Path to git repo root. Defaults to session working directory." }
                }
            }),
        },
        ToolDefinition {
            name: "git_diff".into(),
            description: "Return git diff output. Shows unstaged changes by default.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo_path": { "type": "string" },
                    "staged": { "type": "boolean", "description": "If true, show staged changes (--cached). Default false." },
                    "path": { "type": "string", "description": "Limit diff to a specific file or directory." }
                }
            }),
        },
        ToolDefinition {
            name: "git_log".into(),
            description: "Return recent git commit log.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo_path": { "type": "string" },
                    "limit": { "type": "integer", "description": "Number of commits to show. Default 10." }
                }
            }),
        },
        ToolDefinition {
            name: "git_add".into(),
            description: "Stage files for commit with `git add`.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of file paths to stage. Use ['.'] to stage everything."
                    },
                    "repo_path": { "type": "string" }
                },
                "required": ["paths"]
            }),
        },
        ToolDefinition {
            name: "git_commit".into(),
            description: "Create a git commit with the given message. Staged files must be added first with git_add.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Commit message" },
                    "repo_path": { "type": "string" }
                },
                "required": ["message"]
            }),
        },
        // ── Web ───────────────────────────────────────────────────────────
        ToolDefinition {
            name: "web_search".into(),
            description: "Search the web using DuckDuckGo. Returns titles, URLs, and snippets. No API key required.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "num_results": { "type": "integer", "description": "Number of results to return. Default 5, max 10." }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "fetch_url".into(),
            description: "Fetch a URL and return its text content (HTML tags stripped). Useful for reading documentation, articles, or search result pages.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to fetch" },
                    "max_chars": { "type": "integer", "description": "Truncate output to this many characters. Default 8000." }
                },
                "required": ["url"]
            }),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tools_have_required_fields() {
        for tool in standard_tools() {
            assert!(!tool.name.is_empty(), "tool has no name");
            assert!(!tool.description.is_empty(), "tool {} has no description", tool.name);
            assert!(tool.input_schema.get("type").is_some(), "tool {} schema missing type", tool.name);
        }
    }

    #[test]
    fn test_expected_tool_count() {
        // Ensures no tools are accidentally dropped when editing
        assert_eq!(standard_tools().len(), 16);
    }
}
```

- [x] **Step 3: Run tool definition tests**
```bash
cd src-tauri && cargo test tools::definitions::tests
```
Expected: PASS.

- [x] **Step 4: Wire AppState in `src-tauri/src/lib.rs`**
```rust
use sqlx::SqlitePool;
use tauri::Manager;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

mod db;
mod commands;
mod providers;
mod tools;

#[derive(Default)]
pub struct Settings {
    pub api_keys: HashMap<String, String>,
}

pub struct AppState {
    pub db: SqlitePool,
    pub registry: providers::ProviderRegistry,
    pub settings: RwLock<Settings>,
    pub settings_path: std::path::PathBuf,
    // pending tool approvals: tool_use_id → oneshot sender
    pub pending_approvals: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir().expect("no config dir");
            std::fs::create_dir_all(&config_dir).ok();

            let db_path = format!("sqlite:{}", config_dir.join("sessions.db").display());
            let rt = tokio::runtime::Runtime::new().unwrap();
            let pool = rt.block_on(db::init_db(&db_path)).expect("db init failed");

            let settings_path = config_dir.join("settings.json");
            let settings = load_settings(&settings_path);

            let registry = providers::ProviderRegistry::default();
            registry.register(providers::anthropic::AnthropicProvider);
            registry.register(providers::openai_compat::deepseek());
            registry.register(providers::openai_compat::opencode_local());

            app.manage(AppState {
                db: pool,
                registry,
                settings: RwLock::new(settings),
                settings_path,
                pending_approvals: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::sessions::create_session,
            commands::sessions::list_sessions,
            commands::sessions::get_session,
            commands::sessions::update_session,
            commands::sessions::archive_session,
            commands::messages::add_message,
            commands::messages::list_messages,
            commands::providers::list_providers,
            commands::providers::run_agentic_loop,
            commands::tools::approve_tool,
            commands::tools::deny_tool,
            commands::providers::save_api_key,
            commands::providers::get_api_keys,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn load_settings(path: &std::path::Path) -> Settings {
    if !path.exists() { return Settings::default(); }
    let text = std::fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| {
            let keys = v["api_keys"].as_object()?;
            Some(Settings {
                api_keys: keys.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect(),
            })
        })
        .unwrap_or_default()
}
```

- [x] **Step 5: Create stub modules so it compiles**

Create `src-tauri/src/commands/mod.rs`:
```rust
pub mod sessions;
pub mod messages;
pub mod providers;
pub mod tools;
```

Create `src-tauri/src/providers/mod.rs`:
```rust
pub mod types;
pub mod anthropic;
pub mod openai_compat;

use types::ProviderInfo;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub trait Provider: Send + Sync {
    fn info(&self) -> ProviderInfo;
}

#[derive(Default, Clone)]
pub struct ProviderRegistry {
    inner: Arc<RwLock<HashMap<String, Arc<dyn Provider>>>>,
}

impl ProviderRegistry {
    pub fn register<P: Provider + 'static>(&self, p: P) {
        let info = p.info();
        self.inner.write().unwrap().insert(info.id.clone(), Arc::new(p));
    }
    pub fn get(&self, id: &str) -> Option<Arc<dyn Provider>> {
        self.inner.read().unwrap().get(id).cloned()
    }
    pub fn list_infos(&self) -> Vec<ProviderInfo> {
        self.inner.read().unwrap().values().map(|p| p.info()).collect()
    }
}
```

Create `src-tauri/src/tools/mod.rs`:
```rust
pub mod definitions;
pub mod executor;
```

Create stub files:
```bash
touch src-tauri/src/providers/anthropic.rs
touch src-tauri/src/providers/openai_compat.rs
touch src-tauri/src/tools/executor.rs
touch src-tauri/src/commands/sessions.rs
touch src-tauri/src/commands/messages.rs
touch src-tauri/src/commands/providers.rs
touch src-tauri/src/commands/tools.rs
```

Each stub gets just `// TODO` for now.

- [x] **Step 6: Verify compilation**
```bash
cd src-tauri && cargo check
```

- [x] **Step 7: Commit**
```bash
git add src-tauri/src/
git commit -m "feat: Rust types, AppState, tool definitions scaffold"
```

---

## Task 5: Session CRUD commands

**Files:** `src-tauri/src/commands/sessions.rs`, `src-tauri/src/commands/messages.rs`

- [x] **Step 1: Implement sessions.rs with tests**

Replace `src-tauri/src/commands/sessions.rs`:
```rust
use crate::AppState;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub working_dir: String,
    pub created_at: String,
    pub updated_at: String,
    pub model: String,
    pub provider: String,
    pub archived: bool,
    pub metadata_json: String,
}

#[tauri::command]
pub async fn create_session(name: String, state: tauri::State<'_, AppState>) -> Result<Session, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO sessions (id,name,working_dir,created_at,updated_at,model,provider,archived,metadata_json) VALUES (?,?,?,?,?,'','',0,'{}')")
        .bind(&id).bind(&name).bind("").bind(&now).bind(&now)
        .execute(&state.db).await.map_err(|e| e.to_string())?;
    get_by_id(&state.db, &id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sessions(state: tauri::State<'_, AppState>) -> Result<Vec<Session>, String> {
    sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE archived=0 ORDER BY updated_at DESC")
        .fetch_all(&state.db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_session(id: String, state: tauri::State<'_, AppState>) -> Result<Session, String> {
    get_by_id(&state.db, &id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_session(
    id: String, model: String, provider: String, working_dir: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE sessions SET model=?,provider=?,working_dir=?,updated_at=? WHERE id=?")
        .bind(&model).bind(&provider).bind(&working_dir).bind(&now).bind(&id)
        .execute(&state.db).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn archive_session(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    sqlx::query("UPDATE sessions SET archived=1 WHERE id=?")
        .bind(&id).execute(&state.db).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Session, sqlx::Error> {
    sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id=?")
        .bind(id).fetch_one(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;

    #[tokio::test]
    async fn test_create_and_list() {
        let pool = init_db(":memory:").await.unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO sessions (id,name,working_dir,created_at,updated_at,model,provider,archived,metadata_json) VALUES (?,?,'',?,?,'','',0,'{}')")
            .bind(&id).bind("test").bind(&now).bind(&now)
            .execute(&pool).await.unwrap();
        let rows = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE archived=0")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "test");
    }

    #[tokio::test]
    async fn test_archive_hides_session() {
        let pool = init_db(":memory:").await.unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO sessions (id,name,working_dir,created_at,updated_at,model,provider,archived,metadata_json) VALUES (?,?,'',?,?,'','',0,'{}')")
            .bind(&id).bind("s").bind(&now).bind(&now)
            .execute(&pool).await.unwrap();
        sqlx::query("UPDATE sessions SET archived=1 WHERE id=?").bind(&id).execute(&pool).await.unwrap();
        let rows = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE archived=0")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(rows.len(), 0);
    }
}
```

- [x] **Step 2: Run — verify PASS**
```bash
cd src-tauri && cargo test commands::sessions::tests
```

- [x] **Step 3: Implement messages.rs with tests**

Replace `src-tauri/src/commands/messages.rs`:
```rust
use crate::AppState;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String, // JSON: string or ContentBlock[]
    pub model: String,
    pub provider: String,
    pub timestamp: String,
    pub tokens: i64,
}

#[derive(Debug, Deserialize)]
pub struct NewMessage {
    pub session_id: String,
    pub role: String,
    pub content: String, // serialised JSON
    pub model: String,
    pub provider: String,
    pub tokens: i64,
}

#[tauri::command]
pub async fn add_message(msg: NewMessage, state: tauri::State<'_, AppState>) -> Result<Message, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO messages (id,session_id,role,content,model,provider,timestamp,tokens) VALUES (?,?,?,?,?,?,?,?)")
        .bind(&id).bind(&msg.session_id).bind(&msg.role)
        .bind(&msg.content).bind(&msg.model).bind(&msg.provider)
        .bind(&now).bind(msg.tokens)
        .execute(&state.db).await.map_err(|e| e.to_string())?;
    sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id=?")
        .bind(&id).fetch_one(&state.db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_messages(session_id: String, state: tauri::State<'_, AppState>) -> Result<Vec<Message>, String> {
    sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE session_id=? ORDER BY timestamp ASC")
        .bind(&session_id).fetch_all(&state.db).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;

    #[tokio::test]
    async fn test_add_and_list_messages() {
        let pool = init_db(":memory:").await.unwrap();
        let sid = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO sessions (id,name,working_dir,created_at,updated_at,model,provider,archived,metadata_json) VALUES (?,?,'',?,?,'','',0,'{}')")
            .bind(&sid).bind("s").bind(&now).bind(&now).execute(&pool).await.unwrap();
        let mid = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO messages (id,session_id,role,content,model,provider,timestamp,tokens) VALUES (?,?,'user','\"hello\"','','',?,0)")
            .bind(&mid).bind(&sid).bind(&now).execute(&pool).await.unwrap();
        let msgs = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE session_id=?")
            .bind(&sid).fetch_all(&pool).await.unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "\"hello\"");
    }
}
```

- [x] **Step 4: Run — verify PASS**
```bash
cd src-tauri && cargo test commands::messages::tests
```

- [x] **Step 5: Commit**
```bash
git add src-tauri/src/commands/sessions.rs src-tauri/src/commands/messages.rs
git commit -m "feat: session and message CRUD commands"
```

---

## Task 6: Tool executor

**Files:** `src-tauri/src/tools/mod.rs`, `src-tauri/src/tools/file_ops.rs`, `src-tauri/src/tools/shell.rs`, `src-tauri/src/tools/git.rs`, `src-tauri/src/tools/web.rs`

- [x] **Step 1: Add dependencies to `src-tauri/Cargo.toml`**

```toml
[dependencies]
# (add to existing)
walkdir = "2"
regex = "1"
diffy = "0.4"           # patch_file unified diff

[dev-dependencies]
tempfile = "3"
```

- [x] **Step 2: Create `src-tauri/src/tools/mod.rs`**

```rust
pub mod definitions;
pub mod file_ops;
pub mod shell;
pub mod git;
pub mod web;

use std::path::{Path, PathBuf};

pub struct ToolExecutor {
    pub working_dir: PathBuf,
}

impl ToolExecutor {
    pub fn new(working_dir: impl AsRef<Path>) -> Self {
        Self { working_dir: working_dir.as_ref().to_path_buf() }
    }

    pub fn execute(&self, name: &str, input: &serde_json::Value) -> Result<String, String> {
        match name {
            // file ops
            "read_file"        => file_ops::read_file(&self.working_dir, input),
            "write_file"       => file_ops::write_file(&self.working_dir, input),
            "patch_file"       => file_ops::patch_file(&self.working_dir, input),
            "create_directory" => file_ops::create_directory(&self.working_dir, input),
            "delete_file"      => file_ops::delete_file(&self.working_dir, input),
            "move_file"        => file_ops::move_file(&self.working_dir, input),
            "list_files"       => file_ops::list_files(&self.working_dir, input),
            "search_files"     => file_ops::search_files(&self.working_dir, input),
            // shell
            "run_command"      => shell::run_command(&self.working_dir, input),
            // git
            "git_status"       => git::git_status(&self.working_dir, input),
            "git_diff"         => git::git_diff(&self.working_dir, input),
            "git_log"          => git::git_log(&self.working_dir, input),
            "git_add"          => git::git_add(&self.working_dir, input),
            "git_commit"       => git::git_commit(&self.working_dir, input),
            // web
            "web_search"       => web::web_search(input),
            "fetch_url"        => web::fetch_url(input),
            other              => Err(format!("unknown tool: {other}")),
        }
    }
}
```

- [x] **Step 3: Write failing tests for file_ops**

Create `src-tauri/src/tools/file_ops.rs` with tests first:

```rust
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn abs(working_dir: &Path, rel: &str) -> PathBuf {
    working_dir.join(rel)
}

pub fn read_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    let full = abs(wd, path);
    std::fs::read_to_string(&full).map_err(|e| format!("{e}: {}", full.display()))
}

pub fn write_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    let content = input["content"].as_str().ok_or("missing content")?;
    let full = abs(wd, path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&full, content).map_err(|e| e.to_string())?;
    Ok(format!("wrote {} bytes to {path}", content.len()))
}

pub fn patch_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    let diff = input["diff"].as_str().ok_or("missing diff")?;
    let full = abs(wd, path);
    let original = std::fs::read_to_string(&full)
        .map_err(|e| format!("read failed: {e}"))?;
    let patched = diffy::apply(&original, &diffy::Patch::from_str(diff)
        .map_err(|e| format!("invalid diff: {e}"))?)
        .map_err(|e| format!("patch failed: {e}"))?;
    std::fs::write(&full, &patched).map_err(|e| e.to_string())?;
    Ok(format!("patched {path}"))
}

pub fn create_directory(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    std::fs::create_dir_all(abs(wd, path)).map_err(|e| e.to_string())?;
    Ok(format!("created {path}"))
}

pub fn delete_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let path = input["path"].as_str().ok_or("missing path")?;
    let full = abs(wd, path);
    std::fs::remove_file(&full).map_err(|e| format!("{e}: {}", full.display()))?;
    Ok(format!("deleted {path}"))
}

pub fn move_file(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let src = input["src"].as_str().ok_or("missing src")?;
    let dest = input["dest"].as_str().ok_or("missing dest")?;
    let dest_full = abs(wd, dest);
    if let Some(parent) = dest_full.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(abs(wd, src), &dest_full).map_err(|e| e.to_string())?;
    Ok(format!("moved {src} → {dest}"))
}

pub fn list_files(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let subdir = input["path"].as_str().unwrap_or(".");
    let pattern = input["pattern"].as_str();
    let base = abs(wd, subdir);
    let mut entries = Vec::new();
    for entry in WalkDir::new(&base).max_depth(6) {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(wd).unwrap_or(entry.path()).display().to_string();
            if let Some(pat) = pattern {
                if glob::Pattern::new(pat).map(|p| p.matches(&rel)).unwrap_or(false) {
                    entries.push(rel);
                }
            } else {
                entries.push(rel);
            }
        }
    }
    Ok(if entries.is_empty() { "(no files)".into() } else { entries.join("\n") })
}

pub fn search_files(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let pattern = input["pattern"].as_str().ok_or("missing pattern")?;
    let subdir = input["path"].as_str().unwrap_or(".");
    let file_glob = input["glob"].as_str();
    let base = abs(wd, subdir);
    let re = regex::Regex::new(pattern).map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for entry in WalkDir::new(&base) {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() { continue; }
        let rel = entry.path().strip_prefix(wd).unwrap_or(entry.path()).display().to_string();
        if let Some(g) = file_glob {
            if !glob::Pattern::new(g).map(|p| p.matches(&rel)).unwrap_or(false) { continue; }
        }
        if let Ok(text) = std::fs::read_to_string(entry.path()) {
            for (i, line) in text.lines().enumerate() {
                if re.is_match(line) {
                    results.push(format!("{rel}:{}: {line}", i + 1));
                }
            }
        }
    }
    Ok(if results.is_empty() { "(no matches)".into() } else { results.join("\n") })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_write_then_read() {
        let dir = tempdir().unwrap();
        write_file(dir.path(), &serde_json::json!({"path":"hello.txt","content":"hello world"})).unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"hello.txt"})).unwrap();
        assert_eq!(r, "hello world");
    }

    #[test]
    fn test_patch_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -2,1 +2,1 @@\n-line2\n+LINE2\n";
        patch_file(dir.path(), &serde_json::json!({"path":"f.txt","diff":diff})).unwrap();
        let r = read_file(dir.path(), &serde_json::json!({"path":"f.txt"})).unwrap();
        assert!(r.contains("LINE2"), "expected LINE2 in: {r}");
    }

    #[test]
    fn test_create_and_delete() {
        let dir = tempdir().unwrap();
        create_directory(dir.path(), &serde_json::json!({"path":"sub/nested"})).unwrap();
        assert!(dir.path().join("sub/nested").exists());
        write_file(dir.path(), &serde_json::json!({"path":"sub/nested/x.txt","content":"x"})).unwrap();
        delete_file(dir.path(), &serde_json::json!({"path":"sub/nested/x.txt"})).unwrap();
        assert!(!dir.path().join("sub/nested/x.txt").exists());
    }

    #[test]
    fn test_move_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "data").unwrap();
        move_file(dir.path(), &serde_json::json!({"src":"a.txt","dest":"b.txt"})).unwrap();
        assert!(!dir.path().join("a.txt").exists());
        assert!(dir.path().join("b.txt").exists());
    }

    #[test]
    fn test_list_files_with_pattern() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "").unwrap();
        fs::write(dir.path().join("b.ts"), "").unwrap();
        let r = list_files(dir.path(), &serde_json::json!({"pattern":"*.rs"})).unwrap();
        assert!(r.contains("a.rs"));
        assert!(!r.contains("b.ts"));
    }

    #[test]
    fn test_search_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() { println!(\"hello\"); }").unwrap();
        let r = search_files(dir.path(), &serde_json::json!({"pattern":"println"})).unwrap();
        assert!(r.contains("main.rs"));
    }
}
```

- [x] **Step 4: Run file_ops tests — verify PASS**
```bash
cd src-tauri && cargo test tools::file_ops::tests
```

- [x] **Step 5: Create `src-tauri/src/tools/shell.rs`**

```rust
use std::path::Path;

pub fn run_command(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let command = input["command"].as_str().ok_or("missing command")?;
    let output = std::process::Command::new("sh")
        .arg("-c").arg(command)
        .current_dir(wd)
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    Ok(if combined.trim().is_empty() { "(no output)".into() } else { combined })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_echo() {
        let dir = tempdir().unwrap();
        let r = run_command(dir.path(), &serde_json::json!({"command":"echo hello"})).unwrap();
        assert!(r.contains("hello"));
    }

    #[test]
    fn test_stderr_captured() {
        let dir = tempdir().unwrap();
        let r = run_command(dir.path(), &serde_json::json!({"command":"echo err >&2"})).unwrap();
        assert!(r.contains("err"));
    }
}
```

- [x] **Step 6: Run shell tests — verify PASS**
```bash
cd src-tauri && cargo test tools::shell::tests
```

- [x] **Step 7: Create `src-tauri/src/tools/git.rs`**

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo(wd: &Path, input: &serde_json::Value) -> PathBuf {
    input["repo_path"].as_str()
        .map(|p| if p.is_empty() { wd.to_path_buf() } else { PathBuf::from(p) })
        .unwrap_or_else(|| wd.to_path_buf())
}

fn run_git(args: &[&str], cwd: &Path) -> Result<String, String> {
    let out = Command::new("git").args(args).current_dir(cwd)
        .output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if out.status.success() {
        Ok(if stdout.trim().is_empty() { "(no output)".into() } else { stdout })
    } else {
        Err(if stderr.trim().is_empty() { stdout } else { stderr })
    }
}

pub fn git_status(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    run_git(&["status", "--short"], &repo(wd, input))
}

pub fn git_diff(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let r = repo(wd, input);
    let staged = input["staged"].as_bool().unwrap_or(false);
    let path_arg = input["path"].as_str();
    let mut args = vec!["diff"];
    if staged { args.push("--cached"); }
    if let Some(p) = path_arg { args.push("--"); args.push(p); }
    run_git(&args, &r)
}

pub fn git_log(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let limit = input["limit"].as_u64().unwrap_or(10);
    let n = format!("-{limit}");
    run_git(&["log", "--oneline", &n], &repo(wd, input))
}

pub fn git_add(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let r = repo(wd, input);
    let paths: Vec<&str> = input["paths"].as_array()
        .ok_or("missing paths")?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    if paths.is_empty() { return Err("paths array is empty".into()); }
    let mut args = vec!["add", "--"];
    args.extend(paths.iter().copied());
    run_git(&args, &r)
}

pub fn git_commit(wd: &Path, input: &serde_json::Value) -> Result<String, String> {
    let message = input["message"].as_str().ok_or("missing message")?;
    run_git(&["commit", "-m", message], &repo(wd, input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    fn init_repo(dir: &Path) {
        Command::new("git").args(["init"]).current_dir(dir).output().unwrap();
        Command::new("git").args(["config", "user.email", "test@test.com"]).current_dir(dir).output().unwrap();
        Command::new("git").args(["config", "user.name", "Test"]).current_dir(dir).output().unwrap();
    }

    #[test]
    fn test_git_status_untracked() {
        let dir = tempdir().unwrap();
        init_repo(dir.path());
        fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let r = git_status(dir.path(), &serde_json::json!({})).unwrap();
        assert!(r.contains("a.txt"));
    }

    #[test]
    fn test_git_add_and_commit() {
        let dir = tempdir().unwrap();
        init_repo(dir.path());
        fs::write(dir.path().join("a.txt"), "hi").unwrap();
        git_add(dir.path(), &serde_json::json!({"paths":["a.txt"]})).unwrap();
        let r = git_commit(dir.path(), &serde_json::json!({"message":"initial"})).unwrap();
        assert!(r.contains("initial") || r.contains("master") || r.contains("main"));
    }

    #[test]
    fn test_git_log() {
        let dir = tempdir().unwrap();
        init_repo(dir.path());
        fs::write(dir.path().join("a.txt"), "hi").unwrap();
        git_add(dir.path(), &serde_json::json!({"paths":["a.txt"]})).unwrap();
        git_commit(dir.path(), &serde_json::json!({"message":"first commit"})).unwrap();
        let r = git_log(dir.path(), &serde_json::json!({"limit":5})).unwrap();
        assert!(r.contains("first commit"));
    }
}
```

- [x] **Step 8: Run git tests — verify PASS**
```bash
cd src-tauri && cargo test tools::git::tests
```

- [x] **Step 9: Create `src-tauri/src/tools/web.rs`**

Web search uses DuckDuckGo Lite via curl (no API key required). `fetch_url` downloads any URL and strips HTML tags.

```rust
use std::process::Command;

/// Search the web via DuckDuckGo Lite (free, no key).
/// Parses the simple HTML table that DDG Lite returns.
pub fn web_search(input: &serde_json::Value) -> Result<String, String> {
    let query = input["query"].as_str().ok_or("missing query")?;
    let num = input["num_results"].as_u64().unwrap_or(5).min(10) as usize;

    // URL-encode the query
    let encoded: String = query.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        ' ' => "+".to_string(),
        c => format!("%{:02X}", c as u32),
    }).collect();

    let url = format!("https://lite.duckduckgo.com/lite/?q={encoded}");
    let out = Command::new("curl")
        .args(["-s", "-L", "--max-time", "15",
               "-H", "User-Agent: Mozilla/5.0 (compatible; Lynx/2.8)",
               "-H", "Accept-Language: en-US,en;q=0.9",
               &url])
        .output()
        .map_err(|e| format!("curl failed: {e}"))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }

    let html = String::from_utf8_lossy(&out.stdout);
    let results = parse_ddg_lite(&html, num);
    if results.is_empty() {
        Ok("(no results found)".into())
    } else {
        Ok(results.join("\n\n"))
    }
}

/// Parse DuckDuckGo Lite HTML into (title, url, snippet) triples.
/// DDG Lite uses a simple <table> with alternating result rows.
fn parse_ddg_lite(html: &str, limit: usize) -> Vec<String> {
    let mut results = Vec::new();
    // DDG Lite result links are in <a class="result-link" href="...">Title</a>
    // and snippets in <td class="result-snippet">...</td>
    let mut pos = 0;
    while results.len() < limit {
        // Find next result-link
        let link_start = match html[pos..].find("class=\"result-link\"") {
            Some(i) => pos + i,
            None => break,
        };
        // Extract href
        let href_search = &html[link_start..];
        let href = extract_attr(href_search, "href").unwrap_or_default();
        // Extract link text (title)
        let title = extract_tag_text(&html[link_start..], "a").unwrap_or_default();
        // Find next snippet
        let snip_start = match html[link_start..].find("class=\"result-snippet\"") {
            Some(i) => link_start + i,
            None => { pos = link_start + 1; continue; }
        };
        let snippet = extract_tag_text(&html[snip_start..], "td").unwrap_or_default();

        let title = strip_tags(&title).trim().to_string();
        let snippet = strip_tags(&snippet).trim().to_string();
        if !href.is_empty() && !title.is_empty() {
            results.push(format!("{title}\n{href}\n{snippet}"));
        }
        pos = snip_start + 1;
    }
    results
}

fn extract_attr(html: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = html.find(&needle)? + needle.len();
    let end = html[start..].find('"')? + start;
    Some(html[start..end].to_string())
}

fn extract_tag_text(html: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = html.find(&open)? ;
    let inner_start = html[start..].find('>')? + start + 1;
    let end = html[inner_start..].find(&close)? + inner_start;
    Some(html[inner_start..end].to_string())
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    // Decode common HTML entities
    out.replace("&amp;", "&")
       .replace("&lt;", "<")
       .replace("&gt;", ">")
       .replace("&quot;", "\"")
       .replace("&#x27;", "'")
       .replace("&nbsp;", " ")
}

/// Fetch a URL and return its text content with HTML stripped.
pub fn fetch_url(input: &serde_json::Value) -> Result<String, String> {
    let url = input["url"].as_str().ok_or("missing url")?;
    let max_chars = input["max_chars"].as_u64().unwrap_or(8000) as usize;

    let out = Command::new("curl")
        .args(["-s", "-L", "--max-time", "20",
               "-H", "User-Agent: Mozilla/5.0 (compatible; Lynx/2.8)",
               url])
        .output()
        .map_err(|e| format!("curl failed: {e}"))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }

    let html = String::from_utf8_lossy(&out.stdout);
    let text = strip_tags(&html);
    // Collapse whitespace runs
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(if collapsed.len() > max_chars {
        format!("{}…", &collapsed[..max_chars])
    } else {
        collapsed
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_tags() {
        assert_eq!(strip_tags("<b>hello</b> &amp; world"), "hello & world");
    }

    #[test]
    fn test_parse_ddg_lite_empty() {
        // Should not panic on empty/unexpected HTML
        let results = parse_ddg_lite("", 5);
        assert!(results.is_empty());
    }
}
```

- [x] **Step 10: Run web tests — verify PASS**
```bash
cd src-tauri && cargo test tools::web::tests
```

- [x] **Step 11: Update `src-tauri/src/tools/mod.rs` executor dispatch** (already done in Step 2 above — verify it references all modules)

- [x] **Step 12: Run all tool tests**
```bash
cd src-tauri && cargo test tools::
```
Expected: all pass.

- [x] **Step 13: Commit**
```bash
git add src-tauri/src/tools/ src-tauri/Cargo.toml
git commit -m "feat: full tool suite — file ops, patch, git, web search, fetch_url, shell"
```

---

## Task 7: Provider implementations + agentic loop

**Files:** `src-tauri/src/providers/anthropic.rs`, `src-tauri/src/providers/openai_compat.rs`, `src-tauri/src/commands/providers.rs`, `src-tauri/src/commands/tools.rs`

- [x] **Step 1: Implement Anthropic provider**

Replace `src-tauri/src/providers/anthropic.rs`:
```rust
use super::types::{AgentEvent, ContentBlock, MessageContent, ModelInfo, ProviderInfo, ProviderSource, ToolDefinition};
use futures_util::StreamExt;
use tauri::ipc::Channel;

pub struct AnthropicProvider;

impl super::Provider for AnthropicProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "anthropic".into(), name: "Anthropic".into(),
            source: ProviderSource::Builtin,
            models: vec![
                ModelInfo { id: "claude-sonnet-4-6".into(), name: "Claude Sonnet 4.6".into(), context_window: Some(200_000), is_free: false, is_vision: true },
                ModelInfo { id: "claude-opus-4-6".into(), name: "Claude Opus 4.6".into(), context_window: Some(200_000), is_free: false, is_vision: true },
                ModelInfo { id: "claude-haiku-4-5-20251001".into(), name: "Claude Haiku 4.5".into(), context_window: Some(200_000), is_free: false, is_vision: true },
            ],
        }
    }
}

impl AnthropicProvider {
    pub async fn call(
        messages: &[serde_json::Value],
        model: &str,
        api_key: &str,
        tools: &[ToolDefinition],
        channel: &Channel<AgentEvent>,
    ) -> Result<Vec<ContentBlock>, String> {
        let client = reqwest::Client::new();
        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": 8096,
            "stream": true,
            "messages": messages,
        });
        if !tools.is_empty() {
            body["tools"] = serde_json::to_value(tools.iter().map(|t| serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
            })).collect::<Vec<_>>()).unwrap();
        }

        let resp = client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body).send().await.map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let msg = resp.text().await.unwrap_or_default();
            channel.send(AgentEvent::Error { message: msg.clone() }).ok();
            return Err(msg);
        }

        // Parse SSE stream, collect content blocks
        let mut blocks: Vec<ContentBlock> = Vec::new();
        let mut current_text = String::new();
        let mut current_tool: Option<(String, String, String)> = None; // (id, name, input_json)
        let mut stream = resp.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            for line in String::from_utf8_lossy(&bytes).lines() {
                let Some(data) = line.strip_prefix("data: ") else { continue };
                if data == "[DONE]" { break; }
                let Ok(ev) = serde_json::from_str::<serde_json::Value>(data) else { continue };
                match ev["type"].as_str().unwrap_or("") {
                    "content_block_start" => {
                        let bt = ev["content_block"]["type"].as_str().unwrap_or("");
                        if bt == "tool_use" {
                            let id = ev["content_block"]["id"].as_str().unwrap_or("").to_string();
                            let name = ev["content_block"]["name"].as_str().unwrap_or("").to_string();
                            current_tool = Some((id, name, String::new()));
                        }
                    }
                    "content_block_delta" => {
                        let dt = ev["delta"]["type"].as_str().unwrap_or("");
                        if dt == "text_delta" {
                            let t = ev["delta"]["text"].as_str().unwrap_or("");
                            current_text.push_str(t);
                            channel.send(AgentEvent::Token { content: t.to_string() }).ok();
                        } else if dt == "input_json_delta" {
                            if let Some((_, _, ref mut json_buf)) = current_tool {
                                json_buf.push_str(ev["delta"]["partial_json"].as_str().unwrap_or(""));
                            }
                        }
                    }
                    "content_block_stop" => {
                        if let Some((id, name, json_buf)) = current_tool.take() {
                            let input: serde_json::Value = serde_json::from_str(&json_buf).unwrap_or(serde_json::Value::Null);
                            blocks.push(ContentBlock::ToolUse { id, name, input });
                        } else if !current_text.is_empty() {
                            blocks.push(ContentBlock::Text { text: std::mem::take(&mut current_text) });
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(blocks)
    }
}
```

- [x] **Step 2: Implement OpenAI-compatible provider**

Replace `src-tauri/src/providers/openai_compat.rs`:
```rust
use super::types::{AgentEvent, ContentBlock, ModelInfo, ProviderInfo, ProviderSource, ToolDefinition};
use futures_util::StreamExt;
use tauri::ipc::Channel;

pub struct OpenAICompatProvider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub source: ProviderSource,
    pub models: Vec<ModelInfo>,
}

impl super::Provider for OpenAICompatProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo { id: self.id.clone(), name: self.name.clone(), source: self.source.clone(), models: self.models.clone() }
    }
}

impl OpenAICompatProvider {
    pub async fn call(
        &self,
        messages: &[serde_json::Value],
        model: &str,
        api_key: &str,
        _tools: &[ToolDefinition],  // tool calling varies by provider, basic streaming only for now
        channel: &Channel<AgentEvent>,
    ) -> Result<Vec<ContentBlock>, String> {
        let client = reqwest::Client::new();
        let body = serde_json::json!({ "model": model, "stream": true, "messages": messages });
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let mut req = client.post(&url).header("content-type", "application/json").json(&body);
        if !api_key.is_empty() { req = req.bearer_auth(api_key); }

        let resp = req.send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            let msg = resp.text().await.unwrap_or_default();
            channel.send(AgentEvent::Error { message: msg.clone() }).ok();
            return Err(msg);
        }

        let mut full_text = String::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            for line in String::from_utf8_lossy(&bytes).lines() {
                let Some(data) = line.strip_prefix("data: ") else { continue };
                if data.trim() == "[DONE]" { break; }
                let Ok(ev) = serde_json::from_str::<serde_json::Value>(data) else { continue };
                if let Some(t) = ev["choices"][0]["delta"]["content"].as_str() {
                    if !t.is_empty() {
                        full_text.push_str(t);
                        channel.send(AgentEvent::Token { content: t.to_string() }).ok();
                    }
                }
            }
        }

        Ok(vec![ContentBlock::Text { text: full_text }])
    }
}

pub fn deepseek() -> OpenAICompatProvider {
    OpenAICompatProvider {
        id: "deepseek".into(), name: "DeepSeek".into(),
        base_url: "https://api.deepseek.com/v1".into(),
        source: ProviderSource::Builtin,
        models: vec![
            ModelInfo { id: "deepseek-chat".into(), name: "DeepSeek V3".into(), context_window: Some(64_000), is_free: false, is_vision: false },
        ],
    }
}

pub fn opencode_local() -> OpenAICompatProvider {
    OpenAICompatProvider {
        id: "opencode".into(), name: "OpenCode".into(),
        base_url: "http://localhost:4000/v1".into(),
        source: ProviderSource::Builtin,
        models: vec![],
    }
}
```

- [x] **Step 3: Implement the agentic loop command**

Replace `src-tauri/src/commands/providers.rs`:
```rust
use crate::AppState;
use crate::providers::types::{AgentEvent, ContentBlock, MessageContent, ToolDefinition};
use crate::tools::{definitions::standard_tools, executor::ToolExecutor};
use crate::providers::{anthropic::AnthropicProvider, openai_compat::OpenAICompatProvider};
use tauri::ipc::Channel;
use uuid::Uuid;

#[tauri::command]
pub async fn list_providers(state: tauri::State<'_, AppState>) -> Result<Vec<crate::providers::types::ProviderInfo>, String> {
    Ok(state.registry.list_infos())
}

#[tauri::command]
pub async fn save_api_key(provider: String, key: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    { state.settings.write().unwrap().api_keys.insert(provider, key); }
    let settings = state.settings.read().unwrap();
    let json = serde_json::json!({"api_keys": settings.api_keys});
    std::fs::write(&state.settings_path, serde_json::to_string_pretty(&json).unwrap())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_api_keys(state: tauri::State<'_, AppState>) -> Result<std::collections::HashMap<String, String>, String> {
    Ok(state.settings.read().unwrap().api_keys.clone())
}

/// The core agentic loop. Runs until the AI produces a final response with no tool_use blocks.
/// Emits AgentEvent::ToolCall for each tool — waits for approve/deny from frontend before executing.
#[tauri::command]
pub async fn run_agentic_loop(
    session_id: String,
    user_message: String,
    model: String,
    provider: String,
    working_dir: String,
    auto_approve: bool,
    channel: Channel<AgentEvent>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let api_key = state.settings.read().unwrap().api_keys.get(&provider).cloned().unwrap_or_default();
    let tools = standard_tools();
    let executor = ToolExecutor::new(if working_dir.is_empty() { ".".into() } else { working_dir });

    // Load existing conversation history from DB
    let db_messages = sqlx::query_as::<_, crate::commands::messages::Message>(
        "SELECT * FROM messages WHERE session_id=? ORDER BY timestamp ASC"
    ).bind(&session_id).fetch_all(&state.db).await.map_err(|e| e.to_string())?;

    // Convert to provider message format
    let mut history: Vec<serde_json::Value> = db_messages.iter().map(|m| {
        let content: serde_json::Value = serde_json::from_str(&m.content).unwrap_or(serde_json::Value::String(m.content.clone()));
        serde_json::json!({"role": m.role, "content": content})
    }).collect();

    // Append the new user message
    history.push(serde_json::json!({"role": "user", "content": user_message}));

    // Persist user message
    persist_message(&state.db, &session_id, "user", &serde_json::to_string(&user_message).unwrap(), &model, &provider).await?;

    // Agentic loop
    loop {
        // Call the AI
        let blocks = match provider.as_str() {
            "anthropic" => AnthropicProvider::call(&history, &model, &api_key, &tools, &channel).await?,
            _ => {
                // Get the registered provider
                let prov = state.registry.get(&provider)
                    .ok_or_else(|| format!("unknown provider: {provider}"))?;
                // Downcast to OpenAICompatProvider — simplified: just call openai compat
                let oc = crate::providers::openai_compat::OpenAICompatProvider {
                    id: provider.clone(), name: provider.clone(),
                    base_url: "".into(),
                    source: crate::providers::types::ProviderSource::Builtin,
                    models: vec![],
                };
                oc.call(&history, &model, &api_key, &tools, &channel).await?
            }
        };

        // Find tool_use blocks
        let tool_calls: Vec<_> = blocks.iter().filter_map(|b| {
            if let ContentBlock::ToolUse { id, name, input } = b { Some((id.clone(), name.clone(), input.clone())) } else { None }
        }).collect();

        // Persist assistant turn
        let blocks_json = serde_json::to_string(&blocks).unwrap();
        persist_message(&state.db, &session_id, "assistant", &blocks_json, &model, &provider).await?;
        history.push(serde_json::json!({"role": "assistant", "content": serde_json::from_str::<serde_json::Value>(&blocks_json).unwrap()}));

        if tool_calls.is_empty() {
            // No tool calls — done
            channel.send(AgentEvent::Done).ok();
            break;
        }

        // Handle tool calls
        let run_id = Uuid::new_v4().to_string();
        let mut tool_results: Vec<serde_json::Value> = Vec::new();

        for (tool_use_id, name, input) in &tool_calls {
            // Emit tool call event to frontend
            channel.send(AgentEvent::ToolCall {
                run_id: run_id.clone(),
                tool_use_id: tool_use_id.clone(),
                name: name.clone(),
                input: input.clone(),
            }).ok();

            let approved = if auto_approve {
                true
            } else {
                // Wait for frontend to approve or deny
                let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
                state.pending_approvals.lock().await.insert(tool_use_id.clone(), tx);
                rx.await.unwrap_or(false)
            };

            let (output, is_error) = if approved {
                match executor.execute(name, input) {
                    Ok(out) => (out, false),
                    Err(e) => (e, true),
                }
            } else {
                ("Tool call denied by user.".to_string(), false)
            };

            channel.send(AgentEvent::ToolResult {
                tool_use_id: tool_use_id.clone(),
                output: output.clone(),
                is_error,
            }).ok();

            tool_results.push(serde_json::json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "content": output,
                "is_error": is_error,
            }));
        }

        // Add tool results as user turn
        let results_content = serde_json::to_string(&tool_results).unwrap();
        persist_message(&state.db, &session_id, "user", &results_content, "", "").await?;
        history.push(serde_json::json!({"role": "user", "content": tool_results}));
    }

    Ok(())
}

async fn persist_message(pool: &sqlx::SqlitePool, session_id: &str, role: &str, content: &str, model: &str, provider: &str) -> Result<(), String> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO messages (id,session_id,role,content,model,provider,timestamp,tokens) VALUES (?,?,?,?,?,?,?,0)")
        .bind(&id).bind(session_id).bind(role).bind(content).bind(model).bind(provider).bind(&now)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}
```

- [x] **Step 4: Implement tool approval commands**

Replace `src-tauri/src/commands/tools.rs`:
```rust
use crate::AppState;

#[tauri::command]
pub async fn approve_tool(tool_use_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut pending = state.pending_approvals.lock().await;
    if let Some(tx) = pending.remove(&tool_use_id) {
        tx.send(true).ok();
    }
    Ok(())
}

#[tauri::command]
pub async fn deny_tool(tool_use_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut pending = state.pending_approvals.lock().await;
    if let Some(tx) = pending.remove(&tool_use_id) {
        tx.send(false).ok();
    }
    Ok(())
}
```

- [x] **Step 5: Verify compilation**
```bash
cd src-tauri && cargo check
```

- [x] **Step 6: Commit**
```bash
git add src-tauri/src/providers/ src-tauri/src/commands/providers.rs src-tauri/src/commands/tools.rs
git commit -m "feat: agentic loop with tool calling, approval flow, Anthropic + OpenAI providers"
```

---

## Task 8: TypeScript types + IPC wrappers

**Files:** `src/lib/types.ts`, `src/lib/tauri.ts`

- [x] **Step 1: Create `src/lib/types.ts`**
```typescript
export interface Session {
  id: string
  name: string
  workingDir: string
  createdAt: string
  updatedAt: string
  model: string
  provider: string
  archived: boolean
  metadataJson: string
}

export type ContentBlock =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; input: Record<string, unknown> }
  | { type: 'tool_result'; tool_use_id: string; content: string; is_error?: boolean }

export type MessageContent = string | ContentBlock[]

export interface Message {
  id: string
  sessionId: string
  role: 'user' | 'assistant' | 'system'
  content: string // serialised JSON: string or ContentBlock[]
  model: string
  provider: string
  timestamp: string
  tokens: number
}

export interface ModelInfo {
  id: string; name: string; contextWindow: number | null; isFree: boolean; isVision: boolean
}

export interface ProviderInfo {
  id: string; name: string; source: 'builtin' | { plugin: { pluginId: string } }; models: ModelInfo[]
}

export type AgentEvent =
  | { event: 'token'; content: string }
  | { event: 'tool_call'; runId: string; toolUseId: string; name: string; input: Record<string, unknown> }
  | { event: 'tool_result'; toolUseId: string; output: string; isError: boolean }
  | { event: 'done' }
  | { event: 'error'; message: string }

// Parsed message content helper
export function parseContent(raw: string): MessageContent {
  try { return JSON.parse(raw) } catch { return raw }
}
```

- [x] **Step 2: Create `src/lib/tauri.ts`**
```typescript
import { invoke as tauriInvoke, Channel } from '@tauri-apps/api/core'
import type { Session, Message, ProviderInfo, AgentEvent } from './types'

export const invoke = tauriInvoke

export const listSessions = () => tauriInvoke<Session[]>('list_sessions')
export const createSession = (name: string) => tauriInvoke<Session>('create_session', { name })
export const getSession = (id: string) => tauriInvoke<Session>('get_session', { id })
export const updateSession = (id: string, model: string, provider: string, workingDir: string) =>
  tauriInvoke<void>('update_session', { id, model, provider, workingDir })
export const archiveSession = (id: string) => tauriInvoke<void>('archive_session', { id })

export const listMessages = (sessionId: string) => tauriInvoke<Message[]>('list_messages', { sessionId })

export const listProviders = () => tauriInvoke<ProviderInfo[]>('list_providers')
export const saveApiKey = (provider: string, key: string) => tauriInvoke<void>('save_api_key', { provider, key })
export const getApiKeys = () => tauriInvoke<Record<string, string>>('get_api_keys')

export const approveTool = (toolUseId: string) => tauriInvoke<void>('approve_tool', { toolUseId })
export const denyTool = (toolUseId: string) => tauriInvoke<void>('deny_tool', { toolUseId })

export function runAgenticLoop(
  sessionId: string,
  userMessage: string,
  model: string,
  provider: string,
  workingDir: string,
  autoApprove: boolean,
  onEvent: (e: AgentEvent) => void,
): Promise<void> {
  const channel = new Channel<AgentEvent>()
  channel.onmessage = onEvent
  return tauriInvoke<void>('run_agentic_loop', {
    sessionId, userMessage, model, provider, workingDir, autoApprove, channel,
  })
}
```

- [x] **Step 3: Commit**
```bash
git add src/lib/
git commit -m "feat: TypeScript types and IPC wrappers including agentic loop"
```

---

## Task 9: Svelte stores

**Files:** `src/stores/sessions.svelte.ts`, `src/stores/providers.svelte.ts`, `src/stores/chat.svelte.ts`

- [x] **Step 1: Create `src/stores/sessions.svelte.ts`**
```typescript
import { listSessions as fetch, createSession as apiCreate, archiveSession as apiArchive } from '../lib/tauri'
import type { Session } from '../lib/types'

export const sessions = $state({ list: [] as Session[], openTabs: [] as string[], activeTabId: null as string | null })

export async function loadSessions() { sessions.list = await fetch() }

export function openTab(id: string) {
  if (!sessions.openTabs.includes(id)) sessions.openTabs = [...sessions.openTabs, id]
  sessions.activeTabId = id
  localStorage.setItem('pragma:lastTabId', id)
}

export function closeTab(id: string) {
  const idx = sessions.openTabs.indexOf(id)
  sessions.openTabs = sessions.openTabs.filter(t => t !== id)
  if (sessions.activeTabId === id) {
    sessions.activeTabId = sessions.openTabs[Math.max(0, idx - 1)] ?? null
  }
}

export async function newSession(name?: string) {
  const s = await apiCreate(name ?? `Session ${new Date().toLocaleTimeString()}`)
  sessions.list = [s, ...sessions.list]
  openTab(s.id)
  return s
}

export async function archiveTab(id: string) {
  await apiArchive(id)
  closeTab(id)
  sessions.list = sessions.list.filter(s => s.id !== id)
}

export function getSession(id: string) { return sessions.list.find(s => s.id === id) }
```

- [x] **Step 2: Create `src/stores/providers.svelte.ts`**
```typescript
import { listProviders as fetch } from '../lib/tauri'
import type { ProviderInfo } from '../lib/types'

export const providerState = $state({ providers: [] as ProviderInfo[] })
export async function loadProviders() { providerState.providers = await fetch() }
```

- [x] **Step 3: Create `src/stores/chat.svelte.ts`**
```typescript
import { runAgenticLoop, listMessages as fetchMessages, approveTool as apiApprove, denyTool as apiDeny } from '../lib/tauri'
import type { AgentEvent, ContentBlock } from '../lib/types'

export interface PendingTool {
  runId: string; toolUseId: string; name: string; input: Record<string, unknown>
}

export interface ToolResult {
  toolUseId: string; output: string; isError: boolean
}

export const chat = $state({
  messages: {} as Record<string, import('../lib/types').Message[]>,
  streamingToken: {} as Record<string, string | null>,
  pendingTools: {} as Record<string, PendingTool[]>,
  toolResults: {} as Record<string, ToolResult[]>,
  running: {} as Record<string, boolean>,
})

export async function loadMessages(sessionId: string) {
  chat.messages[sessionId] = await fetchMessages(sessionId)
}

export async function sendMessage(
  sessionId: string, content: string, model: string, provider: string,
  workingDir: string, autoApprove: boolean,
) {
  chat.running[sessionId] = true
  chat.streamingToken[sessionId] = ''
  chat.pendingTools[sessionId] = []

  try {
    await runAgenticLoop(sessionId, content, model, provider, workingDir, autoApprove, (ev: AgentEvent) => {
      if (ev.event === 'token') {
        chat.streamingToken[sessionId] = (chat.streamingToken[sessionId] ?? '') + ev.content
      } else if (ev.event === 'tool_call') {
        chat.pendingTools[sessionId] = [...(chat.pendingTools[sessionId] ?? []), {
          runId: ev.runId, toolUseId: ev.toolUseId, name: ev.name, input: ev.input,
        }]
      } else if (ev.event === 'tool_result') {
        chat.toolResults[sessionId] = [...(chat.toolResults[sessionId] ?? []), {
          toolUseId: ev.toolUseId, output: ev.output, isError: ev.isError,
        }]
        // Remove from pending
        chat.pendingTools[sessionId] = (chat.pendingTools[sessionId] ?? [])
          .filter(t => t.toolUseId !== ev.toolUseId)
      } else if (ev.event === 'done' || ev.event === 'error') {
        chat.streamingToken[sessionId] = null
        chat.running[sessionId] = false
        // Reload messages from DB to get persisted state
        loadMessages(sessionId)
      }
    })
  } finally {
    chat.running[sessionId] = false
    chat.streamingToken[sessionId] = null
  }
}

export async function approveTool(sessionId: string, toolUseId: string) {
  await apiApprove(toolUseId)
}

export async function denyTool(sessionId: string, toolUseId: string) {
  await apiDeny(toolUseId)
}
```

- [x] **Step 4: Commit**
```bash
git add src/stores/
git commit -m "feat: Svelte stores for sessions, providers, and agentic chat"
```

---

## Task 10: Chat UI components

**Files:** `src/components/ToolCard.svelte`, `src/components/ChatMessage.svelte`, `src/components/MessageList.svelte`, `src/components/StatusBar.svelte`, `src/components/InputArea.svelte`, `src/components/ChatPanel.svelte`

- [x] **Step 1: Create `src/components/ToolCard.svelte`**
```svelte
<script lang="ts">
  import { approveTool, denyTool } from '../stores/chat.svelte'
  import type { PendingTool, ToolResult } from '../stores/chat.svelte'

  let { sessionId, tool, result, autoApprove }: {
    sessionId: string
    tool: PendingTool
    result?: ToolResult
    autoApprove: boolean
  } = $props()

  const statusColor = $derived(
    result ? (result.isError ? 'var(--color-accent-danger)' : 'var(--color-accent-success)') : 'var(--color-accent-warning)'
  )
</script>

<div class="tool-card">
  <div class="tool-header">
    <span class="dot" style="background:{statusColor}" aria-hidden="true"></span>
    <span class="tool-name">{tool.name}</span>
    {#if !result && !autoApprove}
      <button class="approve-btn" onclick={() => approveTool(sessionId, tool.toolUseId)}
        aria-label="Approve {tool.name}">Approve</button>
      <button class="deny-btn" onclick={() => denyTool(sessionId, tool.toolUseId)}
        aria-label="Deny {tool.name}">Deny</button>
    {:else if !result}
      <span class="status-text">running…</span>
    {:else}
      <span class="status-text">{result.isError ? 'error' : 'done'}</span>
    {/if}
  </div>

  <pre class="tool-input">{JSON.stringify(tool.input, null, 2)}</pre>

  {#if result}
    <pre class="tool-output" class:error={result.isError}>{result.output}</pre>
  {/if}
</div>

<style>
  .tool-card {
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-md);
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    margin: var(--spacing-2) 0;
  }
  .tool-header {
    display: flex; align-items: center; gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    background: var(--color-bg-elevated);
    border-bottom: 1px solid var(--color-border-subtle);
  }
  .dot { width: 7px; height: 7px; border-radius: 50%; flex-shrink: 0; }
  .tool-name { flex: 1; font-weight: 600; color: var(--color-text-primary); }
  .status-text { color: var(--color-text-muted); font-size: var(--font-size-xs); }
  .approve-btn { background: var(--color-accent-brand); color: white; border: none; border-radius: var(--radius-sm); padding: 2px 10px; }
  .deny-btn { background: transparent; color: var(--color-text-muted); border: 1px solid var(--color-border-default); border-radius: var(--radius-sm); padding: 2px 10px; }
  .tool-input { margin: 0; padding: var(--spacing-2) var(--spacing-3); color: var(--color-text-secondary); background: var(--color-bg-base); overflow-x: auto; white-space: pre-wrap; }
  .tool-output { margin: 0; padding: var(--spacing-2) var(--spacing-3); color: var(--color-text-secondary); background: var(--color-bg-base); border-top: 1px solid var(--color-border-subtle); overflow-x: auto; white-space: pre-wrap; max-height: 200px; overflow-y: auto; }
  .tool-output.error { color: var(--color-accent-danger); }
</style>
```

- [x] **Step 2: Create `src/components/ChatMessage.svelte`**
```svelte
<script lang="ts">
  import { parseContent } from '../lib/types'
  import type { Message } from '../lib/types'

  let { message }: { message: Message } = $props()

  const isUser = message.role === 'user'
  const content = $derived(parseContent(message.content))
  const textBlocks = $derived(
    Array.isArray(content)
      ? content.filter((b: any) => b.type === 'text').map((b: any) => b.text).join('')
      : typeof content === 'string' ? content : ''
  )
  const hasToolBlocks = $derived(
    Array.isArray(content) && content.some((b: any) => b.type === 'tool_use' || b.type === 'tool_result')
  )
</script>

{#if !isUser || textBlocks}
<div class="message" class:user={isUser} class:assistant={!isUser}>
  {#if !isUser}
    <div class="avatar" aria-hidden="true">✦</div>
  {/if}
  <div class="bubble">
    {#if textBlocks}
      <p>{textBlocks}</p>
    {/if}
    {#if !isUser && message.model}
      <div class="meta">{message.model}</div>
    {/if}
  </div>
</div>
{/if}

<style>
  .message { display: flex; gap: var(--spacing-2); padding: var(--spacing-2) var(--spacing-4); align-items: flex-start; }
  .message.user { flex-direction: row-reverse; }
  .avatar { width: 22px; height: 22px; border-radius: 50%; background: var(--color-accent-brand); display: flex; align-items: center; justify-content: center; color: white; font-size: 10px; flex-shrink: 0; margin-top: 2px; }
  .bubble { max-width: 70%; background: var(--color-bg-elevated); border-radius: var(--radius-lg); padding: var(--spacing-2) var(--spacing-3); font-size: var(--font-size-md); line-height: 1.6; }
  .user .bubble { background: color-mix(in srgb, var(--color-accent-brand) 15%, transparent); border: 1px solid color-mix(in srgb, var(--color-accent-brand) 30%, transparent); border-radius: var(--radius-lg) var(--radius-lg) var(--radius-sm) var(--radius-lg); }
  .assistant .bubble { border-radius: var(--radius-sm) var(--radius-lg) var(--radius-lg) var(--radius-lg); }
  p { margin: 0; color: var(--color-text-primary); white-space: pre-wrap; }
  .meta { margin-top: var(--spacing-1); font-size: var(--font-size-xs); color: var(--color-text-muted); }
</style>
```

- [x] **Step 3: Create `src/components/MessageList.svelte`**
```svelte
<script lang="ts">
  import { tick } from 'svelte'
  import ChatMessage from './ChatMessage.svelte'
  import ToolCard from './ToolCard.svelte'
  import { chat } from '../stores/chat.svelte'

  let { sessionId, autoApprove }: { sessionId: string; autoApprove: boolean } = $props()

  let listEl = $state<HTMLElement | null>(null)
  let userScrolled = $state(false)

  const messages = $derived(chat.messages[sessionId] ?? [])
  const streaming = $derived(chat.streamingToken[sessionId])
  const pendingTools = $derived(chat.pendingTools[sessionId] ?? [])
  const toolResults = $derived(chat.toolResults[sessionId] ?? [])

  $effect(() => {
    if (!userScrolled) tick().then(() => { if (listEl) listEl.scrollTop = listEl.scrollHeight })
  })
</script>

<div class="list" bind:this={listEl} onscroll={() => { if (listEl) userScrolled = listEl.scrollHeight - listEl.scrollTop - listEl.clientHeight > 50 }} role="log" aria-live="polite" aria-label="Conversation">
  {#each messages as msg (msg.id)}
    <ChatMessage message={msg} />
  {/each}

  {#each pendingTools as tool (tool.toolUseId)}
    <div style="padding: 0 var(--spacing-4)">
      <ToolCard {sessionId} {tool} result={toolResults.find(r => r.toolUseId === tool.toolUseId)} {autoApprove} />
    </div>
  {/each}

  {#if streaming}
    <div class="message assistant" style="padding: var(--spacing-2) var(--spacing-4)">
      <div class="avatar" aria-hidden="true">✦</div>
      <div class="bubble"><p>{streaming}<span class="cursor" aria-hidden="true">▋</span></p></div>
    </div>
  {/if}
</div>

<style>
  .list { flex: 1; overflow-y: auto; padding: var(--spacing-4) 0; }
  .message { display: flex; gap: var(--spacing-2); align-items: flex-start; }
  .avatar { width: 22px; height: 22px; border-radius: 50%; background: var(--color-accent-brand); display: flex; align-items: center; justify-content: center; color: white; font-size: 10px; flex-shrink: 0; margin-top: 2px; }
  .bubble { background: var(--color-bg-elevated); border-radius: var(--radius-sm) var(--radius-lg) var(--radius-lg) var(--radius-lg); padding: var(--spacing-2) var(--spacing-3); }
  p { margin: 0; color: var(--color-text-primary); white-space: pre-wrap; }
  .cursor { animation: blink 1s step-end infinite; }
  @keyframes blink { 0%,100%{opacity:1} 50%{opacity:0} }
</style>
```

- [x] **Step 4: Create `src/components/InputArea.svelte`**
```svelte
<script lang="ts">
  import { sendMessage, chat } from '../stores/chat.svelte'
  import ModelPicker from './ModelPicker.svelte'
  import { getSession } from '../stores/sessions.svelte'

  let { sessionId }: { sessionId: string } = $props()

  let text = $state('')
  let model = $state('claude-sonnet-4-6')
  let provider = $state('anthropic')
  let autoApprove = $state(false)

  const session = $derived(getSession(sessionId))
  const running = $derived(chat.running[sessionId] ?? false)

  async function submit() {
    const content = text.trim()
    if (!content || running) return
    text = ''
    await sendMessage(sessionId, content, model, provider, session?.workingDir ?? '', autoApprove)
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); submit() }
  }
</script>

<div class="input-area">
  <textarea bind:value={text} {onkeydown} placeholder="Ask anything, or describe what to build…" disabled={running} aria-label="Message input" aria-multiline="true"></textarea>
  <div class="footer">
    <ModelPicker bind:model bind:provider />
    <div class="divider" aria-hidden="true"></div>
    <label class="auto-approve" title="Auto-approve all tool calls without confirmation">
      <input type="checkbox" bind:checked={autoApprove} aria-label="Auto-approve tools" />
      <span>Auto</span>
    </label>
    <div class="footer-right">
      <button class="hint-btn">@ skills</button>
      <button class="hint-btn">/ cmds</button>
      <button class="send-btn" onclick={submit} disabled={running || !text.trim()} aria-label="Send message">
        {running ? '…' : 'Send'}
      </button>
    </div>
  </div>
</div>

<style>
  .input-area { border-top: 1px solid var(--color-border-subtle); padding: var(--spacing-3) var(--spacing-4); display: flex; flex-direction: column; gap: var(--spacing-2); background: var(--color-bg-base); }
  textarea { width: 100%; background: var(--color-bg-elevated); border: 1px solid var(--color-border-default); border-radius: var(--radius-md); padding: var(--spacing-2) var(--spacing-3); color: var(--color-text-primary); font-family: var(--font-sans); font-size: var(--font-size-md); resize: none; min-height: 44px; max-height: 140px; overflow-y: auto; line-height: 1.5; field-sizing: content; }
  textarea:focus { outline: 2px solid var(--color-border-focus); outline-offset: -1px; }
  .footer { display: flex; align-items: center; gap: var(--spacing-2); }
  .divider { width: 1px; height: 18px; background: var(--color-border-default); }
  .auto-approve { display: flex; align-items: center; gap: 4px; font-size: var(--font-size-xs); color: var(--color-text-muted); cursor: pointer; }
  .footer-right { margin-left: auto; display: flex; align-items: center; gap: var(--spacing-2); }
  .hint-btn { background: transparent; border: none; color: var(--color-text-muted); font-size: var(--font-size-sm); padding: var(--spacing-1) var(--spacing-2); border-radius: var(--radius-sm); }
  .hint-btn:hover { color: var(--color-text-primary); background: var(--color-bg-overlay); }
  .send-btn { background: var(--color-accent-brand); color: white; border: none; border-radius: var(--radius-md); padding: var(--spacing-1) var(--spacing-4); font-size: var(--font-size-md); font-weight: 500; }
  .send-btn:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
```

- [x] **Step 5: Create `src/components/StatusBar.svelte`**
```svelte
<script lang="ts">
  let { sessionId }: { sessionId: string } = $props()
</script>
<div class="status-bar" id="slot-chat-toolbar-extra" data-session-id={sessionId}></div>
<style>
  .status-bar:empty { display: none; }
  .status-bar { border-bottom: 1px solid var(--color-border-subtle); font-size: var(--font-size-sm); font-family: var(--font-mono); }
</style>
```

- [x] **Step 6: Create `src/components/ChatPanel.svelte`**
```svelte
<script lang="ts">
  import { onMount } from 'svelte'
  import MessageList from './MessageList.svelte'
  import InputArea from './InputArea.svelte'
  import StatusBar from './StatusBar.svelte'
  import { loadMessages, chat } from '../stores/chat.svelte'

  let { sessionId }: { sessionId: string } = $props()
  const autoApprove = $derived(false) // will come from session settings later

  onMount(() => loadMessages(sessionId))
</script>

<div class="panel">
  <StatusBar {sessionId} />
  <MessageList {sessionId} {autoApprove} />
  <InputArea {sessionId} />
</div>

<style>
  .panel { display: flex; flex-direction: column; height: 100%; overflow: hidden; }
</style>
```

- [x] **Step 7: Commit**
```bash
git add src/components/
git commit -m "feat: chat UI with tool cards, streaming, approve/deny flow"
```

---

## Task 11: Tab bar + model picker + app root

**Files:** `src/components/TabBar.svelte`, `src/components/ModelPicker.svelte`, `src/app.svelte`

- [x] **Step 1: Create TabBar**

Create `src/components/TabBar.svelte`:
```svelte
<script lang="ts">
  import { sessions, openTab, closeTab, newSession } from '../stores/sessions.svelte'
</script>

<div class="tab-bar" role="tablist" aria-label="Sessions">
  <span class="logo" aria-label="Pragma">PRAGMA</span>
  <div class="tabs">
    {#each sessions.openTabs as id (id)}
      {@const s = sessions.list.find(s => s.id === id)}
      {#if s}
        <button class="tab" class:active={sessions.activeTabId === id} role="tab" aria-selected={sessions.activeTabId === id} onclick={() => openTab(id)}>
          <span class="tab-name">{s.name}</span>
          <span class="tab-close" role="button" tabindex="0" aria-label="Close {s.name}"
            onclick={(e) => { e.stopPropagation(); closeTab(id) }}
            onkeydown={(e) => e.key === 'Enter' && (e.stopPropagation(), closeTab(id))}>×</span>
        </button>
      {/if}
    {/each}
  </div>
  <button class="new-tab" onclick={() => newSession()} aria-label="New session">+</button>
  <div class="actions">
    <button aria-label="Settings">⚙</button>
    <button aria-label="Plugins">⧉</button>
  </div>
</div>

<style>
  .tab-bar { display: flex; align-items: stretch; background: var(--color-bg-elevated); border-bottom: 1px solid var(--color-border-subtle); min-height: 36px; user-select: none; }
  .logo { padding: 0 var(--spacing-3); display: flex; align-items: center; font-size: var(--font-size-sm); font-weight: 700; color: var(--color-text-accent); border-right: 1px solid var(--color-border-subtle); flex-shrink: 0; }
  .tabs { display: flex; overflow-x: auto; scrollbar-width: none; flex: 1; }
  .tabs::-webkit-scrollbar { display: none; }
  .tab { display: flex; align-items: center; gap: var(--spacing-1); padding: 0 var(--spacing-3); background: transparent; border: none; border-top: 2px solid transparent; color: var(--color-text-muted); font-size: var(--font-size-sm); white-space: nowrap; cursor: pointer; }
  .tab.active { border-top-color: var(--color-accent-brand); background: var(--color-bg-base); color: var(--color-text-primary); }
  .tab-close { opacity: 0; font-size: 14px; padding: 2px 3px; border-radius: var(--radius-sm); }
  .tab:hover .tab-close, .tab.active .tab-close { opacity: 1; }
  .tab-close:hover { background: var(--color-bg-overlay); }
  .new-tab { padding: 0 var(--spacing-3); background: transparent; border: none; color: var(--color-text-muted); font-size: 18px; cursor: pointer; }
  .actions { margin-left: auto; display: flex; align-items: center; gap: 2px; padding: 0 var(--spacing-3); }
  .actions button { background: transparent; border: none; color: var(--color-text-muted); font-size: var(--font-size-sm); padding: var(--spacing-1) var(--spacing-2); border-radius: var(--radius-sm); }
  .actions button:hover { color: var(--color-text-primary); background: var(--color-bg-overlay); }
</style>
```

- [x] **Step 2: Create ModelPicker**

Create `src/components/ModelPicker.svelte`:
```svelte
<script lang="ts">
  import { onMount } from 'svelte'
  import { loadProviders, providerState } from '../stores/providers.svelte'

  let { model = $bindable(''), provider = $bindable('') }: { model: string; provider: string } = $props()

  let open = $state(false)
  let search = $state('')
  let el = $state<HTMLElement | null>(null)

  onMount(async () => {
    await loadProviders()
    if (!model && providerState.providers[0]) {
      provider = providerState.providers[0].id
      model = providerState.providers[0].models[0]?.id ?? ''
    }
  })

  const filtered = $derived(providerState.providers.map(p => ({
    ...p,
    models: p.models.filter(m => !search || m.name.toLowerCase().includes(search.toLowerCase()) || m.id.toLowerCase().includes(search.toLowerCase()))
  })).filter(p => p.models.length > 0))

  const current = $derived(providerState.providers.flatMap(p => p.models.map(m => ({ ...m, pid: p.id, pname: p.name }))).find(m => m.id === model && m.pid === provider))

  function select(pid: string, mid: string) { provider = pid; model = mid; open = false; search = '' }
</script>

<svelte:window onclick={(e) => { if (el && !el.contains(e.target as Node)) open = false }} />

<div class="picker" bind:this={el}>
  <button class="chip" onclick={() => open = !open} aria-haspopup="listbox" aria-expanded={open}>
    <span class="dot" aria-hidden="true"></span>
    <span>{current?.name ?? 'Choose model'}</span>
    {#if current}<span class="pname">· {current.pname}</span>{/if}
    <span aria-hidden="true">▾</span>
  </button>

  {#if open}
    <div class="dropdown" role="listbox">
      <div class="search"><input type="search" placeholder="Search models…" bind:value={search} autofocus aria-label="Search models" /></div>
      <div class="list">
        {#each filtered as p (p.id)}
          <div class="group-label">{p.name}{#if p.source !== 'builtin'} <span class="plugin-tag">· plugin</span>{/if}</div>
          {#each p.models as m (m.id)}
            <button class="row" class:sel={m.id === model && p.id === provider} onclick={() => select(p.id, m.id)} role="option" aria-selected={m.id === model && p.id === provider}>
              <span class="dot" aria-hidden="true"></span>
              <span class="rname">{m.name}</span>
              {#if m.contextWindow}<span class="rmeta">{(m.contextWindow/1000).toFixed(0)}k</span>{/if}
              {#if m.isFree}<span class="rmeta free">free</span>{/if}
              {#if m.id === model && p.id === provider}<span class="check" aria-hidden="true">✓</span>{/if}
            </button>
          {/each}
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .picker { position: relative; }
  .chip { display: flex; align-items: center; gap: var(--spacing-1); background: var(--color-bg-elevated); border: 1px solid var(--color-border-default); border-radius: var(--radius-md); padding: 3px var(--spacing-2); color: var(--color-text-primary); font-size: var(--font-size-sm); white-space: nowrap; }
  .chip:hover { border-color: var(--color-border-focus); }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--color-accent-brand); flex-shrink: 0; }
  .pname { color: var(--color-text-muted); }
  .dropdown { position: absolute; bottom: calc(100% + 6px); left: 0; width: 280px; background: var(--color-bg-elevated); border: 1px solid var(--color-border-default); border-radius: var(--radius-lg); box-shadow: 0 8px 24px rgba(0,0,0,0.4); overflow: hidden; z-index: 100; }
  .search { padding: var(--spacing-2); border-bottom: 1px solid var(--color-border-subtle); }
  .search input { width: 100%; background: var(--color-bg-base); border: 1px solid var(--color-border-default); border-radius: var(--radius-sm); padding: 4px var(--spacing-2); color: var(--color-text-primary); font-size: var(--font-size-sm); }
  .list { max-height: 300px; overflow-y: auto; padding: var(--spacing-1) 0; }
  .group-label { padding: var(--spacing-1) var(--spacing-3); font-size: var(--font-size-xs); font-weight: 700; color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.05em; }
  .plugin-tag { font-weight: normal; text-transform: none; }
  .row { display: flex; align-items: center; gap: var(--spacing-2); width: 100%; padding: 5px var(--spacing-4); background: transparent; border: none; color: var(--color-text-secondary); font-size: var(--font-size-sm); text-align: left; cursor: pointer; }
  .row:hover { background: var(--color-bg-overlay); color: var(--color-text-primary); }
  .row.sel { background: color-mix(in srgb, var(--color-accent-brand) 10%, transparent); }
  .rname { flex: 1; }
  .rmeta { color: var(--color-text-muted); font-size: var(--font-size-xs); }
  .free { color: var(--color-accent-success); }
  .check { color: var(--color-accent-success); margin-left: auto; }
</style>
```

- [x] **Step 3: Wire everything in `src/app.svelte`**
```svelte
<script lang="ts">
  import './styles/dark.css'
  import './styles/light.css'
  import './styles/high-contrast.css'
  import './styles/base.css'
  import { onMount } from 'svelte'
  import TabBar from './components/TabBar.svelte'
  import ChatPanel from './components/ChatPanel.svelte'
  import { loadSessions, sessions, openTab } from './stores/sessions.svelte'

  onMount(async () => {
    await loadSessions()
    const last = localStorage.getItem('pragma:lastTabId')
    if (last && sessions.list.find(s => s.id === last)) openTab(last)
  })
</script>

<div id="app">
  <TabBar />
  <main>
    {#if sessions.activeTabId}
      <ChatPanel sessionId={sessions.activeTabId} />
    {:else}
      <div class="empty">Open or create a session to start.</div>
    {/if}
  </main>
</div>

<style>
  #app { display: flex; flex-direction: column; height: 100vh; overflow: hidden; }
  main { flex: 1; overflow: hidden; }
  .empty { display: flex; align-items: center; justify-content: center; height: 100%; color: var(--color-text-muted); }
</style>
```

- [x] **Step 4: Full smoke test**
```bash
npm run tauri dev
```
Expected: tab bar, + creates session, chat input visible, message sends, streaming response appears, tool calls show ToolCard with Approve/Deny (or auto-execute if Auto checked).

- [x] **Step 5: Commit**
```bash
git add src/components/TabBar.svelte src/components/ModelPicker.svelte src/app.svelte
git commit -m "feat: tab bar, model picker, full app wired together"
```

---

## Task 12: Settings — API keys + appearance

**Files:** `src/routes/settings/+layout.svelte`, `src/routes/settings/providers/+page.svelte`, `src/routes/settings/appearance/+page.svelte`

- [x] **Step 1: Create settings layout**

Create `src/routes/settings/+layout.svelte`:
```svelte
<script lang="ts">
  import { page } from '$app/stores'
  let { children } = $props()
  const nav = [
    { label: 'General', href: '/settings/general' },
    { label: 'Providers', href: '/settings/providers' },
    { label: 'Plugins', href: '/settings/plugins' },
    { label: 'Appearance', href: '/settings/appearance' },
  ]
</script>
<div class="shell">
  <nav aria-label="Settings navigation">
    <div class="label">SETTINGS</div>
    {#each nav as item}
      <a href={item.href} class="item" class:active={$page.url.pathname === item.href} aria-current={$page.url.pathname === item.href ? 'page' : undefined}>{item.label}</a>
    {/each}
  </nav>
  <div class="content">{@render children()}</div>
</div>
<style>
  .shell { display: flex; height: 100%; }
  nav { width: 180px; background: var(--color-bg-elevated); border-right: 1px solid var(--color-border-subtle); padding: var(--spacing-3) var(--spacing-2); display: flex; flex-direction: column; gap: 2px; flex-shrink: 0; }
  .label { font-size: var(--font-size-xs); font-weight: 700; letter-spacing: 0.05em; color: var(--color-text-muted); padding: var(--spacing-1) var(--spacing-2); margin-bottom: var(--spacing-1); }
  .item { display: block; padding: 5px var(--spacing-2); border-radius: var(--radius-sm); color: var(--color-text-muted); font-size: var(--font-size-sm); text-decoration: none; }
  .item:hover { color: var(--color-text-primary); background: var(--color-bg-overlay); }
  .item.active { color: var(--color-text-accent); background: color-mix(in srgb, var(--color-accent-brand) 15%, transparent); }
  .content { flex: 1; padding: var(--spacing-6); overflow-y: auto; }
</style>
```

- [x] **Step 2: Create providers settings page**

Create `src/routes/settings/providers/+page.svelte`:
```svelte
<script lang="ts">
  import { onMount } from 'svelte'
  import { saveApiKey, getApiKeys } from '../../../lib/tauri'
  import { providerState, loadProviders } from '../../../stores/providers.svelte'

  let keys = $state<Record<string,string>>({})
  let saved = $state<Record<string,boolean>>({})

  onMount(async () => { await loadProviders(); keys = await getApiKeys() })

  async function save(id: string) {
    await saveApiKey(id, keys[id] ?? '')
    saved[id] = true
    setTimeout(() => { saved[id] = false }, 2000)
  }
</script>

<h2 style="margin-top:0;color:var(--color-text-primary)">Providers</h2>
<p style="color:var(--color-text-muted);font-size:var(--font-size-sm)">Keys stored in ~/.config/pragma/settings.json</p>

{#each providerState.providers.filter(p => p.source === 'builtin') as p}
  <div style="margin-top:var(--spacing-6)">
    <label style="display:block;font-weight:500;margin-bottom:6px;color:var(--color-text-primary)" for="k-{p.id}">{p.name}</label>
    <div style="display:flex;gap:8px">
      <input id="k-{p.id}" type="password" bind:value={keys[p.id]} placeholder="API key"
        style="flex:1;background:var(--color-bg-elevated);border:1px solid var(--color-border-default);border-radius:var(--radius-md);padding:6px 10px;color:var(--color-text-primary);font-family:var(--font-mono);font-size:var(--font-size-sm)" />
      <button onclick={() => save(p.id)}
        style="background:var(--color-accent-brand);color:white;border:none;border-radius:var(--radius-md);padding:6px 16px;cursor:pointer">
        {saved[p.id] ? 'Saved ✓' : 'Save'}
      </button>
    </div>
  </div>
{/each}
```

- [x] **Step 3: Create appearance settings page**

Create `src/routes/settings/appearance/+page.svelte`:
```svelte
<script lang="ts">
  let theme = $state(document.documentElement.getAttribute('data-theme') ?? 'dark')

  function setTheme(t: string) {
    theme = t
    document.documentElement.setAttribute('data-theme', t)
    localStorage.setItem('pragma:theme', t)
  }
</script>

<h2 style="margin-top:0;color:var(--color-text-primary)">Appearance</h2>
<p style="color:var(--color-text-muted);font-size:var(--font-size-sm)">Choose a theme.</p>

<div style="display:flex;gap:var(--spacing-3);margin-top:var(--spacing-4)">
  {#each ['dark','light','high-contrast'] as t}
    <button onclick={() => setTheme(t)} style="
      padding:var(--spacing-3) var(--spacing-4);border-radius:var(--radius-md);cursor:pointer;
      border: 2px solid {theme === t ? 'var(--color-accent-brand)' : 'var(--color-border-default)'};
      background:{theme === t ? 'color-mix(in srgb, var(--color-accent-brand) 10%, transparent)' : 'var(--color-bg-elevated)'};
      color:var(--color-text-primary);font-size:var(--font-size-sm);text-transform:capitalize">
      {t.replace('-', ' ')}
    </button>
  {/each}
</div>
```

- [x] **Step 4: Restore theme on startup**

In `src/app.svelte` `onMount`, before `loadSessions()`:
```typescript
const savedTheme = localStorage.getItem('pragma:theme')
if (savedTheme) document.documentElement.setAttribute('data-theme', savedTheme)
```

- [x] **Step 5: Smoke test settings**
```bash
npm run tauri dev
```
Expected: Settings button opens settings, can enter API key and save, theme toggle switches themes.

- [x] **Step 6: Run all tests**
```bash
npx vitest run && cd src-tauri && cargo test
```
Expected: all pass.

- [x] **Step 7: Final commit**
```bash
git add src/routes/ src/app.svelte
git commit -m "feat: settings — API key management, theme switcher, startup restore"
```

---

## Running Tests

```bash
# All Rust unit tests
cd src-tauri && cargo test

# All frontend tests
npx vitest run

# Full release build check
npm run tauri build
```

---

## Plan 2 Preview

**Pragma Plugin System** covers:
1. Plugin manifest loader (`~/.config/pragma/plugins/`)
2. Web Component slot system + `PluginSlot.svelte`
3. Plugin-contributed providers registered at runtime
4. File watcher hot-reload (notify → Tauri events → ES module cache-bust)
5. Settings → Plugins management screen (plugin cards, enable/disable)
6. Handsfree plugin port (PI TS state machine → Rust + Web Component status bar)
