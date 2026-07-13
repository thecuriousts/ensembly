/**
 * Activity/log store — drives shipped append + list on memory and SQLite round-trip.
 */
import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import {
  ACTIVITY_IR_VERSION,
  normalizeActivityEntry,
  filterActivityEntries,
  createMemoryActivityStore,
  openSqliteActivityStore,
  openActivityStore,
  resolveActivityDbPath,
  DEFAULT_ACTIVITY_DB_REL,
} from '../src/activity/index.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

describe('activity IR (pure)', () => {
  it('normalizeActivityEntry assigns version, id, ts, payload', () => {
    const e = normalizeActivityEntry(
      { kind: 'activity.claim', actor: 'operator', payload: { target: 'grocery' } },
      { now: '2026-07-13T07:00:00.000Z', id: 'act_test_1' },
    );
    assert.equal(e.version, ACTIVITY_IR_VERSION);
    assert.equal(e.id, 'act_test_1');
    assert.equal(e.kind, 'activity.claim');
    assert.equal(e.actor, 'operator');
    assert.equal(e.payload.target, 'grocery');
    assert.equal(e.ts, '2026-07-13T07:00:00.000Z');
  });

  it('filterActivityEntries filters by kind and time order', () => {
    const rows = [
      normalizeActivityEntry({
        id: 'b',
        kind: 'log.info',
        ts: '2026-07-13T08:00:00.000Z',
        payload: { m: 2 },
      }),
      normalizeActivityEntry({
        id: 'a',
        kind: 'activity.claim',
        ts: '2026-07-13T07:00:00.000Z',
        payload: { m: 1 },
      }),
      normalizeActivityEntry({
        id: 'c',
        kind: 'log.info',
        ts: '2026-07-13T09:00:00.000Z',
        payload: { m: 3 },
      }),
    ];
    const claims = filterActivityEntries(rows, { kind: 'activity.claim' });
    assert.equal(claims.length, 1);
    assert.equal(claims[0].id, 'a');
    const ordered = filterActivityEntries(rows, {});
    assert.deepEqual(
      ordered.map((r) => r.id),
      ['a', 'b', 'c'],
    );
    const windowed = filterActivityEntries(rows, {
      since: '2026-07-13T07:30:00.000Z',
      until: '2026-07-13T08:30:00.000Z',
    });
    assert.equal(windowed.length, 1);
    assert.equal(windowed[0].id, 'b');
  });
});

describe('memory activity store (shipped path)', () => {
  it('append + list returns same entries ordered by time with kind filter', () => {
    const store = createMemoryActivityStore();
    const a = store.append(
      {
        kind: 'activity.claim',
        actor: 'operator',
        payload: { id: 'healthy-self-energy' },
      },
      { now: '2026-07-13T10:00:00.000Z', id: 'mem_claim_1' },
    );
    const b = store.append(
      {
        kind: 'log.info',
        actor: 'swarm',
        payload: { message: 'turn ran' },
      },
      { now: '2026-07-13T10:01:00.000Z', id: 'mem_log_1' },
    );
    assert.equal(a.kind, 'activity.claim');
    assert.equal(b.kind, 'log.info');

    const all = store.list();
    assert.equal(all.length, 2);
    assert.equal(all[0].id, 'mem_claim_1');
    assert.equal(all[1].id, 'mem_log_1');
    assert.equal(all[0].payload.id, 'healthy-self-energy');
    assert.equal(all[1].payload.message, 'turn ran');

    const logs = store.list({ kind: 'log.info' });
    assert.equal(logs.length, 1);
    assert.equal(logs[0].id, 'mem_log_1');
    assert.equal(logs[0].payload.message, 'turn ran');

    const byActor = store.list({ actor: 'operator' });
    assert.equal(byActor.length, 1);
    assert.equal(byActor[0].id, 'mem_claim_1');

    store.close();
  });

  it('openActivityStore({ backend: memory }) works', async () => {
    const store = await openActivityStore({ backend: 'memory' });
    store.append({ kind: 'log.warn', payload: { x: 1 } }, { id: 'm1', now: '2026-07-13T00:00:00.000Z' });
    assert.equal(store.list().length, 1);
    store.close();
  });
});

describe('sqlite activity store (round-trip reopen)', () => {
  const tmpDir = path.join(root, 'data', 'local', '.test-activity');
  const dbFile = path.join(tmpDir, `roundtrip-${process.pid}.sqlite`);

  before(() => {
    fs.mkdirSync(tmpDir, { recursive: true });
    for (const f of fs.readdirSync(tmpDir)) {
      if (f.startsWith('roundtrip-')) fs.rmSync(path.join(tmpDir, f), { force: true });
    }
  });

  after(() => {
    try {
      for (const f of fs.readdirSync(tmpDir)) {
        if (f.includes(String(process.pid))) fs.rmSync(path.join(tmpDir, f), { force: true });
      }
    } catch {
      /* ignore */
    }
  });

  it('append N, close, reopen same path, list same fields', async () => {
    const store1 = await openSqliteActivityStore({ filePath: dbFile });
    assert.equal(store1.backend, 'sqlite');
    assert.equal(store1.path, dbFile);

    const e1 = store1.append(
      {
        kind: 'activity.approve',
        actor: 'operator',
        correlationId: 'turn-1',
        payload: { authId: 'auth-apply-high-signal' },
      },
      { now: '2026-07-13T11:00:00.000Z', id: 'sql_approve_1' },
    );
    const e2 = store1.append(
      {
        kind: 'log.info',
        actor: 'swarm',
        payload: { message: 'approved gate' },
      },
      { now: '2026-07-13T11:00:01.000Z', id: 'sql_log_1' },
    );
    assert.equal(store1.size(), 2);
    store1.close();

    const store2 = await openSqliteActivityStore({ filePath: dbFile });
    const all = store2.list();
    assert.equal(all.length, 2, 'round-trip must restore both rows');
    assert.equal(all[0].id, e1.id);
    assert.equal(all[0].kind, 'activity.approve');
    assert.equal(all[0].payload.authId, 'auth-apply-high-signal');
    assert.equal(all[0].correlationId, 'turn-1');
    assert.equal(all[1].id, e2.id);
    assert.equal(all[1].kind, 'log.info');
    assert.equal(all[1].payload.message, 'approved gate');

    const onlyLog = store2.list({ kind: 'log.info' });
    assert.equal(onlyLog.length, 1);
    assert.equal(onlyLog[0].id, 'sql_log_1');
    store2.close();
  });
});

describe('activity store path privacy', () => {
  it('default path resolves under data/local and is gitignored', () => {
    const p = resolveActivityDbPath({ root });
    assert.ok(p.includes(path.join('data', 'local')));
    assert.ok(p.endsWith('activity.sqlite') || p.includes(DEFAULT_ACTIVITY_DB_REL.replace(/\\/g, path.sep)));
    assert.match(DEFAULT_ACTIVITY_DB_REL, /^data[/\\]local[/\\]activity\.sqlite$/);

    const check = spawnSync('git', ['check-ignore', '-v', 'data/local/activity.sqlite'], {
      cwd: root,
      encoding: 'utf8',
    });
    // exit 0 = ignored
    assert.equal(check.status, 0, `expected gitignored: ${check.stdout} ${check.stderr}`);
    assert.match(check.stdout + check.stderr, /data/);
  });
});
