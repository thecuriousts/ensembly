/**
 * Durable HITL wait snapshot — idle-resume pattern (Stately/Eve inspired).
 * Pure state transitions; IO lives in turn.js / CLI.
 */

/** @typedef {'pending' | 'approved' | 'denied'} ApprovalStatus */
/** @typedef {'idle_waiting' | 'clear' | 'partial'} SnapshotStatus */

/**
 * @returns {object} empty snapshot
 */
export function emptySnapshot(opts = {}) {
  return {
    version: 1,
    status: 'clear',
    phase: 'IDLE',
    pending: [],
    history: [],
    updatedAt: opts.now || new Date().toISOString(),
    meta: { source: opts.source || 'ensembly' },
  };
}

/**
 * Build or merge pending authorizations from HITL-classified actions.
 * @param {Array<object>} actions enriched with classification
 * @param {object} [existing]
 */
export function upsertPendingFromActions(actions = [], existing = null, opts = {}) {
  const snap = existing ? structuredClone(existing) : emptySnapshot(opts);
  const now = opts.now || new Date().toISOString();
  const byId = new Map((snap.pending || []).map((p) => [p.id, p]));

  for (const action of actions) {
    const needs =
      action.classification?.hitl === true ||
      action.hitl === true ||
      Boolean(action.kind && needsApprovalKind(action.kind));
    if (!needs) continue;

    const id = `auth-${action.id || action.title || Math.random().toString(36).slice(2, 8)}`;
    const key = action.id ? `auth-${action.id}` : id;
    const prev = byId.get(key);
    if (prev && (prev.status === 'approved' || prev.status === 'denied')) {
      // Keep resolved; do not re-open automatically
      continue;
    }
    byId.set(key, {
      id: key,
      actionId: action.id || null,
      title: action.title || action.id || key,
      kind: action.kind || 'hitl',
      area: action.area || null,
      realm: action.realm || action.realmInfo?.realm || null,
      status: prev?.status || 'pending',
      createdAt: prev?.createdAt || now,
      updatedAt: now,
      reason: action.classification?.reason || 'HITL required',
    });
  }

  snap.pending = [...byId.values()];
  snap.updatedAt = now;
  snap.status = deriveStatus(snap.pending);
  snap.phase = snap.status === 'idle_waiting' ? 'HITL_WAIT' : snap.status === 'clear' ? 'CLEAR' : 'PARTIAL';
  return snap;
}

function needsApprovalKind(kind) {
  return [
    'external_email_send',
    'job_application_submit',
    'calendar_mutate',
    'finance_transfer',
    'git_push_shared',
    'publish_private_data',
  ].includes(kind);
}

/**
 * @param {Array<{ status: string }>} pending
 */
export function deriveStatus(pending = []) {
  const open = pending.filter((p) => p.status === 'pending');
  if (open.length === 0) return 'clear';
  const resolved = pending.filter((p) => p.status === 'approved' || p.status === 'denied');
  if (resolved.length > 0 && open.length > 0) return 'partial';
  return 'idle_waiting';
}

/**
 * Apply approve or deny — advances wait snapshot.
 * @param {object} snapshot
 * @param {string} approvalId
 * @param {'approve' | 'deny'} decision
 */
export function applyDecision(snapshot, approvalId, decision, opts = {}) {
  if (!snapshot || typeof snapshot !== 'object') {
    throw new Error('snapshot required');
  }
  if (decision !== 'approve' && decision !== 'deny') {
    throw new Error('decision must be approve or deny');
  }
  const now = opts.now || new Date().toISOString();
  const next = structuredClone(snapshot);
  const item = (next.pending || []).find((p) => p.id === approvalId || p.actionId === approvalId);
  if (!item) {
    throw new Error(`approval not found: ${approvalId}`);
  }
  if (item.status !== 'pending') {
    throw new Error(`approval already ${item.status}: ${item.id}`);
  }
  item.status = decision === 'approve' ? 'approved' : 'denied';
  item.updatedAt = now;
  item.decidedBy = opts.actor || 'operator';
  next.history = [
    ...(next.history || []),
    { id: item.id, decision: item.status, at: now, actor: item.decidedBy },
  ];
  next.updatedAt = now;
  next.status = deriveStatus(next.pending);
  next.phase = next.status === 'idle_waiting' ? 'HITL_WAIT' : next.status === 'clear' ? 'CLEAR' : 'PARTIAL';
  return next;
}

/**
 * Pending authorizations only.
 */
export function listPending(snapshot) {
  return (snapshot?.pending || []).filter((p) => p.status === 'pending');
}

/**
 * JSON-serializable check (round-trip).
 */
export function serializeSnapshot(snapshot) {
  return JSON.stringify(snapshot);
}

export function parseSnapshot(json) {
  const obj = typeof json === 'string' ? JSON.parse(json) : json;
  if (!obj || obj.version !== 1) {
    throw new Error('unsupported snapshot version');
  }
  return obj;
}
