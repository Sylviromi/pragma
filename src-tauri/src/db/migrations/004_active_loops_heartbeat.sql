-- Heartbeat column for the cross-process loop lock. A live loop refreshes this
-- every few seconds; a row whose heartbeat is stale means the owning process
-- died (crash / kill) and the lock can be reclaimed. Guarded in Rust so it only
-- runs once on existing databases (SQLite has no ALTER ... IF NOT EXISTS).
ALTER TABLE active_loops ADD COLUMN heartbeat_at TEXT NOT NULL DEFAULT '';