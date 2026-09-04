/**
 * Command pattern (Patterns.dev) — encapsulate actions; enable history / undo.
 * @see https://www.patterns.dev/vanilla/command-pattern
 * @see https://www.patterns.dev/ai/skills/
 */
import { dispatch } from './session.js';

/**
 * @typedef {{ type: string, payload?: any }} GameAction
 */

/**
 * @param {GameAction} action
 * @param {{ inverse?: GameAction | null }} [meta]
 */
export function createCommand(action, meta = {}) {
  return {
    action,
    inverse: meta.inverse ?? null,
    at: Date.now(),
  };
}

/**
 * Game command bus: execute + history (bounded).
 * Session is replaced immutably via dispatch.
 */
export class CommandBus {
  /**
   * @param {object} session initial session
   * @param {{ maxHistory?: number }} [opts]
   */
  constructor(session, opts = {}) {
    this.session = session;
    this.maxHistory = opts.maxHistory ?? 64;
    /** @type {Array<{ action: GameAction, inverse: GameAction | null, at: number, beforeTick: number }>} */
    this.history = [];
    this.undone = [];
  }

  /**
   * @param {GameAction} action
   * @param {{ inverse?: GameAction | null }} [meta]
   */
  execute(action, meta = {}) {
    if (!action?.type) return this.session;
    const beforeTick = this.session.tick || 0;
    const inverse = meta.inverse ?? inferInverse(this.session, action);
    this.session = dispatch(this.session, action);
    this.history.push({
      action,
      inverse,
      at: Date.now(),
      beforeTick,
    });
    if (this.history.length > this.maxHistory) {
      this.history.shift();
    }
    this.undone = [];
    return this.session;
  }

  /**
   * Undo last reversible command (if inverse recorded).
   */
  undo() {
    const last = this.history.pop();
    if (!last) return this.session;
    if (last.inverse) {
      this.session = dispatch(this.session, last.inverse);
      this.undone.push(last);
    } else {
      // non-invertible — put back
      this.history.push(last);
    }
    return this.session;
  }

  /**
   * Redo last undone command.
   */
  redo() {
    const cmd = this.undone.pop();
    if (!cmd) return this.session;
    this.session = dispatch(this.session, cmd.action);
    this.history.push(cmd);
    return this.session;
  }

  get canUndo() {
    return this.history.some((h) => h.inverse);
  }

  get canRedo() {
    return this.undone.length > 0;
  }
}

/**
 * Best-effort inverse for nav/help (not for approve/deny which are one-way in game).
 * @param {object} session
 * @param {GameAction} action
 */
function inferInverse(session, action) {
  switch (action.type) {
    case 'FOCUS_NEXT':
      return { type: 'FOCUS_PREV' };
    case 'FOCUS_PREV':
      return { type: 'FOCUS_NEXT' };
    case 'FOCUS_LEFT':
      return { type: 'FOCUS_RIGHT' };
    case 'FOCUS_RIGHT':
      return { type: 'FOCUS_LEFT' };
    case 'FOCUS_UP':
      return { type: 'FOCUS_DOWN' };
    case 'FOCUS_DOWN':
      return { type: 'FOCUS_UP' };
    case 'TOGGLE_HELP':
      return { type: 'TOGGLE_HELP' };
    case 'VOICE_START':
      return { type: 'VOICE_STOP' };
    case 'VOICE_STOP':
      return { type: 'VOICE_START' };
    default:
      return null;
  }
}
