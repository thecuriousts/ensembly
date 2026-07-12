import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildTurnSurface, formatTurnMarkdown, runOperatorTurn, runApprovalDecision } from '../src/turn.js';
import { emptySnapshot } from '../src/approvals.js';

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
    const md = formatTurnMarkdown(turn);
    assert.match(md, /Physical world pickups/i);
    assert.match(md, /Pending authorizations/i);
    assert.match(md, /Grocery/);
  });

  it('runOperatorTurn with fixture yields physical + pending', () => {
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
    assert.match(turn.markdown, /Physical world pickups/i);
    assert.ok(fs.existsSync(tmpSnap));
    fs.unlinkSync(tmpSnap);
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
  });
});
