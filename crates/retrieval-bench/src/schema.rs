//! Pulse-memory schema copy from `ensembly-kernel` fixture tests.

pub const PULSE_SCHEMA: &str = "
CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
CREATE TABLE memory_traces (
  id INTEGER PRIMARY KEY, nat_key TEXT NOT NULL UNIQUE, snippet TEXT NOT NULL, source TEXT NOT NULL,
  time TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('user-stated','tool-verified','inferred','unknown')),
  kind TEXT NOT NULL CHECK (kind IN ('data','context','fact','memory')),
  lock TEXT NOT NULL DEFAULT 'open' CHECK (lock IN ('open','locked')),
  verify_hash TEXT, prefetch_key TEXT, seen_count INTEGER NOT NULL DEFAULT 1, superseded_by TEXT,
  first_seen TEXT NOT NULL, last_seen TEXT NOT NULL);
CREATE TABLE conflicts (id INTEGER PRIMARY KEY, subject TEXT NOT NULL, left_id INTEGER NOT NULL,
  right_id INTEGER NOT NULL, resolution TEXT NOT NULL, ts TEXT NOT NULL);
CREATE VIRTUAL TABLE memory_fts USING fts5(nat_key, snippet, source, content='memory_traces', content_rowid='id');
INSERT INTO schema_migrations VALUES (1, '2026-01-01T00:00:00+00:00');
";
