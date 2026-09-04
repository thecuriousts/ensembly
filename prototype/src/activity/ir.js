/**
 * Activity / log IR — pure, versioned append shapes.
 * IR = Intermediate Representation: middle contract between kernel and hosts
 * (not LLVM). Durable SoT adapters (memory, SQLite) consume these shapes.
 *
 * @typedef {object} ActivityEntry
 * @property {number} version schema version (currently 1)
 * @property {string} id stable unique id
 * @property {string} ts ISO-8601 timestamp (ordering key)
 * @property {string} kind activity or log type (e.g. 'activity.claim', 'log.info')
 * @property {string|null} [actor] who/what produced it (operator, swarm, game, …)
 * @property {string|null} [correlationId] links related events
 * @property {object} [payload] free-form structured body
 */

export const ACTIVITY_IR_VERSION = 1;

/** Common kinds (convention; not exhaustive enum) */
export const ACTIVITY_KINDS = Object.freeze({
  ACTIVITY: 'activity',
  LOG: 'log',
  CLAIM: 'activity.claim',
  COMPLETE: 'activity.complete',
  APPROVE: 'activity.approve',
  DENY: 'activity.deny',
  TURN: 'activity.turn',
  LOG_INFO: 'log.info',
  LOG_WARN: 'log.warn',
  LOG_ERROR: 'log.error',
});

/**
 * @param {Partial<ActivityEntry> & { kind: string }} input
 * @param {{ now?: string|Date, id?: string }} [opts]
 * @returns {ActivityEntry}
 */
export function normalizeActivityEntry(input = {}, opts = {}) {
  if (!input || typeof input !== 'object') {
    throw new TypeError('activity entry must be an object');
  }
  const kind = String(input.kind || '').trim();
  if (!kind) throw new TypeError('activity entry requires kind');

  const ts = coerceIso(input.ts ?? opts.now ?? new Date());
  const id =
    input.id ||
    opts.id ||
    makeActivityId(kind, ts);

  let payload = input.payload;
  if (payload === undefined || payload === null) payload = {};
  if (typeof payload !== 'object' || Array.isArray(payload)) {
    throw new TypeError('payload must be a plain object');
  }

  return {
    version: ACTIVITY_IR_VERSION,
    id: String(id),
    ts,
    kind,
    actor: input.actor == null || input.actor === '' ? null : String(input.actor),
    correlationId:
      input.correlationId == null || input.correlationId === ''
        ? null
        : String(input.correlationId),
    payload: { ...payload },
  };
}

/**
 * Pure filter/sort of in-memory entry lists (also used after SQLite fetch).
 * @param {ActivityEntry[]} entries
 * @param {{ since?: string, until?: string, kind?: string, kinds?: string[], actor?: string, limit?: number, offset?: number }} [query]
 * @returns {ActivityEntry[]}
 */
export function filterActivityEntries(entries = [], query = {}) {
  const since = query.since ? coerceIso(query.since) : null;
  const until = query.until ? coerceIso(query.until) : null;
  const kinds = query.kinds
    ? new Set(query.kinds.map(String))
    : query.kind
      ? new Set([String(query.kind)])
      : null;
  const actor = query.actor != null ? String(query.actor) : null;
  const offset = Math.max(0, Number(query.offset) || 0);
  const limit =
    query.limit == null || query.limit === undefined
      ? null
      : Math.max(0, Number(query.limit) || 0);

  let out = (entries || []).filter((e) => {
    if (!e) return false;
    if (since && e.ts < since) return false;
    if (until && e.ts > until) return false;
    if (kinds && !kinds.has(e.kind)) return false;
    if (actor != null && e.actor !== actor) return false;
    return true;
  });

  out = out.slice().sort((a, b) => {
    if (a.ts < b.ts) return -1;
    if (a.ts > b.ts) return 1;
    return String(a.id).localeCompare(String(b.id));
  });

  if (offset) out = out.slice(offset);
  if (limit != null) out = out.slice(0, limit);
  return out;
}

/**
 * Serialize entry for transport / SQL payload column.
 * @param {ActivityEntry} entry
 */
export function serializeActivityEntry(entry) {
  const e = normalizeActivityEntry(entry);
  return JSON.stringify(e);
}

/**
 * @param {string|object} raw
 * @returns {ActivityEntry}
 */
export function parseActivityEntry(raw) {
  const obj = typeof raw === 'string' ? JSON.parse(raw) : raw;
  return normalizeActivityEntry(obj);
}

function coerceIso(v) {
  if (v instanceof Date) return v.toISOString();
  if (typeof v === 'number' && Number.isFinite(v)) return new Date(v).toISOString();
  if (typeof v === 'string' && v.trim()) {
    const d = new Date(v);
    if (Number.isNaN(d.getTime())) throw new TypeError(`invalid timestamp: ${v}`);
    return d.toISOString();
  }
  return new Date().toISOString();
}

function makeActivityId(kind, ts) {
  const slug = String(kind).replace(/[^a-zA-Z0-9._-]+/g, '_').slice(0, 40);
  const rand = Math.random().toString(36).slice(2, 10);
  return `act_${slug}_${Date.parse(ts) || Date.now()}_${rand}`;
}
