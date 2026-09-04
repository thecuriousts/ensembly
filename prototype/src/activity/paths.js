/**
 * Default durable activity store paths — always under gitignored local data.
 */
import path from 'node:path';
import { resolveRoot } from '../ingest.js';

/** Relative to repo root — gitignored via data/ + data/local/ */
export const DEFAULT_ACTIVITY_DB_REL = path.join('data', 'local', 'activity.sqlite');

/**
 * @param {{ root?: string, dbPath?: string }} [opts]
 * @returns {string} absolute path to SQLite file
 */
export function resolveActivityDbPath(opts = {}) {
  if (opts.dbPath) {
    return path.isAbsolute(opts.dbPath)
      ? opts.dbPath
      : path.resolve(opts.root || resolveRoot(), opts.dbPath);
  }
  const root = opts.root || resolveRoot();
  return path.join(root, DEFAULT_ACTIVITY_DB_REL);
}
