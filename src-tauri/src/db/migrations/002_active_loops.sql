-- Cross-process agentic-loop lock: one active loop per session across all app
-- instances (two app processes share sessions.db; the in-memory running_sessions
-- guard only protects within a single process).
CREATE TABLE IF NOT EXISTS active_loops (
    session_id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL
);