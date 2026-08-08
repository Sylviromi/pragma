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