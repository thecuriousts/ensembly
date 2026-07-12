/**
 * Voice / speech-shaped command → game actions (pure).
 * Web Speech API is optional runtime; this mapper is the shipped path.
 */

/** @type {Array<{ re: RegExp, action: { type: string, payload?: object } }>} */
const RULES = [
  { re: /\b(approve|yes|confirm|accept)\b/i, action: { type: 'APPROVE' } },
  { re: /\b(deny|no|reject|refuse)\b/i, action: { type: 'DENY' } },
  { re: /\b(next|forward)\b/i, action: { type: 'FOCUS_NEXT' } },
  { re: /\b(previous|prev|back)\b/i, action: { type: 'FOCUS_PREV' } },
  { re: /\b(up)\b/i, action: { type: 'FOCUS_UP' } },
  { re: /\b(down)\b/i, action: { type: 'FOCUS_DOWN' } },
  { re: /\b(left)\b/i, action: { type: 'FOCUS_LEFT' } },
  { re: /\b(right)\b/i, action: { type: 'FOCUS_RIGHT' } },
  { re: /\b(select|choose|pick)\b/i, action: { type: 'SELECT' } },
  { re: /\b(complete|claim|done|finish|check off)\b/i, action: { type: 'COMPLETE' } },
  { re: /\b(help|commands)\b/i, action: { type: 'TOGGLE_HELP' } },
  { re: /\b(stop listening|cancel voice|quiet)\b/i, action: { type: 'VOICE_STOP' } },
  { re: /\b(listen|start voice|voice)\b/i, action: { type: 'VOICE_START' } },
  { re: /\b(undo)\b/i, action: { type: 'UNDO' } },
  { re: /\b(redo)\b/i, action: { type: 'REDO' } },
];

/**
 * Parse free-text (speech transcript or typed proxy) into actions.
 * @param {string} text
 * @returns {{ actions: Array<{ type: string, payload?: object }>, transcript: string, matched: boolean }}
 */
export function parseVoiceCommand(text) {
  const transcript = String(text || '').trim();
  if (!transcript) {
    return { actions: [], transcript: '', matched: false };
  }
  const actions = [];
  for (const rule of RULES) {
    if (rule.re.test(transcript)) {
      actions.push({ ...rule.action });
    }
  }
  // Always record transcript for HUD
  actions.unshift({ type: 'VOICE_TEXT', payload: { text: transcript } });
  return {
    actions,
    transcript,
    matched: actions.some((a) => a.type !== 'VOICE_TEXT'),
  };
}

/**
 * Vocabulary list for UI / help.
 */
export function voiceVocabulary() {
  return [
    'approve / yes',
    'deny / no',
    'next / previous',
    'up / down / left / right',
    'select / claim / done',
    'help',
    'listen / stop listening',
    'undo / redo',
  ];
}

/**
 * Whether Web Speech API is likely available (browser feature detect helper).
 * @param {object} [globalObj]
 */
export function speechRecognitionAvailable(globalObj = globalThis) {
  return Boolean(
    globalObj.SpeechRecognition || globalObj.webkitSpeechRecognition,
  );
}
