import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {
  createSwarmPropose,
  swarmProposeToActionCandidate,
  swarmProposeToApprovalRecord,
  SWARM_PROPOSE_KIND,
} from '../src/swarm-propose.js';
import { listPending, applyDecision } from '../src/approvals.js';
import { runSwarmProposeCommand, runApprovalDecision } from '../src/turn.js';

describe('swarm propose gate (Grok Bot → hub inbox)', () => {
  it('creates swarm_emit IR with stable approval id', () => {
    const propose = createSwarmPropose({
      title: 'Publish kingsparrow explainer',
      summary: 'static page drafted',
      source: 'grok-build',
      now: '2026-09-01T16:00:00.000Z',
      id: 'propose-kingsparrow-160000',
    });
    assert.equal(propose.kind, SWARM_PROPOSE_KIND);
    assert.equal(propose.source, 'grok-build');
    const action = swarmProposeToActionCandidate(propose);
    assert.equal(action.classification.hitl, true);
    const approval = swarmProposeToApprovalRecord(propose);
    assert.equal(approval.id, 'auth-propose-kingsparrow-160000');
    assert.equal(approval.status, 'pending');
    assert.match(approval.reason, /grok-build/);
    assert.equal(action.id, propose.id);
  });

  it('propose lands pending in wait-snapshot; approve closes gate', () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'ensembly-propose-'));
    const root = path.join(tmp, 'repo');
    fs.mkdirSync(path.join(root, 'private', 'state'), { recursive: true });
    const snapFile = path.join(root, 'private', 'state', 'wait-snapshot.json');

    const proposeResult = runSwarmProposeCommand({
      root,
      snapshotFile: snapFile,
      title: 'Ship kingsparrow explainer',
      summary: 'index.html ready for Vercel',
      source: 'grok-bot',
      now: '2026-09-01T16:05:00.000Z',
      write: true,
    });
    assert.ok(fs.existsSync(snapFile));
    const pending = listPending(proposeResult.snapshot);
    assert.equal(pending.length, 1);
    assert.equal(pending[0].kind, SWARM_PROPOSE_KIND);

    const authId = proposeResult.approval.id;
    const closed = applyDecision(proposeResult.snapshot, authId, 'approve', {
      actor: 'operator',
      now: '2026-09-01T16:10:00.000Z',
    });
    assert.equal(listPending(closed).length, 0);
    const row = (closed.pending || []).find((entry) => entry.id === authId);
    assert.equal(row.status, 'approved');
  });

  it('runApprovalDecision persists deny on shipped IO path', () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'ensembly-propose-io-'));
    const root = path.join(tmp, 'repo');
    const ensemblyRoot = path.join(path.dirname(new URL(import.meta.url).pathname), '..');
    fs.mkdirSync(path.join(root, 'private', 'state'), { recursive: true });
    fs.mkdirSync(path.join(root, 'public', 'persona'), { recursive: true });
    fs.mkdirSync(path.join(root, 'fixtures'), { recursive: true });
    fs.copyFileSync(
      path.join(ensemblyRoot, 'public', 'persona', 'projection.json'),
      path.join(root, 'public', 'persona', 'projection.json'),
    );
    fs.writeFileSync(
      path.join(root, 'private', 'state', 'current.json'),
      `${JSON.stringify({ date: '2026-09-01', actions: [] }, null, 2)}\n`,
      'utf8',
    );
    const snapFile = path.join(root, 'private', 'state', 'wait-snapshot.json');

    const proposeResult = runSwarmProposeCommand({
      root,
      snapshotFile: snapFile,
      title: 'Deny me',
      source: 'grok-bot',
      now: '2026-09-01T17:00:00.000Z',
      write: true,
    });

    const denied = runApprovalDecision('deny', proposeResult.approval.id, {
      root,
      snapshotFile: snapFile,
      write: true,
    });
    assert.equal(denied.decision, 'deny');
    const snap = JSON.parse(fs.readFileSync(snapFile, 'utf8'));
    const row = (snap.pending || []).find((entry) => entry.id === proposeResult.approval.id);
    assert.equal(row.status, 'denied');
    assert.equal(
      listPending(snap).some((entry) => entry.id === proposeResult.approval.id),
      false,
    );
  });
});
