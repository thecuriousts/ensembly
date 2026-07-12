/**
 * Day cycle orchestration: ingest → prioritize → balance → privacy → format.
 * Uses looper-shaped runDayLoop.
 */
import fs from 'node:fs';
import path from 'node:path';
import { loadPersona, loadLocalState, resolveRoot } from './ingest.js';
import { prioritize, mergeCandidates } from './prioritize.js';
import { ensureBalance, proposeSchedule, balanceScore, countByArea } from './balance.js';
import { partitionByVisibility, classifyItem } from './privacy.js';
import { runDayLoop, needsHitl } from './loop.js';
import { formatDailyPlan } from './plan-format.js';

/**
 * Build day plan context from persona + state (pure assembly after load).
 */
export function buildDayPlan(persona, state = {}, opts = {}) {
  const date = opts.date || state.date || new Date().toISOString().slice(0, 10);
  const priorityStack = persona.priority_stack || [];
  const balanceWeights = persona.balance_weights || {};
  const areaCounts = state.area_counts || {};

  const candidates = mergeCandidates(persona.project_seeds || [], state.extra_candidates || []);
  const ranked = prioritize(candidates, {
    balanceWeights,
    areaCounts,
    priorityStack,
  });

  const balanced = ensureBalance(ranked, persona);
  const withPrivacy = balanced.items.map((item) => ({
    ...item,
    classification: classifyItem(item),
  }));

  // Actions for the day: top N after balance, prefer one primary career + non-negotiables
  const actions = selectDailyActions(withPrivacy, persona);
  const { publicItems, privateItems } = partitionByVisibility(actions);
  const schedule = proposeSchedule(persona, actions);
  const balance = balanceScore(actions, {
    nonNegotiableAreas: ['Relationships', 'Health'],
  });

  const projects = withPrivacy.slice(0, 8);
  const hitlActions = actions.filter((a) => a.classification?.hitl);

  return {
    date,
    projects,
    actions,
    schedule,
    balance,
    publicActions: publicItems,
    privateActions: privateItems,
    hitlActions,
    injections: balanced.injections,
    areaCounts: countByArea(actions),
    requiresHitl: needsHitl(actions) && opts.forceHitlPlanGate === true,
  };
}

/**
 * Select a harmonious daily action set: not a single unbounded dump.
 */
export function selectDailyActions(ranked = [], persona = {}) {
  const max = 7;
  const selected = [];
  const seen = new Set();
  const areaOnceDeep = new Set();

  // Always include balance injections first
  for (const item of ranked) {
    if (item.source === 'balance_inject' && !seen.has(item.id)) {
      selected.push(item);
      seen.add(item.id);
    }
  }

  // One primary DO_FIRST career or systems
  for (const item of ranked) {
    if (seen.has(item.id)) continue;
    if (item.quadrant === 'DO_FIRST' && (item.area === 'Career' || item.area === 'Systems')) {
      selected.push(item);
      seen.add(item.id);
      areaOnceDeep.add(item.area);
      break;
    }
  }

  // Fill remaining with diversity
  for (const item of ranked) {
    if (selected.length >= max) break;
    if (seen.has(item.id)) continue;
    if (item.quadrant === 'ELIMINATE') continue;
    const areaCount = selected.filter((s) => s.area === item.area).length;
    if (areaCount >= 2) continue;
    selected.push(item);
    seen.add(item.id);
  }

  // Cap to max
  return selected.slice(0, max);
}

/**
 * Verify plan artifact structure.
 */
export function verifyPlanShape(ctx) {
  const errors = [];
  const md = ctx.artifact || '';
  if (!md || !String(md).trim()) errors.push('artifact empty');
  if (!/## Projects/i.test(md)) errors.push('missing Projects section');
  if (!/## Actions/i.test(md)) errors.push('missing Actions section');
  if (!/## Schedule/i.test(md)) errors.push('missing Schedule section');
  if (!(ctx.actions && ctx.actions.length)) errors.push('no actions selected');
  if (ctx.balance === undefined || ctx.balance === null) errors.push('no balance score');
  return { ok: errors.length === 0, errors };
}

/**
 * Run the full day loop and return markdown + structured result.
 * @param {{ root?: string, date?: string, write?: boolean, outDir?: string }} opts
 */
export function runDailySwarm(opts = {}) {
  const root = opts.root || resolveRoot();
  const { persona, source: personaSource } = loadPersona(root);
  const { state, path: statePath } = loadLocalState(root);

  const result = runDayLoop(
    {
      orient: () => ({ persona, personaSource, state, statePath, root }),
      plan: (ctx) => {
        const built = buildDayPlan(ctx.persona, ctx.state, { date: opts.date });
        return { ...built, requiresHitl: false };
      },
      execute: (ctx) => {
        const built =
          ctx.actions && ctx.projects
            ? ctx
            : buildDayPlan(ctx.persona, ctx.state, { date: opts.date });
        const cardStub = {
          goal: 'Produce balanced daily self-organization plan for operator persona',
          phase: 'EXECUTE',
          budgets: { max_loop_iters: 8, max_repair_rounds: 3 },
          pause_reason: null,
        };
        const artifact = formatDailyPlan({
          date: built.date,
          personaSource: ctx.personaSource,
          card: ctx.card || cardStub,
          trail: ctx.trail || [],
          projects: built.projects,
          actions: built.actions,
          schedule: built.schedule,
          balance: built.balance,
          publicActions: built.publicActions,
          privateActions: built.privateActions,
          hitlActions: built.hitlActions,
          injections: built.injections,
        });
        return { ...built, artifact, personaSource: ctx.personaSource };
      },
      verify: (ctx) => verifyPlanShape(ctx),
      integrate: (ctx) => {
        // Re-format with final card from outer loop — patched by caller via post
        return {
          artifact: ctx.artifact,
          artifacts: ['daily-plan.md'],
        };
      },
    },
    { date: opts.date || state.date, autoApprovePlan: true },
  );

  // Re-stamp artifact with final loop card + trail
  if (result.context && result.artifact) {
    const finalMd = formatDailyPlan({
      date: result.context.date,
      personaSource,
      card: result.card,
      trail: result.trail,
      projects: result.context.projects,
      actions: result.context.actions,
      schedule: result.context.schedule,
      balance: result.context.balance,
      publicActions: result.context.publicActions,
      privateActions: result.context.privateActions,
      hitlActions: result.hitlActions || result.context.hitlActions,
      injections: result.context.injections,
    });
    result.artifact = finalMd;
    result.context.artifact = finalMd;
  }

  if (opts.write !== false) {
    const planDir = opts.outDir || path.join(root, 'private', 'state', 'plans');
    fs.mkdirSync(planDir, { recursive: true });
    const date = result.context?.date || new Date().toISOString().slice(0, 10);
    const outPath = path.join(planDir, `${date}-daily-plan.md`);
    fs.writeFileSync(outPath, result.artifact, 'utf8');
    result.outPath = outPath;
  }

  return result;
}
