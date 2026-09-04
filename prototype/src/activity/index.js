/**
 * Activity / log store public surface.
 * Kernel uses the port (append/list/close); adapters are memory | sqlite.
 * Default durable path: data/local/activity.sqlite (gitignored).
 */
export {
  ACTIVITY_IR_VERSION,
  ACTIVITY_KINDS,
  normalizeActivityEntry,
  filterActivityEntries,
  serializeActivityEntry,
  parseActivityEntry,
} from './ir.js';
export { createMemoryActivityStore } from './memory.js';
export { openSqliteActivityStore } from './sqlite.js';
export { resolveActivityDbPath, DEFAULT_ACTIVITY_DB_REL } from './paths.js';

/**
 * Open an activity store by backend.
 * - backend: 'memory' | 'sqlite' (default 'sqlite' when durable needed; use memory in tests)
 * Lazy: does not open SQLite until called with backend sqlite.
 *
 * @param {{ backend?: 'memory'|'sqlite', root?: string, dbPath?: string, filePath?: string, seed?: object[] }} [opts]
 */
export async function openActivityStore(opts = {}) {
  const backend = opts.backend || 'sqlite';
  if (backend === 'memory') {
    const { createMemoryActivityStore } = await import('./memory.js');
    return createMemoryActivityStore({ seed: opts.seed });
  }
  if (backend === 'sqlite') {
    const { openSqliteActivityStore } = await import('./sqlite.js');
    return openSqliteActivityStore(opts);
  }
  throw new Error(`unknown activity store backend: ${backend}`);
}
