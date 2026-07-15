/**
 * Digital-flow IR — world/CLI activates a digital duty → HITL gate → execute hook.
 * First shipped class: bill_pay (Bank place). No unattended bank/email mutate.
 * Pure state machine; IO only via injected executeHook.
 */

/** @typedef {'idle' | 'pending_auth' | 'approved' | 'denied' | 'executed' | 'dry_run_ok' | 'failed'} FlowStatus */
/** @typedef {'dry_run' | 'live'} ExecutionMode */

export const DIGITAL_FLOW_KINDS = Object.freeze({
  BILL_PAY: 'bill_pay',
});

/** Map product flow kind → approvals/privacy HITL kind */
export const FLOW_TO_HITL_KIND = Object.freeze({
  bill_pay: 'finance_transfer',
});

/** World place label for each flow kind (presentation, not a second product) */
export const FLOW_PLACE = Object.freeze({
  bill_pay: 'Bank',
});

/**
 * @param {{
 *   id?: string,
 *   kind?: string,
 *   title?: string,
 *   amountLabel?: string,
 *   payeeLabel?: string,
 *   area?: string,
 *   executionMode?: ExecutionMode,
 *   now?: string,
 * }} [spec]
 */
export function createDigitalFlow(spec = {}) {
  const kind = spec.kind || DIGITAL_FLOW_KINDS.BILL_PAY;
  const id = spec.id || `flow-${kind}`;
  const hitlKind = FLOW_TO_HITL_KIND[kind] || 'hitl';
  const place = FLOW_PLACE[kind] || 'Digital';
  const now = spec.now || new Date().toISOString();
  return {
    version: 1,
    id,
    kind,
    place,
    title: spec.title || defaultTitle(kind, spec),
    amountLabel: spec.amountLabel || null,
    payeeLabel: spec.payeeLabel || null,
    area: spec.area || (kind === 'bill_pay' ? 'Finance' : 'Systems'),
    hitlKind,
    status: /** @type {FlowStatus} */ ('idle'),
    executionMode: spec.executionMode === 'live' ? 'live' : 'dry_run',
    authId: `auth-${id}`,
    actionId: id,
    createdAt: now,
    updatedAt: now,
    history: [],
    lastResult: null,
    reason: 'HITL required before digital mutate',
  };
}

function defaultTitle(kind, spec) {
  if (kind === 'bill_pay') {
    const payee = spec.payeeLabel || 'bill';
    return `Pay ${payee} at Bank`;
  }
  return `Digital flow ${kind}`;
}

/** Terminal / restartable statuses — operator may run another Bank pay cycle. */
export const REACTIVATABLE_FLOW_STATUSES = Object.freeze([
  'idle',
  'denied',
  'failed',
  'dry_run_ok',
  'executed',
  // approved without execute (stuck mid-cycle) — re-open as new activation
  'approved',
]);

/**
 * Activate flow → pending authorization (does not execute).
 * Re-activate allowed after dry_run_ok / executed so durable store is not a one-shot trap.
 * @param {object} flow
 * @param {{ now?: string, actor?: string }} [opts]
 */
export function activateDigitalFlow(flow, opts = {}) {
  if (!flow || typeof flow !== 'object') throw new Error('flow required');
  if (flow.status === 'pending_auth') {
    throw new Error(`cannot activate from status ${flow.status}`);
  }
  if (!REACTIVATABLE_FLOW_STATUSES.includes(flow.status)) {
    throw new Error(`cannot activate from status ${flow.status}`);
  }
  const now = opts.now || new Date().toISOString();
  const next = structuredClone(flow);
  next.status = 'pending_auth';
  next.updatedAt = now;
  next.history = [
    ...(next.history || []),
    { event: 'activate', at: now, actor: opts.actor || 'operator', place: next.place },
  ];
  next.lastResult = null;
  return next;
}

/**
 * Apply approve | deny on an activated flow.
 * @param {object} flow
 * @param {'approve' | 'deny'} decision
 * @param {{ now?: string, actor?: string }} [opts]
 */
export function applyFlowDecision(flow, decision, opts = {}) {
  if (!flow || typeof flow !== 'object') throw new Error('flow required');
  if (flow.status !== 'pending_auth') {
    throw new Error(`cannot decide from status ${flow.status}`);
  }
  if (decision !== 'approve' && decision !== 'deny') {
    throw new Error('decision must be approve or deny');
  }
  const now = opts.now || new Date().toISOString();
  const next = structuredClone(flow);
  next.status = decision === 'approve' ? 'approved' : 'denied';
  next.updatedAt = now;
  next.history = [
    ...(next.history || []),
    {
      event: decision,
      at: now,
      actor: opts.actor || 'operator',
    },
  ];
  return next;
}

/**
 * Execute only after approve. Deny path must never call this with effect.
 * Injected executeHook receives redacted ctx; default dry_run never mutates external systems.
 *
 * @param {object} flow
 * @param {{
 *   now?: string,
 *   actor?: string,
 *   executeHook?: (ctx: object) => object | Promise<object>,
 * }} [opts]
 * @returns {object | Promise<object>}
 */
export function executeDigitalFlow(flow, opts = {}) {
  if (!flow || typeof flow !== 'object') throw new Error('flow required');
  if (flow.status !== 'approved') {
    throw new Error(`cannot execute from status ${flow.status} (approve first)`);
  }
  const now = opts.now || new Date().toISOString();
  const mode = flow.executionMode === 'live' ? 'live' : 'dry_run';

  const ctx = {
    flowId: flow.id,
    kind: flow.kind,
    place: flow.place,
    title: flow.title,
    hitlKind: flow.hitlKind,
    executionMode: mode,
    // Never put real account numbers in IR; labels only
    amountLabel: flow.amountLabel,
    payeeLabel: flow.payeeLabel,
    actor: opts.actor || 'operator',
    at: now,
  };

  const hook =
    opts.executeHook ||
    ((c) => ({
      ok: true,
      mode: c.executionMode,
      message:
        c.executionMode === 'dry_run'
          ? `dry-run: would pay ${c.payeeLabel || 'bill'} at ${c.place}`
          : `live hook not configured for ${c.kind}`,
      mutated: false,
    }));

  const finish = (result) => {
    const next = structuredClone(flow);
    const ok = result && result.ok !== false;
    if (mode === 'dry_run') {
      next.status = ok ? 'dry_run_ok' : 'failed';
    } else {
      next.status = ok ? 'executed' : 'failed';
    }
    next.updatedAt = now;
    next.lastResult = {
      ok,
      mode,
      message: result?.message || null,
      mutated: Boolean(result?.mutated),
      detail: result?.detail ?? null,
    };
    next.history = [
      ...(next.history || []),
      {
        event: 'execute',
        at: now,
        actor: opts.actor || 'operator',
        mode,
        ok,
        mutated: next.lastResult.mutated,
      },
    ];
    return next;
  };

  const maybe = hook(ctx);
  if (maybe && typeof maybe.then === 'function') {
    return maybe.then(finish);
  }
  return finish(maybe);
}

/**
 * Full pure cycle for tests / CLI: activate → decide → (execute if approve).
 * Deny never invokes executeHook.
 *
 * @param {object} flowOrSpec flow or createDigitalFlow spec
 * @param {'approve' | 'deny'} decision
 * @param {{ now?: string, actor?: string, executeHook?: Function, executionMode?: ExecutionMode }} [opts]
 */
export function runDigitalFlowCycle(flowOrSpec, decision, opts = {}) {
  let flow =
    flowOrSpec?.status != null
      ? structuredClone(flowOrSpec)
      : createDigitalFlow({ ...flowOrSpec, executionMode: opts.executionMode || flowOrSpec?.executionMode });
  if (opts.executionMode) flow.executionMode = opts.executionMode === 'live' ? 'live' : 'dry_run';

  let executed = false;
  let hookCalls = 0;
  const wrappedHook = (ctx) => {
    hookCalls += 1;
    executed = true;
    if (opts.executeHook) return opts.executeHook(ctx);
    return {
      ok: true,
      mode: ctx.executionMode,
      message: `dry-run: would pay ${ctx.payeeLabel || 'bill'} at ${ctx.place}`,
      mutated: false,
    };
  };

  flow = activateDigitalFlow(flow, opts);
  const approval = flowToApprovalRecord(flow);
  flow = applyFlowDecision(flow, decision, opts);

  if (decision === 'deny') {
    return {
      flow,
      approval: flowToApprovalRecord(flow),
      executed: false,
      hookCalls: 0,
      result: null,
    };
  }

  const after = executeDigitalFlow(flow, { ...opts, executeHook: wrappedHook });
  if (after && typeof after.then === 'function') {
    return after.then((flowDone) => ({
      flow: flowDone,
      // Post-execute approval must be closed (not pending) — use flowToApprovalRecord
      approval: flowToApprovalRecord(flowDone),
      executed: true,
      hookCalls,
      result: flowDone.lastResult,
    }));
  }
  return {
    flow: after,
    approval: flowToApprovalRecord(after),
    executed: true,
    hookCalls,
    result: after.lastResult,
  };
}

/**
 * Map digital-flow status → approvals IR status (pending | approved | denied).
 * Only pending_auth is an open HITL gate. dry_run_ok / executed close the gate.
 * @param {string} flowStatus
 * @returns {'pending' | 'approved' | 'denied'}
 */
export function mapFlowStatusToApprovalStatus(flowStatus) {
  switch (flowStatus) {
    case 'pending_auth':
      return 'pending';
    case 'denied':
      return 'denied';
    case 'approved':
    case 'dry_run_ok':
    case 'executed':
    case 'failed':
      // HITL was granted (or execute failed after grant) — gate is closed
      return 'approved';
    case 'idle':
    default:
      // Not an open gate; idle must not reopen wait-snapshot as pending
      return 'approved';
  }
}

/**
 * Map a digital flow into an approvals-compatible pending row + action candidate.
 * @param {object} flow
 */
export function flowToApprovalRecord(flow) {
  return {
    id: flow.authId || `auth-${flow.id}`,
    actionId: flow.actionId || flow.id,
    title: flow.title,
    kind: flow.hitlKind || FLOW_TO_HITL_KIND[flow.kind] || 'hitl',
    area: flow.area || 'Finance',
    realm: 'digital',
    status: mapFlowStatusToApprovalStatus(flow.status),
    reason: flow.reason || 'HITL required before digital mutate',
    place: flow.place || null,
    digitalFlowId: flow.id,
    flowStatus: flow.status || null,
  };
}

/**
 * Action candidate for day/turn/graph (bill_pay → finance_transfer HITL).
 * @param {object} flow
 */
export function flowToActionCandidate(flow) {
  return {
    id: flow.actionId || flow.id,
    title: flow.title,
    area: flow.area || 'Finance',
    importance: 4,
    urgency: 4,
    public: false,
    kind: flow.hitlKind || 'finance_transfer',
    realm: 'digital',
    tags: ['digital', 'digital-flow', flow.kind, flow.place].filter(Boolean),
    source: 'digital-flow',
    place: flow.place,
    digitalFlowId: flow.id,
    classification: {
      visibility: 'private',
      reason: 'finance digital-flow default-deny',
      hitl: true,
      pushable: false,
    },
  };
}

/**
 * Graph/world place node for Bank (presentation of digital-flow IR).
 * @param {object} flow
 */
export function flowToPlaceNode(flow) {
  return {
    id: `place-${(flow.place || 'digital').toLowerCase()}`,
    type: 'place',
    label: flow.place || 'Digital',
    kind: 'world_place',
    flowId: flow.id,
    flowKind: flow.kind,
    flowStatus: flow.status,
    hitl: true,
    realm: 'digital',
    area: flow.area || null,
  };
}
