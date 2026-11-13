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
CREATE TABLE conflicts (id INTEGER PRIMARY KEY, subject TEXT NOT NULL, left_id INTEGER NOT NULL,
  right_id INTEGER NOT NULL, resolution TEXT NOT NULL, ts TEXT NOT NULL);
CREATE TABLE supersession_events (id INTEGER PRIMARY KEY, old_key TEXT NOT NULL, new_key TEXT NOT NULL,
  source TEXT NOT NULL, event_time TEXT NOT NULL, reason TEXT NOT NULL, writer TEXT NOT NULL,
  conflict_id INTEGER, phase TEXT NOT NULL CHECK (phase IN ('intent','applied','verified')),
  UNIQUE (old_key, new_key));
INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, lock, verify_hash, prefetch_key, seen_count, superseded_by, first_seen, last_seen) VALUES
 ('demo/f-pinned', 'pinned', 'fixture', '2026-11-01', 'tool-verified', 'fact', 'open', 'sha256:3fab5c181bd28a09b64397df76ae2bfaf1eac182979b5fdb7a342858004f36af', NULL, 1, NULL, '2026-11-01', '2026-11-01'),
 ('demo/f-null', 'no pin', 'fixture', '2026-11-01', 'tool-verified', 'fact', 'open', NULL, NULL, 1, NULL, '2026-11-01', '2026-11-01'),
 ('demo/f-empty', 'empty pin', 'fixture', '2026-11-01', 'tool-verified', 'fact', 'open', '', NULL, 1, NULL, '2026-11-01', '2026-11-01'),
 ('demo/f-wrong', 'wrong pin', 'fixture', '2026-11-01', 'tool-verified', 'fact', 'open', 'sha256:d9298a10d1b0735837dc4bd85dac641b0f3cef27a47e5d53a54f2f3f5b2fcffa', NULL, 1, NULL, '2026-11-01', '2026-11-01'),
 ('demo/f-stated', 'stated, no pin', 'fixture', '2026-11-01', 'user-stated', 'memory', 'open', NULL, NULL, 1, NULL, '2026-11-01', '2026-11-01'),
 ('demo/s-a', 'a', 'fixture', '2026-11-01', 'user-stated', 'memory', 'open', NULL, NULL, 1, NULL, '2026-11-01', '2026-11-01'),
 ('demo/s-a2', 'a2', 'fixture', '2026-11-02', 'user-stated', 'memory', 'open', NULL, NULL, 1, NULL, '2026-11-02', '2026-11-02'),
 ('demo/s-b', 'b', 'fixture', '2026-11-01', 'user-stated', 'memory', 'open', NULL, NULL, 1, 'demo/s-b2', '2026-11-01', '2026-11-01'),
 ('demo/s-b2', 'b2', 'fixture', '2026-11-02', 'user-stated', 'memory', 'open', NULL, NULL, 1, NULL, '2026-11-02', '2026-11-02'),
 ('demo/s-c', 'c', 'fixture', '2026-11-01', 'user-stated', 'memory', 'open', NULL, NULL, 1, 'demo/s-c2', '2026-11-01', '2026-11-01'),
 ('demo/s-c2', 'c2', 'fixture', '2026-11-02', 'user-stated', 'memory', 'open', NULL, NULL, 1, NULL, '2026-11-02', '2026-11-02');
INSERT INTO supersession_events (old_key, new_key, source, event_time, reason, writer, conflict_id, phase) VALUES
 ('demo/s-a', 'demo/s-a2', 'fixture', '2026-11-02T09:00:00Z', 'synthetic', 'laptop-1', NULL, 'intent'),
 ('demo/s-b', 'demo/s-b2', 'fixture', '2026-11-02T09:00:00Z', 'synthetic', 'laptop-1', NULL, 'applied'),
 ('demo/s-c', 'demo/s-c2', 'fixture', '2026-11-02T09:00:00Z', 'synthetic', 'laptop-1', NULL, 'verified');
