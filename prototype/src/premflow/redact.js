/**
 * Capture privacy — micro-capture may hold finance/health/PII.
 * Symlink is local-OK; any *derived* insight for share/cloud/public IR must redact.
 */
import { classifyItem } from '../privacy.js';

/**
 * Classify a single todo/log/journal line as pushable or private.
 * @param {string} line
 */
export function classifyCaptureLine(line) {
  const text = String(line || '').trim();
  const c = classifyItem({ title: text, body: text });
  return {
    line: text,
    visibility: c.visibility,
    pushable: c.pushable,
    hitl: c.hitl,
    reason: c.reason,
  };
}

/**
 * Redact a line for shareable output (Eve, public dashboard, logs that might be committed).
 * Private lines become a stub with no raw text.
 * @param {string} line
 * @param {{ mode?: 'stub'|'drop' }} [opts]
 * @returns {string|null} null if drop
 */
export function redactCaptureLine(line, opts = {}) {
  const c = classifyCaptureLine(line);
  if (c.pushable && c.visibility === 'public') {
    return c.line;
  }
  if (opts.mode === 'drop') return null;
  const tag = c.hitl ? 'private+hitl' : 'private';
  return `[redacted:${tag}]`;
}

/**
 * Aggregate stats + redacted samples for insights / dashboards that must not leak.
 * Never includes raw private/finance/medical text in `shareable`.
 *
 * @param {{ todos?: string[], logTail?: string[], journalFiles?: string[] }} snap
 */
export function projectCaptureForShare(snap = {}) {
  const todos = snap.todos || [];
  const logTail = snap.logTail || [];

  let privateTodo = 0;
  let publicTodo = 0;
  let hitlFlags = 0;
  const shareableTodos = [];
  const privateReasons = {};

  for (const t of todos) {
    const c = classifyCaptureLine(t);
    if (c.visibility === 'public' && c.pushable) {
      publicTodo += 1;
      shareableTodos.push(c.line);
    } else {
      privateTodo += 1;
      if (c.hitl) hitlFlags += 1;
      const key = c.reason || 'private';
      privateReasons[key] = (privateReasons[key] || 0) + 1;
    }
  }

  let privateLog = 0;
  let publicLog = 0;
  const shareableLog = [];
  for (const l of logTail) {
    const c = classifyCaptureLine(l);
    if (c.visibility === 'public' && c.pushable) {
      publicLog += 1;
      shareableLog.push(c.line);
    } else {
      privateLog += 1;
    }
  }

  return {
    version: 1,
    kind: 'capture_share_projection',
    /** Safe for Eve digests / public IR — no private raw text */
    shareable: {
      todoPublicCount: publicTodo,
      todoPrivateCount: privateTodo,
      logPublicCount: publicLog,
      logPrivateCount: privateLog,
      hitlRelatedCount: hitlFlags,
      todos: shareableTodos,
      logTail: shareableLog,
      journalFileCount: (snap.journalFiles || []).length,
      // filenames only if non-sensitive pattern; never file bodies
      journalFiles: (snap.journalFiles || []).map((f) => String(f)),
    },
    /** Local diagnostics only — reasons aggregated, not full lines */
    localOnly: {
      privateReasonCounts: privateReasons,
      totalTodos: todos.length,
      totalLogLines: logTail.length,
    },
    policy:
      'default-deny; finance/medical/PII patterns private; never embed raw private capture in public/watch or Eve',
  };
}
