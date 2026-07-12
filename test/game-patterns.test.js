import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  createGameStore,
  Observable,
  CommandBus,
  createSession,
  mapKeyEvent,
  parseVoiceCommand,
} from '../src/game/index.js';

const graph = {
  nodes: [
    { id: 'a', label: 'Alpha', type: 'action', position: { x: 0, y: 0 } },
    { id: 'b', label: 'Beta', type: 'physical', position: { x: 100, y: 0 } },
    { id: 'c', label: 'Gamma', type: 'hitl', hitl: true, position: { x: 200, y: 0 } },
  ],
  edges: [{ id: 'e1', source: 'a', target: 'b', kind: 'flow' }],
};

describe('Patterns.dev game patterns (shipped)', () => {
  it('Observable notifies subscribers (observer pattern)', () => {
    const o = new Observable();
    const seen = [];
    const unsub = o.subscribe((d) => seen.push(d));
    o.notify({ x: 1 });
    o.notify({ x: 2 });
    unsub();
    o.notify({ x: 3 });
    assert.deepEqual(seen, [{ x: 1 }, { x: 2 }]);
    assert.equal(o.size, 0);
  });

  it('createGameStore dispatches and notifies observers', () => {
    const store = createGameStore(graph, {
      pending: [{ id: 'auth-x', title: 'X', status: 'pending' }],
    });
    const events = [];
    store.subscribe((e) => events.push(e.reason));
    store.dispatch({ type: 'FOCUS_NEXT' });
    store.dispatch({ type: 'APPROVE' });
    assert.ok(events.includes('FOCUS_NEXT'));
    assert.ok(events.includes('APPROVE'));
    assert.equal(store.view.pendingOpen, 0);
    assert.equal(store.view.focusIndex, 1);
  });

  it('CommandBus undo reverses nav (command pattern)', () => {
    const session = createSession(graph);
    const bus = new CommandBus(session);
    bus.execute({ type: 'FOCUS_NEXT' });
    assert.equal(bus.session.focusIndex, 1);
    bus.undo();
    assert.equal(bus.session.focusIndex, 0);
    bus.redo();
    assert.equal(bus.session.focusIndex, 1);
  });

  it('mapKeyEvent exposes undo/redo bindings', () => {
    assert.equal(mapKeyEvent({ key: 'u' }).type, 'UNDO');
    assert.equal(mapKeyEvent({ key: 'r' }).type, 'REDO');
  });

  it('FOCUS_INDEX jumps without thrashing next-loop', () => {
    const store = createGameStore(graph);
    store.dispatch({ type: 'FOCUS_INDEX', payload: { index: 2 } });
    assert.equal(store.view.focusIndex, 2);
    assert.equal(store.view.selectedId, 'c');
  });

  it('voice still maps to command vocabulary', () => {
    const r = parseVoiceCommand('undo then approve');
    // approve matches; undo not in voice rules yet — approve must
    assert.ok(r.actions.some((a) => a.type === 'APPROVE'));
  });
});
