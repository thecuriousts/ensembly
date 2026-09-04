/**
 * SQLite activity store — durable local SoT (node:sqlite DatabaseSync).
 * One writer, WAL; path under gitignored data/local by default.
 */
import fs from 'node:fs';
import path from 'node:path';
import {
  normalizeActivityEntry,
  filterActivityEntries,
  ACTIVITY_IR_VERSION,
} from './ir.js';
import { resolveActivityDbPath } from './paths.js';

const SCHEMA_VERSION = 1;

const SCHEMA_SQL = `
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA temp_store = MEMORY;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS activity_events (
  id TEXT PRIMARY KEY,
  ts TEXT NOT NULL,
  kind TEXT NOT NULL,
  actor TEXT,
  correlation_id TEXT,
  payload_json TEXT NOT NULL,
  ir_version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_activity_events_ts ON activity_events(ts);
CREATE INDEX IF NOT EXISTS idx_activity_events_kind ON activity_events(kind);
CREATE INDEX IF NOT EXISTS idx_activity_events_actor ON activity_events(actor);
`;

/**
 * Dynamically load node:sqlite so pure imports of activity kit stay free of
 * hard dependency when only memory backend is used.
 * @returns {typeof import('node:sqlite')}
 */
async function loadSqlite() {
  return import('node:sqlite');
}

/**
 * Open durable SQLite activity store.
 * @param {{ root?: string, dbPath?: string, filePath?: string }} [opts]
 * @returns {Promise<object>}
 */
export async function openSqliteActivityStore(opts = {}) {
  const filePath = opts.filePath || resolveActivityDbPath(opts);
  fs.mkdirSync(path.dirname(filePath), { recursive: true });

  const { DatabaseSync } = await loadSqlite();
  const db = new DatabaseSync(filePath);
  db.exec(SCHEMA_SQL);
  migrate(db);

  let closed = false;

  function assertOpen() {
    if (closed) throw new Error('activity store is closed');
  }

  const store = {
    backend: 'sqlite',
    path: filePath,

    /**
     * @param {object} input
     * @param {{ now?: string|Date }} [appendOpts]
     */
    append(input, appendOpts = {}) {
      assertOpen();
      const entry = normalizeActivityEntry(input, appendOpts);
      const now = new Date().toISOString();
      try {
        db.prepare(
          `INSERT INTO activity_events
            (id, ts, kind, actor, correlation_id, payload_json, ir_version, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        ).run(
          entry.id,
          entry.ts,
          entry.kind,
          entry.actor,
          entry.correlationId,
          JSON.stringify(entry.payload ?? {}),
          entry.version ?? ACTIVITY_IR_VERSION,
          now,
        );
      } catch (err) {
        const msg = String(err?.message || err);
        if (msg.includes('UNIQUE') || msg.includes('unique')) {
          throw new Error(`duplicate activity id: ${entry.id}`);
        }
        throw err;
      }
      return entry;
    },

    /**
     * @param {object} [query]
     */
    list(query = {}) {
      assertOpen();
      // Fetch ordered; apply pure filter for kinds/actor/since (SQL for common paths)
      const params = [];
      const clauses = [];
      if (query.since) {
        clauses.push('ts >= ?');
        params.push(new Date(query.since).toISOString());
      }
      if (query.until) {
        clauses.push('ts <= ?');
        params.push(new Date(query.until).toISOString());
      }
      if (query.kind) {
        clauses.push('kind = ?');
        params.push(String(query.kind));
      }
      if (query.actor != null) {
        clauses.push('actor = ?');
        params.push(String(query.actor));
      }
      const where = clauses.length ? `WHERE ${clauses.join(' AND ')}` : '';
      const rows = db
        .prepare(
          `SELECT id, ts, kind, actor, correlation_id, payload_json, ir_version
           FROM activity_events ${where}
           ORDER BY ts ASC, id ASC`,
        )
        .all(...params);

      const entries = rows.map(rowToEntry);
      // kinds[] multi-filter + limit/offset via pure IR (keeps one code path honest)
      return filterActivityEntries(entries, {
        kinds: query.kinds,
        limit: query.limit,
        offset: query.offset,
      });
    },

    size() {
      assertOpen();
      const row = db.prepare('SELECT COUNT(*) AS n FROM activity_events').get();
      return Number(row?.n ?? 0);
    },

    close() {
      if (closed) return;
      closed = true;
      try {
        db.close();
      } catch {
        /* ignore double-close */
      }
    },
  };

  return store;
}

function migrate(db) {
  const row = db
    .prepare('SELECT MAX(version) AS v FROM schema_migrations')
    .get();
  const current = Number(row?.v ?? 0);
  if (current >= SCHEMA_VERSION) return;
  db.prepare(
    'INSERT INTO schema_migrations (version, applied_at) VALUES (?, ?)',
  ).run(SCHEMA_VERSION, new Date().toISOString());
}

function rowToEntry(row) {
  let payload = {};
  try {
    payload = JSON.parse(row.payload_json || '{}');
  } catch {
    payload = {};
  }
  return normalizeActivityEntry({
    version: row.ir_version ?? ACTIVITY_IR_VERSION,
    id: row.id,
    ts: row.ts,
    kind: row.kind,
    actor: row.actor,
    correlationId: row.correlation_id,
    payload,
  });
}
