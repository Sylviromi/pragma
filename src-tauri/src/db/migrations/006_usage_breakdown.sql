-- 006: record the real per-part prompt breakdown with each usage row.
-- The agentic loop measures what it actually sends (system prompt + tool
-- schemas, conversation blocks, tool_result payloads, chars/4) and stores it
-- here, so the sidebar's ring composition comes from REAL values instead of
-- a hardcoded system estimate or re-deriving from stored message JSON.
-- Nullable: legacy rows (pre-006) have no breakdown → stats falls back to
-- char-estimates for those sessions.
ALTER TABLE session_usage ADD COLUMN system_tokens INTEGER;
ALTER TABLE session_usage ADD COLUMN conversation_tokens INTEGER;
ALTER TABLE session_usage ADD COLUMN files_tokens INTEGER;