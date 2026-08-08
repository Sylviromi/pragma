# Project: Pragma Core — Tauri 2 AI coding harness
Generated: 2026-08-08
Last updated: 2026-08-08

## Milestone 1: Project bootstrap
### Task 1.1: Bootstrap Tauri 2 + Svelte 5 project
Approach: Scaffold with create-tauri-app (svelte-ts template), npm install, add Rust deps to Cargo.toml, configure tauri.conf.json identity, add vitest test tooling, verify `cargo check` passes.
Status: done

## Milestone 2: Theme system
### Task 2.1: CSS token system + dark/light/high-contrast themes
Approach: Write token test, create dark.css/light.css/high-contrast.css/base.css token files, wire into app.svelte, verify vitest test passes.
Status: done

## Milestone 3: Database
### Task 3.1: SQLite schema + migrations
Approach: Create db/mod.rs with init_db + run_migrations, migration 001_init.sql (sessions/messages/session_tags), unit tests, verify cargo test passes.
Status: done

## Milestone 4: Rust types + AppState
### Task 4.1: Provider types (ContentBlock, ChatMessage, ProviderInfo, AgentEvent)
Approach: providers/types.rs with serde types, tools/definitions.rs with 16 tool definitions + tests, AppState wiring in lib.rs with stub modules, verify cargo check.
Status: done

## Milestone 5: Session/message CRUD
### Task 5.1: Session CRUD commands + tests
Approach: commands/sessions.rs with create/list/get/update/archive, unit tests, verify cargo test.
Status: done
### Task 5.2: Message CRUD commands + tests
Approach: commands/messages.rs with add/list storing content as JSON, unit tests, verify cargo test.
Status: done

## Milestone 6: Tool executor
### Task 6.1: File ops tools (read/write/patch/create/delete/move/list/search)
Approach: tools/file_ops.rs with tests using tempfile; add walkdir/regex/diffy deps.
Status: done
### Task 6.2: Shell + git + web tools
Approach: tools/shell.rs, tools/git.rs, tools/web.rs (DDG lite search, fetch_url) with tests; wire dispatch in tools/mod.rs.
Status: done

## Milestone 7: Providers + agentic loop
### Task 7.1: Anthropic provider (SSE streaming, tool_use aware)
Approach: providers/anthropic.rs parsing content_block events into ContentBlock vec, emitting Token events over Channel.
Status: done
### Task 7.2: OpenAI-compatible provider
Approach: providers/openai_compat.rs streaming chat completions; deepseek + opencode factories.
Status: done
### Task 7.3: Agentic loop command + tool approval
Approach: commands/providers.rs run_agentic_loop with history load, tool execution, approve/deny via oneshot; commands/tools.rs approve/deny handlers.
Status: done

## Milestone 8: Frontend IPC layer
### Task 8.1: TypeScript types + IPC wrappers
Approach: src/lib/types.ts (Session, Message, ContentBlock, AgentEvent) and src/lib/tauri.ts (typed invoke wrappers + Channel-based agentic loop).
Status: done

## Milestone 9: Svelte stores
### Task 9.1: Sessions, providers, chat stores
Approach: stores/sessions.svelte.ts, providers.svelte.ts, chat.svelte.ts with $state runes and agentic event handling.
Status: done

## Milestone 10: Chat UI components
### Task 10.1: ToolCard, ChatMessage, MessageList components
Approach: ToolCard with approve/deny buttons and status dot; ChatMessage rendering text/tool blocks; MessageList with streaming token + pending tools.
Status: done
### Task 10.2: InputArea, StatusBar, ChatPanel components
Approach: InputArea with model picker + auto-approve + send; StatusBar plugin slot; ChatPanel assembly.
Status: done

## Milestone 11: App shell
### Task 11.1: TabBar + ModelPicker + app root
Approach: TabBar browser-style tabs; ModelPicker searchable dropdown; app.svelte wiring stores and layout; smoke test.
Status: done

## Milestone 12: Settings
### Task 12.1: Settings layout + providers keys + appearance theme
Approach: routes/settings layout with nav, providers page for API keys, appearance page theme switcher, theme restore on startup.
Status: done

## Final verification
### Task F.1: Full test suite + build
Approach: npx vitest run, cd src-tauri && cargo test, npm run tauri build.
Status: done
