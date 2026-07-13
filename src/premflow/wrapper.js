/**
 * ensembly wrapper around the real `premflow` binary — single invent surface
 * for micro-capture without a second data store.
 */
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {
  resolveSharedCaptureRoot,
  resolveSharedCapturePaths,
  ensureLifeOsCaptureLink,
  readSharedCaptureSnapshot,
  isCaptureLinkHealthy,
  resolveLifeOsCaptureLinkPath,
} from './paths.js';
import { projectCaptureForShare } from './redact.js';

/** Subcommands forwarded to premflow (passthrough after `flow`) */
export const PREMFLOW_FORWARD_SUBS = Object.freeze([
  'note',
  'win',
  'task',
  'pomo',
  'journal',
  'stats',
  'review',
  'search',
  'edit',
  'config',
]);

/**
 * Resolve premflow executable (PATH or common install locations).
 * @param {{ bin?: string, pathEnv?: string }} [opts]
 * @returns {string|null}
 */
export function resolvePremflowBin(opts = {}) {
  if (opts.bin) return opts.bin;
  if (process.env.PREMFLOW_BIN) return process.env.PREMFLOW_BIN;

  const which = spawnSync('which', ['premflow'], {
    encoding: 'utf8',
    env: { ...process.env, PATH: opts.pathEnv || process.env.PATH },
  });
  if (which.status === 0 && which.stdout.trim()) {
    return which.stdout.trim().split('\n')[0];
  }

  const home = process.env.HOME || '';
  const candidates = [
    path.join(home, '.local', 'bin', 'premflow'),
    path.join(home, 'Work', 'personal', 'premflow', 'build', 'premflow'),
    path.join(home, 'Work', 'personal', 'premflow', 'premflow'),
  ];
  for (const c of candidates) {
    if (c && fs.existsSync(c)) return c;
  }
  return null;
}

/**
 * Build spawn plan for premflow (testable pure structure).
 * @param {string[]} forwardArgs args after `flow` (e.g. ['task','list'] or [])
 * @param {{ bin?: string, home?: string }} [opts]
 */
export function buildPremflowSpawn(forwardArgs = [], opts = {}) {
  const bin = resolvePremflowBin(opts);
  const dataDir = resolveSharedCaptureRoot({ home: opts.home });
  const args = Array.isArray(forwardArgs) ? forwardArgs.filter((a) => a != null) : [];
  return {
    bin,
    args,
    dataDir,
    /** Premflow uses HOME/.premflow — keep HOME so SoT stays one place */
    env: {
      HOME: opts.home || process.env.HOME,
      PATH: process.env.PATH,
      ...opts.env,
    },
    missingBin: !bin,
  };
}

/**
 * Run premflow with shared SoT. Does not reimplement premflow logic.
 * @param {string[]} forwardArgs
 * @param {{ bin?: string, home?: string, encoding?: string }} [opts]
 */
export function runPremflow(forwardArgs = [], opts = {}) {
  const plan = buildPremflowSpawn(forwardArgs, opts);
  if (plan.missingBin) {
    return {
      ok: false,
      status: 127,
      stdout: '',
      stderr:
        'premflow binary not found on PATH (install ~/Work/personal/premflow → ~/.local/bin). Shared data would be at: ' +
        plan.dataDir,
      plan,
    };
  }

  // Ensure data dir exists so first note does not fail mid-flight
  fs.mkdirSync(plan.dataDir, { recursive: true });

  const r = spawnSync(plan.bin, plan.args, {
    encoding: opts.encoding || 'utf8',
    env: { ...process.env, ...plan.env },
    cwd: opts.cwd || process.cwd(),
  });

  return {
    ok: r.status === 0,
    status: r.status ?? 1,
    stdout: r.stdout || '',
    stderr: r.stderr || '',
    error: r.error ? String(r.error.message || r.error) : null,
    plan,
  };
}

/**
 * Describe shared mesh status for `flow path` / agents.
 * @param {{ home?: string, lifeOsRoot?: string, shareSafe?: boolean }} [opts]
 *        shareSafe: true → never include raw private todo text (for export/logs)
 */
export function sharedCaptureStatus(opts = {}) {
  const sharedRoot = resolveSharedCaptureRoot(opts);
  const paths = resolveSharedCapturePaths({ root: sharedRoot });
  const linkPath = resolveLifeOsCaptureLinkPath(opts);
  const linkOk = isCaptureLinkHealthy(linkPath, sharedRoot);
  const snap = readSharedCaptureSnapshot({ root: sharedRoot, todoLimit: 20, logLimit: 10 });
  const bin = resolvePremflowBin(opts);
  const projection = projectCaptureForShare(snap);

  const shareSafe = opts.shareSafe === true;
  return {
    sharedRoot,
    paths,
    lifeOsCaptureLink: linkPath,
    lifeOsLinkHealthy: linkOk,
    premflowBin: bin,
    todoCount: snap.todos.length,
    /** Local TTY may show raw samples; shareSafe uses redacted projection only */
    sampleTodos: shareSafe
      ? projection.shareable.todos.slice(0, 5)
      : snap.todos.slice(0, 5),
    journalFiles: snap.journalFiles,
    privacy: {
      captureIsPrivateByDefault: true,
      symlinkDoesNotCopyBytes: true,
      neverCommitCapture: true,
      shareableProjection: projection.shareable,
      policy: projection.policy,
    },
    law: 'one filesystem SoT (~/.premflow); ensembly flow wrapper; life-os capture symlink; day/HITL stays ensembly; derived insights redacted for share',
  };
}

export { ensureLifeOsCaptureLink, readSharedCaptureSnapshot };
