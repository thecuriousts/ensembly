/**
 * Durable HITL wait snapshot — idle-resume pattern (Stately/Eve inspired).
 * Also holds physical claim/complete so body work closes like auth.
 * Pure state transitions; IO lives in turn.js / CLI.
 */

/** @typedef {'pending' | 'approved' | 'denied'} ApprovalStatus */
/** @typedef {'open' | 'claimed' | 'completed'} PhysicalStatus */
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
    physical: [],
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
 * Upsert physical rows from realm-physical actions (do not reopen completed).
 * @param {Array<object>} actions
 * @param {object|null} existing
 */
export function upsertPhysicalFromActions(actions = [], existing = null, opts = {}) {
  const snap = existing ? structuredClone(existing) : emptySnapshot(opts);
  if (!Array.isArray(snap.physical)) snap.physical = [];
  const now = opts.now || new Date().toISOString();
  const byId = new Map(snap.physical.map((p) => [p.id, p]));

  for (const action of actions) {
    const realm = action.realm || action.realmInfo?.realm;
    if (realm !== 'physical') continue;
    const id = action.id || action.title;
    if (!id) continue;
    const prev = byId.get(id);
    if (prev && prev.status === 'completed') {
      // Keep closed; do not re-open automatically
      continue;
    }
    byId.set(id, {
      id,
      title: action.title || id,
      area: action.area || null,
      status: prev?.status || 'open',
      createdAt: prev?.createdAt || now,
      updatedAt: now,
    });
  }

  snap.physical = [...byId.values()];
  snap.updatedAt = now;
  return snap;
}

/**
 * Apply claim | complete | release on a physical action id.
 * @param {object} snapshot
 * @param {string} actionId
 * @param {'claim' | 'complete' | 'release'} decision
 */
export function applyPhysicalDecision(snapshot, actionId, decision, opts = {}) {
  if (!snapshot || typeof snapshot !== 'object') {
    throw new Error('snapshot required');
  }
  if (!['claim', 'complete', 'release'].includes(decision)) {
    throw new Error('decision must be claim, complete, or release');
  }
  const now = opts.now || new Date().toISOString();
  const next = structuredClone(snapshot);
  if (!Array.isArray(next.physical)) next.physical = [];

  let item = next.physical.find((p) => p.id === actionId);
  if (!item) {
    // Allow claim/complete of an id not yet upserted (operator names the act)
    item = {
      id: actionId,
      title: opts.title || actionId,
      area: opts.area || null,
      status: 'open',
      createdAt: now,
      updatedAt: now,
    };
    next.physical.push(item);
  }

  if (decision === 'claim') {
    if (item.status === 'completed') {
      throw new Error(`physical already completed: ${item.id}`);
    }
    item.status = 'claimed';
  } else if (decision === 'complete') {
    item.status = 'completed';
  } else {
    // release → open again (unless we want complete sticky — release only from claimed)
    if (item.status === 'completed') {
      throw new Error(`cannot release completed physical: ${item.id}`);
    }
    item.status = 'open';
  }
  item.updatedAt = now;
  item.actor = opts.actor || 'operator';
  next.history = [
    ...(next.history || []),
    { id: item.id, decision: `physical_${decision}`, at: now, actor: item.actor },
  ];
  next.updatedAt = now;
  return next;
}

/**
 * Physical rows that are still open or claimed (not completed).
 */
export function listActivePhysical(snapshot) {
  return (snapshot?.physical || []).filter((p) => p.status === 'open' || p.status === 'claimed');
}

/**
 * Map of physical id → status from snapshot.
 */
export function physicalStatusMap(snapshot) {
  const map = new Map();
  for (const p of snapshot?.physical || []) {
    map.set(p.id, p.status);
  }
  return map;
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
  // Backward compatible: older snapshots lack physical[]
  if (!Array.isArray(obj.physical)) obj.physical = [];
  return obj;
}
