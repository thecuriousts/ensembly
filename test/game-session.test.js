import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  createSession,
  dispatch,
  sessionView,
  mapKeyEvent,
  parseVoiceCommand,
  helpLines,
  mapGamepadButton,
} from '../src/game/index.js';

function sampleGraph() {
  return {
    meta: { nodeCount: 3 },
    nodes: [
      { id: 'action-a', label: 'Ship swarm', type: 'action', position: { x: 0, y: 0 } },
      { id: 'action-b', label: 'Grocery errand', type: 'physical', position: { x: 220, y: 0 } },
      {
        id: 'hitl-apply',
        label: 'HITL Apply',
        type: 'hitl',
        hitl: true,
        position: { x: 440, y: 0 },
      },
    ],
    edges: [{ id: 'e1', source: 'action-a', target: 'hitl-apply', kind: 'requires_auth' }],
  };
}

const pendingSnap = {
  pending: [
    {
      id: 'auth-apply-high-signal',
      title: 'Apply packet',
      actionId: 'apply-high-signal',
      status: 'pending',
    },
    {
      id: 'auth-other',
      title: 'Other',
      status: 'pending',
    },
  ],
};

describe('game session + input (shipped)', () => {
  it('createSession focuses first node', () => {
    const s = createSession(sampleGraph(), pendingSnap);
    const v = sessionView(s);
    assert.equal(v.focusIndex, 0);
    assert.equal(v.selectedId, 'action-a');
    assert.equal(v.pendingOpen, 2);
    assert.equal(v.nodeCount, 3);
  });

  it('keyboard Tab / arrows change focus (real mapKeyEvent → dispatch)', () => {
    let s = createSession(sampleGraph(), pendingSnap);
    const next = mapKeyEvent({ key: 'Tab' });
    assert.equal(next.type, 'FOCUS_NEXT');
    s = dispatch(s, next);
    assert.equal(sessionView(s).focusIndex, 1);
    assert.equal(sessionView(s).focusLabel, 'Grocery errand');

    const right = mapKeyEvent({ key: 'ArrowRight' });
    s = dispatch(s, right);
    assert.ok(sessionView(s).focusIndex >= 0);
    assert.notEqual(sessionView(s).selectedId, null);

    const prev = mapKeyEvent({ key: 'Tab', shiftKey: true });
    assert.equal(prev.type, 'FOCUS_PREV');
    s = dispatch(s, prev);
    assert.equal(typeof sessionView(s).focusIndex, 'number');
  });

  it('approve/deny keys resolve pending and change snapshot counts', () => {
    let s = createSession(sampleGraph(), pendingSnap);
    assert.equal(sessionView(s).pendingOpen, 2);
    const approve = mapKeyEvent({ key: 'a' });
    assert.equal(approve.type, 'APPROVE');
    s = dispatch(s, approve);
    assert.equal(sessionView(s).pendingOpen, 1);
    assert.equal(sessionView(s).pendingResolved, 1);
    assert.match(sessionView(s).message, /approved/i);

    const deny = mapKeyEvent({ key: 'd' });
    s = dispatch(s, deny);
    assert.equal(sessionView(s).pendingOpen, 0);
    assert.equal(sessionView(s).pendingResolved, 2);
  });

  it('help toggle changes mode and helpOpen', () => {
    let s = createSession(sampleGraph());
    s = dispatch(s, mapKeyEvent({ key: '?' }));
    assert.equal(sessionView(s).helpOpen, true);
    assert.equal(sessionView(s).mode, 'help');
    s = dispatch(s, mapKeyEvent({ key: 'Escape' }));
    assert.equal(sessionView(s).helpOpen, false);
    assert.ok(helpLines().length >= 5);
  });

  it('voice parse maps approve/next to same action vocabulary', () => {
    const r = parseVoiceCommand('please approve the next item');
    assert.equal(r.matched, true);
    assert.ok(r.actions.some((a) => a.type === 'APPROVE'));
    assert.ok(r.actions.some((a) => a.type === 'FOCUS_NEXT'));
    assert.ok(r.actions.some((a) => a.type === 'VOICE_TEXT'));

    let s = createSession(sampleGraph(), pendingSnap);
    const beforePending = sessionView(s).pendingOpen;
    const beforeFocus = sessionView(s).focusIndex;
    for (const a of r.actions) s = dispatch(s, a);
    const after = sessionView(s);
    // FOCUS_NEXT and APPROVE both fire — pending and/or focus must change
    assert.ok(
      after.pendingOpen < beforePending || after.focusIndex !== beforeFocus,
      'voice actions must change session state',
    );
    assert.ok(after.tick >= 2);
  });

  it('gamepad button 0 approves', () => {
    const a = mapGamepadButton(0);
    assert.equal(a.type, 'APPROVE');
  });
});
