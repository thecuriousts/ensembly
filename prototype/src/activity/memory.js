/**
 * In-memory activity store — tests / hot session; same port as SQLite.
 */
import {
  normalizeActivityEntry,
  filterActivityEntries,
} from './ir.js';

/**
 * @param {{ seed?: object[] }} [opts]
 * @returns {{ backend: 'memory', append: Function, list: Function, close: Function, size: Function }}
 */
export function createMemoryActivityStore(opts = {}) {
  /** @type {import('./ir.js').ActivityEntry[]} */
  let rows = [];
  if (Array.isArray(opts.seed)) {
    rows = opts.seed.map((e) => normalizeActivityEntry(e));
  }
  let closed = false;

  function assertOpen() {
    if (closed) throw new Error('activity store is closed');
  }

  return {
    backend: 'memory',
    path: null,

    /**
     * @param {object} input
     * @param {{ now?: string|Date }} [appendOpts]
     */
    append(input, appendOpts = {}) {
      assertOpen();
      const entry = normalizeActivityEntry(input, appendOpts);
      if (rows.some((r) => r.id === entry.id)) {
        throw new Error(`duplicate activity id: ${entry.id}`);
      }
      rows.push(entry);
      return entry;
    },

    /**
     * @param {object} [query]
     */
    list(query = {}) {
      assertOpen();
      return filterActivityEntries(rows, query);
    },

    size() {
      assertOpen();
      return rows.length;
    },

    close() {
      closed = true;
    },
  };
}
