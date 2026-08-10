-- Milestone 6 telemetry: per-session usage, touched files, plan items.
-- All statements are guarded where possible; the migration runner treats this
-- file as idempotent against :memory: test DBs (CREATE TABLE IF NOT EXISTS).

CREATE TABLE IF NOT EXISTS session_usage (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    model TEXT NOT NULL DEFAULT '',
    provider TEXT NOT NULL DEFAULT '',
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd REAL NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_session_usage_session ON session_usage(session_id);

CREATE TABLE IF NOT EXISTS touched_files (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('read','edited','created','deleted')),
    added INTEGER,
    removed INTEGER,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_touched_files_session ON touched_files(session_id);

CREATE TABLE IF NOT EXISTS plan_items (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    idx INTEGER NOT NULL,
    label TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'todo' CHECK (state IN ('todo','active','done')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (session_id, idx)
);