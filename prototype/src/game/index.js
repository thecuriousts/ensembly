/**
 * Game of Peram — pure game kit API
 * Patterns.dev: module, command, observer, performance
 * @see https://www.patterns.dev/ai/skills/
 */
export { createSession, dispatch, focusedNode, sessionView } from './session.js';
export {
  KEY_BINDINGS,
  mapKeyEvent,
  bindingKeyFromEvent,
  helpLines,
  mapGamepadButton,
  GAMEPAD_BUTTON_ACTIONS,
} from './input.js';
export {
  parseVoiceCommand,
  voiceVocabulary,
  speechRecognitionAvailable,
} from './voice.js';
export { Observable } from './observable.js';
export { CommandBus, createCommand } from './commands.js';
export { createGameStore } from './store.js';
export {
  syncWorldFocusFromSession,
  applyFocusMirror,
  applyFocusDoubleAdvance,
} from './focus-sync.js';
export {
  emptyGrowth,
  nodeRole,
  isCompletable,
  levelFromXp,
  buildQuests,
  growthView,
  growthCoachLine,
  applyGain,
  XP_TABLE,
  LEVEL_TITLES,
} from './growth.js';
export {
  SPN_SYMBOL,
  SPN_BASE,
  buildSpnQuote,
  spnFromSession,
  eventToPriceDelta,
  formatSpnTicker,
  seriesToSvgPath,
} from './spn.js';
export * from './world/index.js';
