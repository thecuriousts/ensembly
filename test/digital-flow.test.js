import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {
  createDigitalFlow,
  activateDigitalFlow,
  applyFlowDecision,
  executeDigitalFlow,
  runDigitalFlowCycle,
  flowToApprovalRecord,
  flowToActionCandidate,
  flowToPlaceNode,
  mapFlowStatusToApprovalStatus,
  DIGITAL_FLOW_KINDS,
} from '../src/digital-flow.js';
import { upsertPendingFromActions, applyDecision, listPending } from '../src/approvals.js';
import { runDigitalFlowCommand } from '../src/turn.js';

describe('digital-flow IR (bill_pay / Bank HITL + dry-run)', () => {
  it('activate → pending → approve → dry-run execute (hook runs once)', () => {
    let hookCalls = 0;
    const cycle = runDigitalFlowCycle(
      {
        id: 'flow-bill_pay',
        kind: DIGITAL_FLOW_KINDS.BILL_PAY,
        payeeLabel: 'electric',
        amountLabel: 'synthetic',
      },
      'approve',
      {
        now: '2026-07-15T10:00:00.000Z',
        executionMode: 'dry_run',
        executeHook: (ctx) => {
          hookCalls += 1;
          assert.equal(ctx.kind, 'bill_pay');
          assert.equal(ctx.place, 'Bank');
          assert.equal(ctx.executionMode, 'dry_run');
          assert.equal(ctx.payeeLabel, 'electric');
          return { ok: true, mode: 'dry_run', message: 'dry-run wire ok', mutated: false };
        },
      },
    );
    assert.equal(cycle.flow.status, 'dry_run_ok');
    assert.equal(cycle.executed, true);
    assert.equal(hookCalls, 1);
    assert.equal(cycle.flow.lastResult.mutated, false);
    assert.equal(cycle.flow.place, 'Bank');
    assert.match(cycle.flow.lastResult.message || '', /dry-run/i);
  });

  it('deny path never invokes execute hook', () => {
    let hookCalls = 0;
    const cycle = runDigitalFlowCycle(
      { id: 'flow-bill_pay', kind: 'bill_pay', payeeLabel: 'rent' },
      'deny',
      {
        now: '2026-07-15T11:00:00.000Z',
        executeHook: () => {
          hookCalls += 1;
          return { ok: true, mutated: true };
        },
      },
    );
    assert.equal(cycle.flow.status, 'denied');
    assert.equal(cycle.executed, false);
    assert.equal(hookCalls, 0);
    assert.equal(cycle.hookCalls, 0);
  });

  it('cannot execute before approve', () => {
    let flow = createDigitalFlow({ id: 'flow-x', kind: 'bill_pay' });
    flow = activateDigitalFlow(flow, { now: '2026-07-15T12:00:00.000Z' });
    assert.equal(flow.status, 'pending_auth');
    assert.throws(() => executeDigitalFlow(flow), /approve first|cannot execute/i);
  });

  it('maps to approvals pending + finance_transfer action candidate', () => {
    let flow = createDigitalFlow({
      id: 'flow-bill_pay',
      kind: 'bill_pay',
      payeeLabel: 'water',
      now: '2026-07-15T12:00:00.000Z',
    });
    flow = activateDigitalFlow(flow);
    const approval = flowToApprovalRecord(flow);
    assert.equal(approval.status, 'pending');
    assert.equal(approval.kind, 'finance_transfer');
    assert.equal(approval.place, 'Bank');

    const action = flowToActionCandidate(flow);
    assert.equal(action.kind, 'finance_transfer');
    assert.equal(action.public, false);
    assert.equal(action.classification.hitl, true);
    assert.equal(action.classification.visibility, 'private');

    const place = flowToPlaceNode(flow);
    assert.equal(place.label, 'Bank');
    assert.equal(place.type, 'place');

    // Wire through real approvals IR (shipped functions)
    let snap = upsertPendingFromActions([action], null, { now: '2026-07-15T12:00:00.000Z' });
    assert.ok(listPending(snap).length >= 1);
    const id = listPending(snap)[0].id;
    snap = applyDecision(snap, id, 'approve', { actor: 'operator' });
    assert.equal(listPending(snap).length, 0);
  });

  it('step-wise activate / approve / execute matches cycle', () => {
    let flow = createDigitalFlow({
      id: 'flow-bill_pay',
      kind: 'bill_pay',
      payeeLabel: 'internet',
      now: '2026-07-15T13:00:00.000Z',
    });
    flow = activateDigitalFlow(flow, { now: '2026-07-15T13:00:01.000Z' });
    assert.equal(flow.status, 'pending_auth');
    flow = applyFlowDecision(flow, 'approve', { now: '2026-07-15T13:00:02.000Z' });
    assert.equal(flow.status, 'approved');
    flow = executeDigitalFlow(flow, {
      now: '2026-07-15T13:00:03.000Z',
      executeHook: (ctx) => ({
        ok: true,
        mode: ctx.executionMode,
        message: `dry-run: would pay ${ctx.payeeLabel} at ${ctx.place}`,
        mutated: false,
      }),
    });
    assert.equal(flow.status, 'dry_run_ok');
    assert.match(flow.lastResult.message, /internet/);
  });

  it('after approve→dry-run, approval record is closed (not pending)', () => {
    const cycle = runDigitalFlowCycle(
      { id: 'flow-bill_pay', kind: 'bill_pay', payeeLabel: 'electric' },
      'approve',
      { now: '2026-07-15T14:00:00.000Z', executionMode: 'dry_run' },
    );
    assert.equal(cycle.flow.status, 'dry_run_ok');
    // Skeptic bug: dry_run_ok must not map to pending
    assert.equal(mapFlowStatusToApprovalStatus('dry_run_ok'), 'approved');
    assert.equal(mapFlowStatusToApprovalStatus('executed'), 'approved');
    assert.equal(mapFlowStatusToApprovalStatus('pending_auth'), 'pending');
    const rec = flowToApprovalRecord(cycle.flow);
    assert.equal(rec.status, 'approved');
    assert.notEqual(rec.status, 'pending');
    assert.equal(cycle.approval.status, 'approved');
    // Merging into approvals listPending must exclude bill_pay
    let snap = upsertPendingFromActions([], null, { now: '2026-07-15T14:00:00.000Z' });
    snap.pending = [rec];
    assert.equal(listPending(snap).length, 0);
  });

  it('re-activate works after dry_run_ok so durable store is not one-shot', () => {
    let flow = createDigitalFlow({ id: 'flow-bill_pay', kind: 'bill_pay' });
    const first = runDigitalFlowCycle(flow, 'approve', {
      now: '2026-07-15T15:00:00.000Z',
      executionMode: 'dry_run',
    });
    assert.equal(first.flow.status, 'dry_run_ok');
    // Second cycle from terminal status — must not throw
    const second = runDigitalFlowCycle(first.flow, 'approve', {
      now: '2026-07-15T15:01:00.000Z',
      executionMode: 'dry_run',
    });
    assert.equal(second.flow.status, 'dry_run_ok');
    assert.equal(flowToApprovalRecord(second.flow).status, 'approved');
    // activate alone after dry_run_ok
    const again = activateDigitalFlow(second.flow, { now: '2026-07-15T15:02:00.000Z' });
    assert.equal(again.status, 'pending_auth');
    assert.equal(flowToApprovalRecord(again).status, 'pending');
  });

  it('runDigitalFlowCommand cycle closes bill_pay in wait-snapshot (shipped IO path)', () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'ensembly-df-'));
    const root = path.join(tmp, 'repo');
    // Minimal root so resolve paths work under opts.root
    fs.mkdirSync(path.join(root, 'private', 'state'), { recursive: true });
    const snapFile = path.join(root, 'private', 'state', 'wait-snapshot.json');
    const flowFile = path.join(root, 'private', 'state', 'digital-flows.json');

    const r1 = runDigitalFlowCommand('cycle', {
      root,
      snapshotFile: snapFile,
      flowFile,
      write: true,
      executionMode: 'dry_run',
      payeeLabel: 'water',
      now: '2026-07-15T16:00:00.000Z',
    });
    assert.equal(r1.flow.status, 'dry_run_ok');
    assert.equal(r1.approval.status, 'approved');
    const snap1 = JSON.parse(fs.readFileSync(snapFile, 'utf8'));
    const bill = (snap1.pending || []).find(
      (p) => p.id === 'auth-flow-bill_pay' || p.digitalFlowId === 'flow-bill_pay' || p.actionId === 'flow-bill_pay',
    );
    assert.ok(bill, 'bill_pay approval row should exist in snapshot');
    assert.equal(bill.status, 'approved');
    assert.notEqual(bill.status, 'pending');
    assert.equal(
      listPending(snap1).filter((p) => p.id === bill.id).length,
      0,
    );

    // Second cycle from durable store must succeed and stay closed
    const r2 = runDigitalFlowCommand('cycle', {
      root,
      snapshotFile: snapFile,
      flowFile,
      write: true,
      executionMode: 'dry_run',
      now: '2026-07-15T16:01:00.000Z',
    });
    assert.equal(r2.flow.status, 'dry_run_ok');
    const snap2 = JSON.parse(fs.readFileSync(snapFile, 'utf8'));
    const bill2 = (snap2.pending || []).find((p) => p.id === bill.id);
    assert.equal(bill2.status, 'approved');
  });
});
