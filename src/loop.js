/**
 * Looper-shaped bounded day cycle for the life swarm.
 * Phases: ORIENT → PLAN → EXECUTE → VERIFY → REVIEW_GATE → INTEGRATE → DONE
 * Hard budgets + HITL pause points. Pure control logic + card state.
 */

export const PHASES = [
  'IDLE',
  'ORIENT',
  'PLAN',
  'HITL_PLAN_GATE',
  'EXECUTE',
  'VERIFY',
  'REPAIR',
  'REVIEW_GATE',
  'HITL_REVIEW',
  'INTEGRATE',
  'DONE',
  'CANCELLED',
  'BLOCKED',
];

export const DEFAULT_BUDGETS = {
  max_loop_iters: 8,
  max_repair_rounds: 3,
  max_step_retries: 2,
  max_tool_calls_per_step: 25,
};

/**
 * Create a fresh Loop Card for the daily swarm.
 * @param {{ goal?: string, date?: string }} opts
 */
export function createLoopCard(opts = {}) {
  return {
    goal: opts.goal || 'Produce balanced daily self-organization plan for operator persona',
    phase: 'IDLE',
    date: opts.date || new Date().toISOString().slice(0, 10),
    success_criteria: [
      'Non-empty plan with projects, actions, schedule/balance sections',
      'Privacy classification applied to all actions',
      'Non-negotiable life areas represented or injected',
      'HITL gates listed for high-stakes items',
    ],
    budgets: { ...DEFAULT_BUDGETS, remaining_loop_iters: DEFAULT_BUDGETS.max_loop_iters, remaining_repair: DEFAULT_BUDGETS.max_repair_rounds },
    steps: [],
    model_role: 'fast',
    last_progress: null,
    pause_reason: null,
    open_risks: [],
    handoff: { artifacts: [], open_risks: [] },
    phase_log: [],
  };
}

/**
 * Transition phase with guard recording.
 * @param {object} card
 * @param {string} nextPhase
 * @param {{ progress?: string, pause_reason?: string|null, model_role?: string }} meta
 */
export function transition(card, nextPhase, meta = {}) {
  if (!PHASES.includes(nextPhase)) {
    throw new Error(`Unknown phase: ${nextPhase}`);
  }
  const prev = card.phase;
  const next = {
    ...card,
    phase: nextPhase,
    last_progress: meta.progress ?? card.last_progress,
    pause_reason: meta.pause_reason !== undefined ? meta.pause_reason : card.pause_reason,
    model_role: meta.model_role || card.model_role,
    phase_log: [
      ...(card.phase_log || []),
      { from: prev, to: nextPhase, at: new Date().toISOString(), progress: meta.progress || null },
    ],
  };
  if (prev !== 'IDLE' && nextPhase !== prev) {
    next.budgets = {
      ...next.budgets,
      remaining_loop_iters: Math.max(0, (next.budgets.remaining_loop_iters ?? next.budgets.max_loop_iters) - (isOuterCycleEdge(prev, nextPhase) ? 1 : 0)),
    };
  }
  return next;
}

function isOuterCycleEdge(from, to) {
  return from === 'INTEGRATE' && (to === 'PLAN' || to === 'ORIENT');
}

/**
 * Determine if HITL is required for a plan batch.
 * @param {Array<{ classification?: { hitl?: boolean } }>} actions
 */
export function needsHitl(actions = []) {
  return actions.some((a) => a.classification?.hitl || a.hitl);
}

/**
 * Run the deterministic day-cycle skeleton (no external side effects).
 * EXECUTE here means "assemble plan artifact", not mutate email/calendar.
 *
 * @param {{
 *   orient: () => object,
 *   plan: (ctx: object) => object,
 *   execute: (ctx: object) => object,
 *   verify: (ctx: object) => { ok: boolean, errors: string[] },
 *   integrate: (ctx: object) => object,
 * }} handlers
 * @param {{ autoApprovePlan?: boolean, date?: string }} opts
 */
export function runDayLoop(handlers, opts = {}) {
  let card = createLoopCard({ date: opts.date });
  const autoApprove = opts.autoApprovePlan !== false;
  const trail = [];

  card = transition(card, 'ORIENT', { model_role: 'fast', progress: 'start orient' });
  const orientCtx = handlers.orient();
  trail.push({ phase: 'ORIENT', ok: true });
  card = transition(card, 'PLAN', { model_role: 'deep', progress: 'context loaded' });

  const planCtx = handlers.plan(orientCtx);
  trail.push({ phase: 'PLAN', ok: true });

  if (planCtx.requiresHitl && !autoApprove) {
    card = transition(card, 'HITL_PLAN_GATE', {
      pause_reason: 'High-stakes actions in plan require human approval',
      model_role: 'review',
    });
    trail.push({ phase: 'HITL_PLAN_GATE', ok: false, paused: true });
    return {
      card,
      trail,
      status: 'HITL_PLAN_GATE',
      artifact: planCtx.artifact || null,
      context: { ...orientCtx, ...planCtx },
    };
  }

  card = transition(card, 'EXECUTE', { model_role: 'coding', progress: 'assemble day plan' });
  const execCtx = handlers.execute({ ...orientCtx, ...planCtx });
  trail.push({ phase: 'EXECUTE', ok: true });

  card = transition(card, 'VERIFY', { model_role: 'fast', progress: 'verify plan shape' });
  let verify = handlers.verify(execCtx);
  let repairUsed = 0;
  while (!verify.ok && repairUsed < card.budgets.max_repair_rounds) {
    card = transition(card, 'REPAIR', {
      model_role: 'coding',
      progress: `repair ${repairUsed + 1}: ${verify.errors.join('; ')}`,
    });
    repairUsed += 1;
    card.budgets.remaining_repair = card.budgets.max_repair_rounds - repairUsed;
    // Re-execute assembly only (bounded)
    const repaired = handlers.execute({ ...orientCtx, ...planCtx, repair: verify.errors });
    Object.assign(execCtx, repaired);
    card = transition(card, 'VERIFY', { progress: 're-verify' });
    verify = handlers.verify(execCtx);
    trail.push({ phase: 'REPAIR', ok: verify.ok, errors: verify.errors });
  }

  if (!verify.ok) {
    card = transition(card, 'BLOCKED', {
      pause_reason: verify.errors.join('; '),
      progress: 'verify failed budget exhausted',
    });
    trail.push({ phase: 'BLOCKED', ok: false });
    return { card, trail, status: 'BLOCKED', artifact: execCtx.artifact || null, context: execCtx };
  }

  trail.push({ phase: 'VERIFY', ok: true });

  const hitlActions = (execCtx.actions || []).filter((a) => a.classification?.hitl);
  card = transition(card, 'REVIEW_GATE', {
    model_role: 'review',
    progress: `${hitlActions.length} HITL-flagged actions`,
  });
  trail.push({ phase: 'REVIEW_GATE', ok: true, hitl_count: hitlActions.length });

  card = transition(card, 'INTEGRATE', {
    model_role: 'fast',
    progress: 'write artifact summary',
  });
  const integrated = handlers.integrate(execCtx);
  trail.push({ phase: 'INTEGRATE', ok: true });

  card = transition(card, 'DONE', { progress: 'day plan ready' });
  card.handoff = {
    artifacts: integrated.artifacts || [],
    open_risks: hitlActions.map((a) => a.title || a.id),
  };
  trail.push({ phase: 'DONE', ok: true });

  return {
    card,
    trail,
    status: 'DONE',
    artifact: integrated.artifact || execCtx.artifact,
    context: { ...execCtx, ...integrated },
    hitlActions,
  };
}
