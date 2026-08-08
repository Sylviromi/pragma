-- File-op journal for "return to this point" reverts.
-- Before every mutating file tool (write_file / patch_file / delete_file /
-- move_file) the agentic loop stores the pre-op state of the affected file(s).
-- Reverting a conversation then replays the ops after the checkpoint in
-- reverse, restoring the filesystem to its state at that moment.
CREATE TABLE IF NOT EXISTS file_snapshots (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id     TEXT NOT NULL,
    message_rowid  INTEGER NOT NULL,   -- rowid of the assistant message that carried the tool_use
    tool_use_id    TEXT NOT NULL,
    tool_name      TEXT NOT NULL,
    path           TEXT NOT NULL,      -- absolute path of the primary affected file
    content_before TEXT NOT NULL,      -- file contents before the op ('' if it didn't exist)
    existed_before INTEGER NOT NULL,   -- 1 if the file existed before the op
    extra_before   TEXT NOT NULL DEFAULT '{}'  -- move_file: { dest, dest_content, dest_existed }
);

CREATE INDEX IF NOT EXISTS idx_snapshots_session ON file_snapshots(session_id, message_rowid);