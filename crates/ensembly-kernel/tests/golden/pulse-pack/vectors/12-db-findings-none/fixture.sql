CREATE TABLE memory_traces (
  id INTEGER PRIMARY KEY, nat_key TEXT NOT NULL UNIQUE, snippet TEXT NOT NULL, source TEXT NOT NULL,
  time TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('user-stated','tool-verified','inferred','unknown')),
  kind TEXT NOT NULL CHECK (kind IN ('data','context','fact','memory')),
  lock TEXT NOT NULL DEFAULT 'open' CHECK (lock IN ('open','locked')),
  verify_hash TEXT, prefetch_key TEXT, seen_count INTEGER NOT NULL DEFAULT 1, superseded_by TEXT,
  first_seen TEXT NOT NULL, last_seen TEXT NOT NULL);
CREATE TABLE archive_events (id INTEGER PRIMARY KEY, ts TEXT NOT NULL, source TEXT NOT NULL,
  payload TEXT NOT NULL, correlation_id TEXT);
INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, lock, verify_hash, prefetch_key, seen_count, superseded_by, first_seen, last_seen) VALUES
 ('demo/db-b', 'db beta', 'fixture', '2026-11-02 08:15', 'tool-verified', 'fact', 'locked', 'sha256:3bb8f99d2e34c831dc44b02c67db9df52dfcbd074f9f55d510619eedb8696808', NULL, 2, NULL, '2026-11-02', '2026-11-03T09:00:00+01:00'),
 ('demo/db-a', 'db alpha', 'fixture', '2026-11-01T09:00:00+00:00', 'user-stated', 'memory', 'open', NULL, 'demo/pf', 1, 'demo/db-b', '2026-11-01T09:00:00+00:00', '2026-11-01T09:00:00+00:00');
INSERT INTO archive_events (ts, source, payload, correlation_id) VALUES
 ('2026-11-02T08:15:00Z', 'fixture', '{"admit":"demo/db-b"}', 'demo/db-b');
