/**
 * Keyboard → game action map (pure).
 * Game-control kit: nav, select, approve/deny, help, voice toggle.
 */

/** @type {Record<string, { type: string, payload?: object }>} */
export const KEY_BINDINGS = Object.freeze({
  Tab: { type: 'FOCUS_NEXT' },
  'Shift+Tab': { type: 'FOCUS_PREV' },
  ArrowRight: { type: 'FOCUS_RIGHT' },
  ArrowLeft: { type: 'FOCUS_LEFT' },
  ArrowUp: { type: 'FOCUS_UP' },
  ArrowDown: { type: 'FOCUS_DOWN' },
  j: { type: 'FOCUS_NEXT' },
  k: { type: 'FOCUS_PREV' },
  Enter: { type: 'SELECT' },
  ' ': { type: 'SELECT' },
  c: { type: 'COMPLETE' },
  C: { type: 'COMPLETE' },
  a: { type: 'APPROVE' },
  A: { type: 'APPROVE' },
  d: { type: 'DENY' },
  D: { type: 'DENY' },
  y: { type: 'APPROVE' },
  n: { type: 'DENY' },
  '?': { type: 'TOGGLE_HELP' },
  '/': { type: 'TOGGLE_HELP' },
  Escape: { type: 'CLOSE_CHROME' },
  v: { type: 'VOICE_START' },
  V: { type: 'VOICE_START' },
  Escape_voice: { type: 'VOICE_STOP' },
  u: { type: 'UNDO' },
  U: { type: 'UNDO' },
  r: { type: 'REDO' },
  R: { type: 'REDO' },
  // Chrome on demand — clean world by default
  b: { type: 'TOGGLE_BOARD' },
  B: { type: 'TOGGLE_BOARD' },
  q: { type: 'TOGGLE_BOARD' },
  Q: { type: 'TOGGLE_BOARD' },
  i: { type: 'TOGGLE_BARS' },
  I: { type: 'TOGGLE_BARS' },
  m: { type: 'TOGGLE_MENU' },
  M: { type: 'TOGGLE_MENU' },
  h: { type: 'TOGGLE_HELP' },
  H: { type: 'TOGGLE_HELP' },
});

/**
 * Normalize a keyboard event-like object to a binding key.
 * @param {{ key?: string, code?: string, shiftKey?: boolean, ctrlKey?: boolean, metaKey?: boolean, altKey?: boolean }} ev
 */
export function bindingKeyFromEvent(ev = {}) {
  const key = ev.key || ev.code || '';
  if (key === 'Tab' && ev.shiftKey) return 'Shift+Tab';
  if (key === 'Tab') return 'Tab';
  if (key === ' ') return ' ';
  if (key === 'Escape') return 'Escape';
  if (key === '?' || (key === '/' && ev.shiftKey)) return '?';
  if (key === '/') return '/';
  // single printable
  if (key.length === 1) return key;
  return key;
}

/**
 * Map event → action or null.
 * @param {{ key?: string, shiftKey?: boolean }} ev
 * @param {{ voiceListening?: boolean }} [ctx]
 */
export function mapKeyEvent(ev, ctx = {}) {
  if (ctx.voiceListening && (ev.key === 'Escape' || ev.key === 'v' || ev.key === 'V')) {
    return { type: 'VOICE_STOP' };
  }
  const bk = bindingKeyFromEvent(ev);
  return KEY_BINDINGS[bk] || null;
}

/**
 * Human-readable help lines for HUD.
 */
export function helpLines() {
  return [
    '— Clean world · open chrome with keys —',
    'B / Q · growth board (quests + beacons)',
    'I · top bars (level · XP · gates · GPU)',
    'M · command menu (voice · type)',
    '? / H · this help',
    'Esc · close all panels / help / voice',
    '',
    '— Play —',
    'Tab / j · next focus',
    'Shift+Tab / k · prev focus',
    'Arrows · grid nav',
    'Enter / Space / C · claim beacon (+XP)',
    'A / Y · approve HITL (+XP)',
    'D / N · deny HITL (+XP)',
    'V · voice',
    'U · undo · R · redo',
    '',
    'Growth: physical > presence > craft · clear gates · streak',
  ];
}

/**
 * Optional gamepad-shaped button index map (game control kit).
 * Standard mapping: 0=A approve, 1=B deny, 12/13/14/15 d-pad, 9=start help
 */
export const GAMEPAD_BUTTON_ACTIONS = Object.freeze({
  0: { type: 'APPROVE' },
  1: { type: 'DENY' },
  9: { type: 'TOGGLE_HELP' },
  12: { type: 'FOCUS_UP' },
  13: { type: 'FOCUS_DOWN' },
  14: { type: 'FOCUS_LEFT' },
  15: { type: 'FOCUS_RIGHT' },
});

/**
 * @param {number} buttonIndex
 */
export function mapGamepadButton(buttonIndex) {
  return GAMEPAD_BUTTON_ACTIONS[buttonIndex] || null;
}
