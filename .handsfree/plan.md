# Fix: model repeatedly re-calls the same tool & ignores new prompts

## Diagnosis (reproduced from the user's own sessions.db)

**Root cause — history corruption hides the user's prompts from the model.**

1. `run_agentic_loop` persists the assistant `tool_use` message to the DB
   **before** executing the tool and persisting its `tool_result`. Any
   interleaving or interruption leaves a **dangling tool_use** (no matching
   result) in `messages`. Evidence in the DB:
   - duplicated user prompts (`[0]` & `[30]`, `[38]` & `[40]`) → concurrent
     loops (two app instances share `sessions.db`; `running_sessions` guard is
     per-process only), and
   - out-of-order writes: `assistant(tool_use) → assistant(text) →
     tool_result` (`[34]→[35]→[36]`, `[44]→[45]→[46]`).

2. `to_openai_messages()` in `openai_compat.rs` sees the dangling tool_call
   and runs `out.truncate(i)` — **cutting everything after it, INCLUDING the
   user's brand-new prompt**. Verified with a simulator against the real DB:
   of 4 user prompts, only the first ever reached the model. The model then
   only sees the tail end of the old tool-call exchanges → it keeps calling the
   same tool and never sees "unrelated" prompts. The loop then re-persists new
   messages *after* the dangling one, so every subsequent request truncates at
   the same spot → the pattern perpetuates.

3. No context-window management at all: `history` is re-sent in full every
   iteration, results are never capped, and `context_window` metadata is never
   consulted → long/looping runs blow the window, which further degrades the
   model into "repeat the last tool call".

## Fixes

Backend (`src-tauri/src/commands/providers.rs`)
- Repair/drop dangling tool exchanges at history **load** time (works for both
  providers; also fixes Anthropic hard-400 on dangling tool_use).
- Cap tool-result outputs (truncate to ~32k chars) before persisting.
- Trim history to a `MAX_HISTORY_CHARS` budget before each provider call,
  always keeping system prompt + newest user message, dropping oldest complete
  tool exchanges first.
- Cross-process single-loop lock: new `active_loops` table + `INSERT …
  ON CONFLICT DO NOTHING` (two app instances can't both run a loop on the same
  session).
- RAII guard for `running_sessions` (panics/`?` can't leave a session stuck).

`openai_compat.rs`
- Sanitizer must never drop the newest user message (keep trailing user turns
  when truncating), and must drop stray `role:tool` orphans.

`tools/shell.rs`
- Implement the promised 60s timeout + cap output size.

Frontend (`src/stores/chat.svelte.ts`)
- Persist/restore `running` flag across HMR (textarea was enabled while a loop
  ran → duplicate sends).
- On "Session already has an active agentic loop" error: keep UI in running
  state and re-subscribe instead of flipping to idle.

## Tests
- Unit tests for repair history, sanitizer truncation (user prompt survives),
  and the trim function. `cargo test` + `npm run check`.

## Status — implemented & verified
- All backend units + new tests: `cargo test` 45 passed.
- `npm run check`: 0 errors. `npm run test`: pass.
- Verified the new repair logic against the user's real corrupted session: all
  4 real user prompts + new unrelated prompt now reach the model (previously
  only 1 of 4 survived).
- Lock semantics (INSERT ON CONFLICT / token-checked DELETE) validated against
  a copy of the real DB.