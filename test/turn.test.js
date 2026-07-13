import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  buildTurnSurface,
  buildTurnStatus,
  formatTurnMarkdown,
  selectNextPhysical,
  selectNextAuth,
  runOperatorTurn,
  runApprovalDecision,
  runPhysicalDecision,
} from '../src/turn.js';
import { emptySnapshot, applyPhysicalDecision } from '../src/approvals.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const scratchSnapDir = path.join(root, 'private', 'state');

describe('turn surface (shipped entry path)', () => {
  it('buildTurnSurface lists physical and pending', () => {
    const turn = buildTurnSurface({
      date: '2026-07-13',
      actions: [
        {
          id: 'grocery-errand',
          title: 'Grocery errand',
          realm: 'physical',
          area: 'Relationships',
          classification: { hitl: false },
        },
        {
          id: 'apply-high-signal',
          title: 'Apply',
          kind: 'job_application_submit',
          realm: 'digital',
          classification: { hitl: true, reason: 'HITL' },
        },
      ],
      snapshot: emptySnapshot(),
    });
    assert.ok(turn.physicalPickups.length >= 1);
    assert.ok(turn.pendingAuthorizations.length >= 1);
    assert.equal(turn.summary.snapshotStatus, 'idle_waiting');
    assert.ok(turn.nextPhysical);
    assert.equal(turn.nextPhysical.id, 'grocery-errand');
    assert.ok(turn.nextAuth);
    assert.equal(turn.nextAuth.id, 'auth-apply-high-signal');
    const md = formatTurnMarkdown(turn);
    assert.match(md, /Next action \(primary\)/i);
    assert.match(md, /Physical world pickups/i);
    assert.match(md, /Pending authorizations/i);
    assert.match(md, /Grocery/);
    assert.match(md, /swarm\.js claim grocery-errand/);
    assert.match(md, /swarm\.js approve auth-apply-high-signal/);
  });

  it('selectNextPhysical prefers current schedule window over priority', () => {
    const pickups = [
      { id: 'grocery-errand', title: 'Grocery', urgency: 5, importance: 5 },
      { id: 'evening-outdoor', title: 'Walk', urgency: 2, importance: 2 },
    ];
    const schedule = [
      {
        start: '17:00',
        end: '18:00',
        label: 'Evening outdoor',
        assigned: { id: 'evening-outdoor', title: 'Walk' },
      },
    ];
    // 17:30 → evening window is current
    const next = selectNextPhysical(pickups, schedule, {
      now: new Date('2026-07-13T17:30:00'),
    });
    assert.equal(next.id, 'evening-outdoor');
    assert.equal(next.reason, 'current_window');
    assert.equal(next.scheduleWindow.start, '17:00');
    assert.ok(next.commands.claim.includes('evening-outdoor'));
  });

  it('selectNextPhysical falls back to priority when no schedule match', () => {
    const pickups = [
      { id: 'low', title: 'Low', urgency: 1, importance: 1 },
      { id: 'high', title: 'High', urgency: 4, importance: 4 },
    ];
    const next = selectNextPhysical(pickups, [], { now: '2026-07-13T10:00:00' });
    assert.equal(next.id, 'high');
    assert.equal(next.reason, 'priority');
  });

  it('selectNextAuth returns oldest pending with approve/deny commands', () => {
    const next = selectNextAuth([
      {
        id: 'auth-b',
        title: 'B',
        kind: 'hitl',
        status: 'pending',
        createdAt: '2026-07-13T14:00:00.000Z',
      },
      {
        id: 'auth-a',
        title: 'A',
        kind: 'job_application_submit',
        status: 'pending',
        createdAt: '2026-07-13T12:00:00.000Z',
      },
    ]);
    assert.equal(next.id, 'auth-a');
    assert.match(next.commands.approve, /approve auth-a/);
    assert.match(next.commands.deny, /deny auth-a/);
  });

  it('buildTurnStatus exposes next physical and pending queues', () => {
    const turn = buildTurnSurface({
      date: '2026-07-13',
      actions: [
        {
          id: 'grocery-errand',
          title: 'Grocery errand',
          realm: 'physical',
          area: 'Relationships',
        },
        {
          id: 'apply-high-signal',
          title: 'Prepare high-signal FT application packet (public CV path)',
          kind: 'job_application_submit',
          realm: 'digital',
          classification: { hitl: true },
        },
      ],
      snapshot: emptySnapshot({ now: '2026-07-13T12:00:00.000Z' }),
      now: '2026-07-13T12:00:00.000Z',
    });
    const status = buildTurnStatus(turn);
    assert.equal(status.version, 1);
    assert.ok(status.next.physical);
    assert.equal(status.next.physical.id, 'grocery-errand');
    assert.ok(status.next.authorization);
    assert.equal(status.next.authorization.id, 'auth-apply-high-signal');
    assert.ok(status.physical.some((p) => p.id === 'grocery-errand'));
    assert.ok(status.pending.some((p) => p.id === 'auth-apply-high-signal'));
    assert.ok(status.counts.physical >= 1);
    assert.ok(status.counts.pending >= 1);
  });

  it('runOperatorTurn with fixture yields physical + pending + next primaries', () => {
    fs.mkdirSync(scratchSnapDir, { recursive: true });
    const tmpSnap = path.join(scratchSnapDir, `test-snap-${Date.now()}.json`);
    const turn = runOperatorTurn({
      root,
      fixture: path.join(root, 'fixtures', 'state-sample.json'),
      snapshotFile: tmpSnap,
      write: true,
    });
    assert.ok(turn.summary.physicalCount >= 1, 'expected physical pickups from fixture');
    assert.ok(turn.summary.pendingCount >= 1, 'expected pending authorizations');
    assert.ok(turn.nextPhysical, 'expected primary next physical');
    assert.ok(turn.nextAuth, 'expected primary next auth');
    assert.equal(turn.nextAuth.id, 'auth-apply-high-signal');
    // Next physical is schedule-aware (rhythm blocks may win over errand candidates)
    assert.ok(turn.nextPhysical.id, 'next physical must have stable id');
    assert.ok(
      turn.physicalPickups.some((p) => p.id === turn.nextPhysical.id),
      `next physical ${turn.nextPhysical.id} must be in open pickup queue`,
    );
    assert.ok(
      turn.nextPhysical.commands?.claim?.includes(turn.nextPhysical.id),
      'claim command must target next physical id',
    );
    assert.match(turn.markdown, /Next action \(primary\)/i);
    assert.match(turn.markdown, /Physical world pickups/i);
    assert.ok(fs.existsSync(tmpSnap));
    assert.ok(turn.statusPath && fs.existsSync(turn.statusPath));
    const status = JSON.parse(fs.readFileSync(turn.statusPath, 'utf8'));
    assert.equal(status.next.authorization.id, 'auth-apply-high-signal');
    assert.ok(status.next.physical?.id);
    fs.unlinkSync(tmpSnap);
    if (fs.existsSync(turn.statusPath)) fs.unlinkSync(turn.statusPath);
  });

  it('approve then deny advances wait snapshot on real path', () => {
    const tmpSnap = path.join(
      root,
      'private',
      'state',
      `test-wait-${Date.now()}.json`,
    );
    const turn = runOperatorTurn({
      root,
      fixture: path.join(root, 'fixtures', 'state-sample.json'),
      snapshotFile: tmpSnap,
      write: true,
    });
    const pending = turn.pendingAuthorizations.map((p) => p.id);
    assert.ok(pending.length >= 1);
    const first = pending[0];
    const approved = runApprovalDecision('approve', first, {
      root,
      snapshotFile: tmpSnap,
      write: true,
    });
    assert.ok(['partial', 'clear'].includes(approved.status));
    assert.ok(!approved.pendingRemaining.find((p) => p.id === first));

    if (approved.pendingRemaining.length > 0) {
      const second = approved.pendingRemaining[0].id;
      const denied = runApprovalDecision('deny', second, {
        root,
        snapshotFile: tmpSnap,
        write: true,
      });
      assert.ok(!denied.pendingRemaining.find((p) => p.id === second));
      assert.ok(['clear', 'partial'].includes(denied.status));
    }
    fs.unlinkSync(tmpSnap);
    const statusFile = path.join(path.dirname(tmpSnap), 'turn-status.json');
    if (fs.existsSync(statusFile)) fs.unlinkSync(statusFile);
  });

  it('claim then complete removes physical from open queue on real path', () => {
    const tmpSnap = path.join(scratchSnapDir, `test-phys-${Date.now()}.json`);
    const turn = runOperatorTurn({
      root,
      fixture: path.join(root, 'fixtures', 'state-sample.json'),
      snapshotFile: tmpSnap,
      write: true,
    });
    const physId = turn.nextPhysical?.id || turn.physicalPickups[0]?.id;
    assert.ok(physId, 'fixture must expose a physical pickup');

    const claimed = runPhysicalDecision('claim', physId, {
      root,
      snapshotFile: tmpSnap,
      write: true,
    });
    const claimedRow = claimed.snapshot.physical.find((p) => p.id === physId);
    assert.equal(claimedRow.status, 'claimed');
    assert.ok(
      (claimed.openPhysical || []).some((p) => p.id === physId),
      'claimed remains visible until complete',
    );

    const done = runPhysicalDecision('complete', physId, {
      root,
      snapshotFile: tmpSnap,
      write: true,
    });
    assert.ok(
      !(done.openPhysical || []).some((p) => p.id === physId),
      'completed physical must leave open queue',
    );
    assert.equal(
      done.snapshot.physical.find((p) => p.id === physId).status,
      'completed',
    );
    // Pure filter: completed excluded from turn pickups
    const after = runOperatorTurn({
      root,
      fixture: path.join(root, 'fixtures', 'state-sample.json'),
      snapshotFile: tmpSnap,
      write: false,
    });
    assert.ok(!after.physicalPickups.some((p) => p.id === physId));
    if (after.nextPhysical) {
      assert.notEqual(after.nextPhysical.id, physId);
    }

    fs.unlinkSync(tmpSnap);
    const statusFile = path.join(path.dirname(tmpSnap), 'turn-status.json');
    if (fs.existsSync(statusFile)) fs.unlinkSync(statusFile);
  });

  it('applyPhysicalDecision pure claim/complete/release', () => {
    let snap = emptySnapshot({ now: '2026-07-13T12:00:00.000Z' });
    snap = applyPhysicalDecision(snap, 'grocery-errand', 'claim', {
      now: '2026-07-13T12:01:00.000Z',
    });
    assert.equal(snap.physical[0].status, 'claimed');
    snap = applyPhysicalDecision(snap, 'grocery-errand', 'complete', {
      now: '2026-07-13T12:02:00.000Z',
    });
    assert.equal(snap.physical[0].status, 'completed');
  });

  it('complete removes id from next physical and refreshes public watch HTML', () => {
    const tmpSnap = path.join(scratchSnapDir, `test-watch-sync-${Date.now()}.json`);
    // Isolate watch dir under private so we do not thrash committed public/watch mid-test
    // — syncPublicWatch always writes public/watch; assert that path after complete.
    const turn = runOperatorTurn({
      root,
      fixture: path.join(root, 'fixtures', 'state-sample.json'),
      snapshotFile: tmpSnap,
      write: true,
    });
    const physId = turn.nextPhysical?.id || turn.physicalPickups[0]?.id;
    assert.ok(physId);

    const done = runPhysicalDecision('complete', physId, {
      root,
      fixture: path.join(root, 'fixtures', 'state-sample.json'),
      snapshotFile: tmpSnap,
      write: true,
    });
    assert.equal(done.physicalStatus, 'completed');
    assert.equal(done.leftOpenQueue, true);
    assert.ok(done.next);
    if (done.next.physical) {
      assert.notEqual(done.next.physical.id, physId, 'next body must not be the completed id');
    }
    assert.ok(done.watchPaths?.htmlPath, 'complete must refresh public watch HTML');
    const html = fs.readFileSync(done.watchPaths.htmlPath, 'utf8');
    // Completed id must not be offered as primary next body commands
    assert.doesNotMatch(
      html,
      new RegExp(`Body — next physical[\\s\\S]*swarm\\.js claim ${physId}`),
    );
    const statusPub = JSON.parse(
      fs.readFileSync(path.join(root, 'public', 'watch', 'turn-status.json'), 'utf8'),
    );
    if (statusPub.next?.physical) {
      assert.notEqual(statusPub.next.physical.id, physId);
    }
    assert.ok(!statusPub.physical.some((p) => p.id === physId));

    fs.unlinkSync(tmpSnap);
    const statusFile = path.join(path.dirname(tmpSnap), 'turn-status.json');
    if (fs.existsSync(statusFile)) fs.unlinkSync(statusFile);
  });
});
