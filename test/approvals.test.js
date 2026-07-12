import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  emptySnapshot,
  upsertPendingFromActions,
  applyDecision,
  listPending,
  serializeSnapshot,
  parseSnapshot,
  deriveStatus,
} from '../src/approvals.js';

describe('approvals (shipped durable snapshot)', () => {
  it('upsertPendingFromActions creates pending for HITL actions', () => {
    const snap = upsertPendingFromActions(
      [
        {
          id: 'apply-high-signal',
          title: 'Apply',
          kind: 'job_application_submit',
          classification: { hitl: true, reason: 'HITL' },
        },
        { id: 'oss', title: 'Ship OSS', classification: { hitl: false } },
      ],
      emptySnapshot(),
      { now: '2026-07-13T12:00:00.000Z' },
    );
    assert.equal(snap.status, 'idle_waiting');
    assert.equal(snap.phase, 'HITL_WAIT');
    assert.equal(listPending(snap).length, 1);
    assert.equal(listPending(snap)[0].id, 'auth-apply-high-signal');
  });

  it('applyDecision approve advances status', () => {
    let snap = upsertPendingFromActions(
      [
        {
          id: 'a1',
          title: 'Send email',
          kind: 'external_email_send',
          classification: { hitl: true },
        },
        {
          id: 'a2',
          title: 'Transfer',
          kind: 'finance_transfer',
          classification: { hitl: true },
        },
      ],
      null,
      { now: '2026-07-13T12:00:00.000Z' },
    );
    assert.equal(listPending(snap).length, 2);
    snap = applyDecision(snap, 'auth-a1', 'approve', { now: '2026-07-13T12:01:00.000Z' });
    assert.equal(listPending(snap).length, 1);
    assert.equal(snap.status, 'partial');
    const approved = snap.pending.find((p) => p.id === 'auth-a1');
    assert.equal(approved.status, 'approved');
    snap = applyDecision(snap, 'auth-a2', 'deny', { now: '2026-07-13T12:02:00.000Z' });
    assert.equal(listPending(snap).length, 0);
    assert.equal(snap.status, 'clear');
    assert.equal(snap.phase, 'CLEAR');
    assert.ok(snap.history.length >= 2);
  });

  it('serialize/parse round-trips', () => {
    const snap = emptySnapshot({ now: '2026-07-13T00:00:00.000Z' });
    const again = parseSnapshot(serializeSnapshot(snap));
    assert.equal(again.version, 1);
    assert.equal(again.status, 'clear');
  });

  it('deriveStatus maps pending sets', () => {
    assert.equal(deriveStatus([]), 'clear');
    assert.equal(deriveStatus([{ status: 'pending' }]), 'idle_waiting');
    assert.equal(
      deriveStatus([{ status: 'pending' }, { status: 'approved' }]),
      'partial',
    );
  });
});
