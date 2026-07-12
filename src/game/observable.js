/**
 * Observer pattern (Patterns.dev) — subscribe / unsubscribe / notify.
 * @see https://www.patterns.dev/vanilla/observer-pattern
 * @see https://www.patterns.dev/ai/skills/
 */

export class Observable {
  constructor() {
    /** @type {Set<Function>} */
    this.observers = new Set();
  }

  /**
   * @param {(data: any) => void} fn
   * @returns {() => void} unsubscribe
   */
  subscribe(fn) {
    this.observers.add(fn);
    return () => this.unsubscribe(fn);
  }

  /**
   * @param {(data: any) => void} fn
   */
  unsubscribe(fn) {
    this.observers.delete(fn);
  }

  /**
   * @param {any} data
   */
  notify(data) {
    for (const fn of this.observers) {
      try {
        fn(data);
      } catch (err) {
        console.error('[Observable] observer error', err);
      }
    }
  }

  get size() {
    return this.observers.size;
  }
}
