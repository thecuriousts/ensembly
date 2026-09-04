/**
 * Mediator-style game store: one place for session + observers + commands.
 * Patterns: Observer + Command + Module (Patterns.dev).
 * @see https://www.patterns.dev/ai/skills/
 */
import { createSession, sessionView } from './session.js';
import { CommandBus } from './commands.js';
import { Observable } from './observable.js';

/**
 * @param {{ nodes?: any[], edges?: any[], meta?: object }} graph
 * @param {object | null} snapshot
 * @param {{ maxHistory?: number }} [opts]
 */
export function createGameStore(graph, snapshot = null, opts = {}) {
  const bus = new CommandBus(createSession(graph, snapshot), opts);
  const changed = new Observable();

  function emit(reason) {
    changed.notify({
      reason,
      session: bus.session,
      view: sessionView(bus.session),
      canUndo: bus.canUndo,
      canRedo: bus.canRedo,
    });
  }

  return {
    get session() {
      return bus.session;
    },
    get view() {
      return sessionView(bus.session);
    },
    /**
     * @param {(data: any) => void} fn
     */
    subscribe(fn) {
      return changed.subscribe(fn);
    },
    /**
     * @param {{ type: string, payload?: any }} action
     */
    dispatch(action) {
      bus.execute(action);
      emit(action.type);
      return bus.session;
    },
    undo() {
      bus.undo();
      emit('UNDO');
      return bus.session;
    },
    redo() {
      bus.redo();
      emit('REDO');
      return bus.session;
    },
    get canUndo() {
      return bus.canUndo;
    },
    get canRedo() {
      return bus.canRedo;
    },
  };
}
