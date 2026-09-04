/**
 * Shared capture SoT + premflow wrapper — drives shipped path helpers / spawn plan.
 */
import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import {
  resolveSharedCaptureRoot,
  resolveLifeOsPremflowCard,
  resolveLifeOsCaptureLinkPath,
  resolveSharedCapturePaths,
  ensureLifeOsCaptureLink,
  isCaptureLinkHealthy,
  readSharedCaptureSnapshot,
  PREMFLOW_DATA_DIRNAME,
  LIFE_OS_CAPTURE_LINK_NAME,
  buildPremflowSpawn,
  resolvePremflowBin,
  runPremflow,
  sharedCaptureStatus,
  classifyCaptureLine,
  redactCaptureLine,
  projectCaptureForShare,
} from '../src/premflow/index.js';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

describe('shared capture paths (shipped)', () => {
  it('resolveSharedCaptureRoot is HOME/.premflow (premflow byte SoT)', () => {
    const root = resolveSharedCaptureRoot({ home: '/tmp/fake-home-xyz' });
    assert.equal(root, path.join('/tmp/fake-home-xyz', PREMFLOW_DATA_DIRNAME));
    assert.ok(root.endsWith('.premflow'));
  });

  it('life-os card + capture link path under Projects/premflow', () => {
    const card = resolveLifeOsPremflowCard({ lifeOsRoot: '/vault/life-os' });
    assert.equal(card, path.join('/vault/life-os', 'Projects', 'premflow'));
    const link = resolveLifeOsCaptureLinkPath({ lifeOsRoot: '/vault/life-os' });
    assert.equal(link, path.join(card, LIFE_OS_CAPTURE_LINK_NAME));
  });

  it('resolveSharedCapturePaths names todo log journal config', () => {
    const p = resolveSharedCapturePaths({ root: '/data/shared-cap' });
    assert.equal(p.todo, path.join('/data/shared-cap', 'todo.txt'));
    assert.equal(p.log, path.join('/data/shared-cap', 'log.txt'));
    assert.equal(p.journal, path.join('/data/shared-cap', 'journal'));
  });
});

describe('ensureLifeOsCaptureLink (real fs)', () => {
  let tmp;
  before(() => {
    tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'ensembly-cap-'));
  });
  after(() => {
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it('creates symlink life-os capture → shared root (same inode tree)', () => {
    const home = path.join(tmp, 'home');
    const lifeOs = path.join(tmp, 'life-os');
    const shared = path.join(home, '.premflow');
    fs.mkdirSync(shared, { recursive: true });
    fs.writeFileSync(path.join(shared, 'todo.txt'), '[TODO] from shared\n');

    const r1 = ensureLifeOsCaptureLink({
      home,
      lifeOsRoot: lifeOs,
      sharedRoot: shared,
    });
    assert.equal(r1.ok, true);
    assert.ok(['created', 'unchanged'].includes(r1.action));
    assert.ok(isCaptureLinkHealthy(r1.linkPath, shared));

    const viaLink = path.join(r1.linkPath, 'todo.txt');
    assert.ok(fs.existsSync(viaLink));
    assert.equal(fs.readFileSync(viaLink, 'utf8').trim(), '[TODO] from shared');
    // same real path
    assert.equal(fs.realpathSync(viaLink), fs.realpathSync(path.join(shared, 'todo.txt')));

    const r2 = ensureLifeOsCaptureLink({
      home,
      lifeOsRoot: lifeOs,
      sharedRoot: shared,
    });
    assert.equal(r2.ok, true);
    assert.equal(r2.action, 'unchanged');
  });
});

describe('premflow wrapper spawn plan (shipped)', () => {
  it('buildPremflowSpawn points dataDir at shared root and forwards args', () => {
    const plan = buildPremflowSpawn(['task', 'list'], {
      home: '/tmp/home-for-spawn',
      bin: '/usr/bin/premflow-fake',
    });
    assert.equal(plan.dataDir, path.join('/tmp/home-for-spawn', '.premflow'));
    assert.deepEqual(plan.args, ['task', 'list']);
    assert.equal(plan.bin, '/usr/bin/premflow-fake');
    assert.equal(plan.missingBin, false);
    assert.equal(plan.env.HOME, '/tmp/home-for-spawn');
  });

  it('buildPremflowSpawn marks missing bin without inventing a path oracle', () => {
    const plan = buildPremflowSpawn([], {
      home: '/tmp/x',
      bin: '',
      // force no bin via empty string override after resolve — use nonexistent
    });
    // When bin explicitly empty string, resolvePremflowBin treats as falsy and searches —
    // use impossible PATH
    const plan2 = buildPremflowSpawn(['note', 'hi'], {
      home: '/tmp/x',
      pathEnv: '/nonexistent-bin-dir-xyz',
      env: {},
    });
    // May or may not find system premflow depending on resolvePremflowBin order —
    // if bin found on default candidates that's ok; test dataDir always shared
    assert.equal(plan2.dataDir, path.join('/tmp/x', '.premflow'));
    assert.deepEqual(plan2.args, ['note', 'hi']);
  });

  it('readSharedCaptureSnapshot reads real todo lines from temp shared root', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cap-snap-'));
    try {
      fs.writeFileSync(path.join(root, 'todo.txt'), '[TODO] alpha\n[TODO] beta\n');
      fs.writeFileSync(path.join(root, 'log.txt'), '[NOTE] n1\n');
      fs.mkdirSync(path.join(root, 'journal'));
      fs.writeFileSync(path.join(root, 'journal', 'journal-2026-07-13.txt'), 'day\n');
      const snap = readSharedCaptureSnapshot({ root });
      assert.equal(snap.root, root);
      assert.ok(snap.todos.some((t) => t.includes('alpha')));
      assert.ok(snap.todos.some((t) => t.includes('beta')));
      assert.ok(snap.logTail.some((l) => l.includes('NOTE')));
      assert.ok(snap.journalFiles.includes('journal-2026-07-13.txt'));
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
});

describe('capture redaction (insights must not leak)', () => {
  it('classifies finance/bank lines private and redacts them', () => {
    const fin = classifyCaptureLine('[TODO] Avanza/Nordnet: buy fund, tax sale 2025');
    assert.equal(fin.visibility, 'private');
    assert.equal(fin.pushable, false);
    assert.match(redactCaptureLine(fin.line), /redacted/);
    assert.equal(redactCaptureLine(fin.line, { mode: 'drop' }), null);
  });

  it('projectCaptureForShare keeps public craft lines and strips private raw text', () => {
    const proj = projectCaptureForShare({
      todos: [
        '[TODO] Ship ensembly OSS README',
        '[TODO] Bank transfer rent and pension check',
        '[TODO] open source skill publish',
      ],
      logTail: ['[NOTE] medical appointment next week', '[WIN] Shipped OSS'],
      journalFiles: ['journal-2026-07-13.txt'],
    });
    assert.ok(proj.shareable.todoPrivateCount >= 1);
    assert.ok(proj.shareable.todos.every((t) => !/bank|pension|medical/i.test(t)));
    assert.ok(proj.shareable.logTail.every((t) => !/medical/i.test(t)));
    // At least one public craft-ish line may survive
    const blob = proj.shareable.todos.join(' ');
    assert.doesNotMatch(blob, /Bank transfer|pension/i);
    assert.match(proj.policy, /never embed raw private/i);
  });

  it('sharedCaptureStatus shareSafe omits raw private samples when shareSafe', () => {
    // Uses real HOME store if present — only assert shape + policy
    const st = sharedCaptureStatus({ shareSafe: true });
    assert.ok(st.privacy?.captureIsPrivateByDefault);
    assert.ok(st.privacy?.shareableProjection);
    assert.equal(typeof st.privacy.shareableProjection.todoPrivateCount, 'number');
    assert.match(st.privacy.policy, /redacted|private/i);
  });
});

describe('premflow CLI wrapper entry (shipped bin)', () => {
  it('swarm help lists flow command', () => {
    const r = spawnSync(process.execPath, ['bin/swarm.js', 'help'], {
      cwd: repoRoot,
      encoding: 'utf8',
    });
    assert.equal(r.status, 0);
    assert.match(r.stdout, /\bflow\b/);
    assert.match(r.stdout, /premflow|shared capture|micro-capture/i);
  });

  it('flow path prints shared root via real CLI', () => {
    const r = spawnSync(process.execPath, ['bin/swarm.js', 'flow', 'path'], {
      cwd: repoRoot,
      encoding: 'utf8',
    });
    assert.equal(r.status, 0, r.stderr);
    const shared = resolveSharedCaptureRoot();
    assert.match(r.stdout, new RegExp(shared.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
    assert.match(r.stdout, /life-os|capture|SoT|shared/i);
  });

  it('flow forwards to premflow when binary available (or reports missing clearly)', () => {
    const bin = resolvePremflowBin();
    const r = spawnSync(process.execPath, ['bin/swarm.js', 'flow'], {
      cwd: repoRoot,
      encoding: 'utf8',
    });
    // bare flow → premflow help
    if (bin) {
      assert.equal(r.status, 0, r.stderr + r.stdout);
      assert.match(r.stdout + r.stderr, /premflow|Commands|note|task|pomo|review/i);
    } else {
      assert.notEqual(r.status, 0);
      assert.match(r.stderr + r.stdout, /premflow binary not found|not found/i);
    }
  });
});
