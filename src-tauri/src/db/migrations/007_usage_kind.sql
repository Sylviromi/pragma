-- 007: usage kind — distinguishes agent-loop calls from harness verification
-- calls (reviewer). Default 'agent' keeps existing rows classified correctly.
ALTER TABLE session_usage ADD COLUMN kind TEXT NOT NULL DEFAULT 'agent';
