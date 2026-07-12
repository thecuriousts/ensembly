/**
 * Ingest persona + optional local state. IO isolated here.
 */
import fs from 'node:fs';
import path from 'node:path';

/**
 * Load JSON file if present.
 * @param {string} filePath
 */
export function loadJson(filePath) {
  const raw = fs.readFileSync(filePath, 'utf8');
  return JSON.parse(raw);
}

/**
 * Prefer full local persona when present; fall back to public projection.
 * @param {string} root repo root
 */
export function loadPersona(root) {
  const fullPath = path.join(root, 'private', 'persona', 'full.json');
  const publicPath = path.join(root, 'public', 'persona', 'projection.json');
  if (fs.existsSync(fullPath)) {
    return { persona: loadJson(fullPath), source: 'private/persona/full.json', path: fullPath };
  }
  if (fs.existsSync(publicPath)) {
    return { persona: loadJson(publicPath), source: 'public/persona/projection.json', path: publicPath };
  }
  throw new Error('No persona found (private/persona/full.json or public/persona/projection.json)');
}

/**
 * Optional runtime state: recent area counts, extra candidates, notes.
 * @param {string} root
 */
export function loadLocalState(root) {
  const candidates = [
    path.join(root, 'private', 'state', 'current.json'),
    path.join(root, 'data', 'local', 'state.json'),
    path.join(root, 'fixtures', 'state-sample.json'),
  ];
  for (const p of candidates) {
    if (fs.existsSync(p)) {
      return { state: loadJson(p), path: p };
    }
  }
  return {
    state: {
      area_counts: {},
      extra_candidates: [],
      notes: [],
      date: new Date().toISOString().slice(0, 10),
    },
    path: null,
  };
}

/**
 * Resolve repo root from cwd or import meta.
 */
export function resolveRoot(start = process.cwd()) {
  let dir = path.resolve(start);
  for (let i = 0; i < 8; i++) {
    if (fs.existsSync(path.join(dir, 'package.json')) && fs.existsSync(path.join(dir, 'public', 'persona'))) {
      return dir;
    }
    if (fs.existsSync(path.join(dir, 'package.json')) && fs.existsSync(path.join(dir, 'src', 'day.js'))) {
      return dir;
    }
    const parent = path.dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return path.resolve(start);
}
