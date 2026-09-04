/**
 * Session focus is source of truth; WASM world only mirrors index.
 * Prevents double-advance (dispatch + worldFocusNext) desync.
 */

/**
 * @param {{ focusIndex?: number } | null} session
 * @param {(slot: number) => void} setFocus
 */
export function syncWorldFocusFromSession(session, setFocus) {
  if (!session || typeof setFocus !== 'function') return false;
  const idx = Number(session.focusIndex);
  if (!Number.isFinite(idx) || idx < 0) return false;
  setFocus(idx | 0);
  return true;
}

/**
 * Simulate host runAction focus path: only session advances; mirror once.
 * @param {number} sessionIndex after dispatch
 * @param {number} wasmSlot before mirror
 * @returns {{ sessionIndex: number, wasmSlot: number }}
 */
export function applyFocusMirror(sessionIndex, wasmSlot) {
  // Correct path: wasm becomes session (not session+1)
  return { sessionIndex, wasmSlot: sessionIndex };
}

/**
 * Broken path that caused Grocery world vs Ship HUD (documented for test).
 */
export function applyFocusDoubleAdvance(sessionIndexBefore, wasmSlotBefore) {
  const sessionIndex = sessionIndexBefore + 1;
  let wasmSlot = wasmSlotBefore;
  wasmSlot = sessionIndex; // subscribe setFocus
  wasmSlot = wasmSlot + 1; // erroneous worldFocusNext
  return { sessionIndex, wasmSlot };
}
