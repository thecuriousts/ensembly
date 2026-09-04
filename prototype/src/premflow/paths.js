/**
 * Shared micro-capture paths — one filesystem SoT for notes/tasks/journal/pomo.
 *
 * Premflow C binary hardcodes HOME/.premflow (DATA_DIR). That directory **is**
 * the byte SoT. life-os and ensembly link/view the same inode tree; they do not
 * invent a second todo.txt.
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

/** Premflow data dir name under $HOME (must match premflow.h DATA_DIR) */
export const PREMFLOW_DATA_DIRNAME = '.premflow';

/** Subdir name under life-os Projects/premflow that symlinks to the shared tree */
export const LIFE_OS_CAPTURE_LINK_NAME = 'capture';

/** Files/dirs that constitute shared micro-capture (premflow layout) */
export const SHARED_CAPTURE_ENTRIES = Object.freeze([
  'todo.txt',
  'log.txt',
  'journal',
  'config.txt',
]);

/**
 * Absolute path to the shared micro-capture root (premflow data home).
 * @param {{ home?: string }} [opts]
 */
export function resolveSharedCaptureRoot(opts = {}) {
  const home = opts.home || process.env.HOME || os.homedir();
  if (!home) throw new Error('HOME not set — cannot resolve shared capture root');
  return path.join(home, PREMFLOW_DATA_DIRNAME);
}

/**
 * life-os portfolio project card root for premflow product.
 * @param {{ lifeOsRoot?: string, home?: string }} [opts]
 */
export function resolveLifeOsPremflowCard(opts = {}) {
  const lifeOs =
    opts.lifeOsRoot ||
    process.env.LIFE_OS_ROOT ||
    path.join(opts.home || process.env.HOME || os.homedir(), 'life-os');
  return path.join(lifeOs, 'Projects', 'premflow');
}

/**
 * Path of the capture symlink inside the life-os premflow project card.
 * @param {{ lifeOsRoot?: string, home?: string }} [opts]
 */
export function resolveLifeOsCaptureLinkPath(opts = {}) {
  return path.join(resolveLifeOsPremflowCard(opts), LIFE_OS_CAPTURE_LINK_NAME);
}

/**
 * Absolute paths for the main shared files.
 * @param {{ home?: string, root?: string }} [opts]
 */
export function resolveSharedCapturePaths(opts = {}) {
  const root = opts.root || resolveSharedCaptureRoot(opts);
  return {
    root,
    todo: path.join(root, 'todo.txt'),
    log: path.join(root, 'log.txt'),
    journal: path.join(root, 'journal'),
    config: path.join(root, 'config.txt'),
  };
}

/**
 * Whether `linkPath` is a symlink whose realpath equals `sharedRoot`.
 * @param {string} linkPath
 * @param {string} sharedRoot
 */
export function isCaptureLinkHealthy(linkPath, sharedRoot) {
  try {
    if (!fs.existsSync(linkPath)) return false;
    const st = fs.lstatSync(linkPath);
    if (!st.isSymbolicLink()) return false;
    const target = fs.realpathSync(linkPath);
    const want = fs.realpathSync(sharedRoot);
    return target === want;
  } catch {
    return false;
  }
}

/**
 * Create/repair symlink: life-os Projects/premflow/capture → shared root.
 * Pure enough to test with temp dirs; does not move existing data.
 *
 * @param {{
 *   home?: string,
 *   lifeOsRoot?: string,
 *   sharedRoot?: string,
 *   force?: boolean,
 * }} [opts]
 * @returns {{ ok: boolean, linkPath: string, sharedRoot: string, action: string, error?: string }}
 */
export function ensureLifeOsCaptureLink(opts = {}) {
  const sharedRoot = opts.sharedRoot || resolveSharedCaptureRoot(opts);
  const linkPath = resolveLifeOsCaptureLinkPath(opts);
  const card = path.dirname(linkPath);

  try {
    fs.mkdirSync(sharedRoot, { recursive: true });
    fs.mkdirSync(card, { recursive: true });

    if (isCaptureLinkHealthy(linkPath, sharedRoot)) {
      return { ok: true, linkPath, sharedRoot, action: 'unchanged' };
    }

    if (fs.existsSync(linkPath)) {
      const st = fs.lstatSync(linkPath);
      if (st.isSymbolicLink()) {
        fs.unlinkSync(linkPath);
      } else if (!opts.force) {
        return {
          ok: false,
          linkPath,
          sharedRoot,
          action: 'blocked',
          error: `path exists and is not a symlink (refusing clobber): ${linkPath}`,
        };
      } else {
        fs.rmSync(linkPath, { recursive: true, force: true });
      }
    }

    fs.symlinkSync(sharedRoot, linkPath, 'dir');
    return { ok: true, linkPath, sharedRoot, action: 'created' };
  } catch (err) {
    return {
      ok: false,
      linkPath,
      sharedRoot,
      action: 'error',
      error: String(err?.message || err),
    };
  }
}

/**
 * Snapshot of shared store for agent/ensembly views (read-only, no spawn).
 * @param {{ home?: string, root?: string, todoLimit?: number, logLimit?: number }} [opts]
 */
export function readSharedCaptureSnapshot(opts = {}) {
  const paths = resolveSharedCapturePaths(opts);
  const todoLimit = opts.todoLimit ?? 50;
  const logLimit = opts.logLimit ?? 30;

  const readLines = (file, limit) => {
    if (!fs.existsSync(file)) return [];
    const raw = fs.readFileSync(file, 'utf8');
    const lines = raw.split('\n').filter((l) => l.trim().length > 0);
    return lines.slice(-limit);
  };

  let journalFiles = [];
  if (fs.existsSync(paths.journal) && fs.statSync(paths.journal).isDirectory()) {
    journalFiles = fs
      .readdirSync(paths.journal)
      .filter((f) => f.startsWith('journal-') || f.endsWith('.txt'))
      .sort()
      .slice(-10);
  }

  return {
    root: paths.root,
    todoPath: paths.todo,
    logPath: paths.log,
    journalPath: paths.journal,
    todos: readLines(paths.todo, todoLimit),
    logTail: readLines(paths.log, logLimit),
    journalFiles,
    exists: fs.existsSync(paths.root),
  };
}
