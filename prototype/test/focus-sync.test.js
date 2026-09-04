import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  createGameStore,
  syncWorldFocusFromSession,
  applyFocusMirror,
  applyFocusDoubleAdvance,
  mapKeyEvent,
} from '../src/game/index.js';

describe('focus sync SoT (shipped)', () => {
  it('syncWorldFocusFromSession sets wasm to session index only', () => {
    const calls = [];
    const ok = syncWorldFocusFromSession({ focusIndex: 4 }, (s) => calls.push(s));
    assert.equal(ok, true);
    assert.deepEqual(calls, [4]);
  });

  it('FOCUS_NEXT mirror keeps session and world equal', () => {
    const before = 3;
    const good = applyFocusMirror(before + 1, before);
    assert.equal(good.sessionIndex, good.wasmSlot);
    assert.equal(good.sessionIndex, 4);
  });

  it('documents broken double-advance desync', () => {
    const bad = applyFocusDoubleAdvance(3, 3);
    assert.equal(bad.sessionIndex, 4); // Ship
    assert.equal(bad.wasmSlot, 5); // Grocery — one ahead (the bug)
    assert.notEqual(bad.sessionIndex, bad.wasmSlot);
  });

  it('store focus index drives single mirror after Tab-like next', () => {
    const graph = {
      nodes: [
        { id: 'a', label: 'Ship ensembly OSS', type: 'action' },
        { id: 'b', label: 'Grocery errand', type: 'physical', realm: 'physical' },
        { id: 'c', label: 'HITL', type: 'hitl', hitl: true },
      ],
    };
    const store = createGameStore(graph);
    let wasm = 0;
    store.subscribe(() => {
      syncWorldFocusFromSession(store.session, (s) => {
        wasm = s;
      });
    });
    store.dispatch(mapKeyEvent({ key: 'Tab' })); // FOCUS_NEXT
    assert.equal(store.view.focusIndex, 1);
    assert.equal(store.view.focusLabel, 'Grocery errand');
    assert.equal(wasm, 1);
    store.dispatch(mapKeyEvent({ key: 'Tab' }));
    assert.equal(store.view.focusIndex, 2);
    assert.equal(wasm, 2);
  });
});
