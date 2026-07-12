import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  createSession,
  dispatch,
  sessionView,
  nodeRole,
  isCompletable,
  levelFromXp,
  buildQuests,
  growthView,
  growthCoachLine,
  XP_TABLE,
  mapKeyEvent,
  parseVoiceCommand,
} from '../src/game/index.js';

function growthGraph() {
  return {
    nodes: [
      { id: 'game-root', type: 'game', label: 'Root', kind: 'root' },
      { id: 'phys', type: 'physical', realm: 'physical', label: 'Grocery' },
      { id: 'craft', type: 'action', realm: 'digital', label: 'Ship OSS', area: 'craft' },
      { id: 'family', type: 'schedule', label: 'Family outdoor', area: 'family' },
      { id: 'hitl-x', type: 'hitl', hitl: true, label: 'HITL Apply' },
    ],
    pending: [{ id: 'auth-apply-high-signal', title: 'Apply', status: 'pending' }],
  };
}

describe('growth engine (shipped)', () => {
  it('classifies roles for life axes', () => {
    assert.equal(nodeRole({ type: 'physical' }), 'physical');
    assert.equal(nodeRole({ type: 'schedule', area: 'family' }), 'presence');
    assert.equal(nodeRole({ type: 'action', realm: 'digital' }), 'craft');
    assert.equal(nodeRole({ type: 'hitl', hitl: true }), 'hitl');
    assert.equal(nodeRole({ type: 'game', kind: 'root' }), 'meta');
    assert.equal(isCompletable({ type: 'physical' }), true);
    assert.equal(isCompletable({ type: 'hitl' }), false);
  });

  it('levels and titles ascend with XP', () => {
    assert.equal(levelFromXp(0).level, 1);
    assert.equal(levelFromXp(0).title, 'Ember');
    assert.equal(levelFromXp(100).level, 2);
    assert.ok(levelFromXp(250).level >= 3);
  });

  it('claim physical awards XP and fills balance', () => {
    let s = createSession(growthGraph());
    // focus grocery (index 1)
    s = dispatch(s, { type: 'FOCUS_INDEX', payload: { index: 1 } });
    s = dispatch(s, { type: 'COMPLETE' });
    const v = sessionView(s);
    assert.ok(v.growth.xp >= XP_TABLE.physical);
    assert.equal(v.growth.balance.physical, 1);
    assert.equal(v.growth.questsDone >= 1, true);
    assert.match(v.message, /\+\d+ XP/);
  });

  it('Enter claims completable beacon (SELECT → complete)', () => {
    let s = createSession(growthGraph());
    s = dispatch(s, { type: 'FOCUS_INDEX', payload: { index: 2 } });
    const enter = mapKeyEvent({ key: 'Enter' });
    assert.equal(enter.type, 'SELECT');
    s = dispatch(s, enter);
    assert.ok(sessionView(s).growth.xp >= XP_TABLE.craft);
    assert.equal(sessionView(s).growth.balance.craft, 1);
  });

  it('approve HITL awards XP and clears pending', () => {
    let s = createSession(growthGraph());
    assert.equal(sessionView(s).pendingOpen, 1);
    s = dispatch(s, { type: 'APPROVE' });
    const v = sessionView(s);
    assert.equal(v.pendingOpen, 0);
    assert.ok(v.growth.xp >= XP_TABLE.hitl_approve);
    assert.equal(v.growth.balance.hitl, 1);
  });

  it('streak adds combo bonus on second claim', () => {
    let s = createSession(growthGraph());
    s = dispatch(s, { type: 'FOCUS_INDEX', payload: { index: 1 } });
    s = dispatch(s, { type: 'COMPLETE' });
    const xp1 = sessionView(s).growth.xp;
    s = dispatch(s, { type: 'FOCUS_INDEX', payload: { index: 2 } });
    s = dispatch(s, { type: 'COMPLETE' });
    const v = sessionView(s);
    assert.ok(v.growth.streak >= 2);
    assert.ok(v.growth.xp > xp1 + XP_TABLE.craft); // combo on top
  });

  it('buildQuests lists open life work', () => {
    const s = createSession(growthGraph());
    const q = buildQuests(s);
    assert.ok(q.some((x) => x.role === 'physical'));
    assert.ok(q.some((x) => x.role === 'craft'));
    assert.ok(q.some((x) => x.role === 'presence'));
    assert.ok(q.some((x) => x.role === 'hitl'));
  });

  it('growthCoachLine steers toward physical when missing', () => {
    const s = createSession(growthGraph());
    const line = growthCoachLine(growthView(s));
    assert.match(line, /physical|Next:|Growth tip/i);
  });

  it('voice claim maps to COMPLETE', () => {
    const r = parseVoiceCommand('claim this beacon');
    assert.ok(r.actions.some((a) => a.type === 'COMPLETE'));
  });

  it('double claim is idempotent', () => {
    let s = createSession(growthGraph());
    s = dispatch(s, { type: 'FOCUS_INDEX', payload: { index: 1 } });
    s = dispatch(s, { type: 'COMPLETE' });
    const xp = sessionView(s).growth.xp;
    s = dispatch(s, { type: 'COMPLETE' });
    assert.equal(sessionView(s).growth.xp, xp);
    assert.match(sessionView(s).message, /Already claimed/);
  });
});
