/**
 * Operator turn surface: physical pickups + pending authorizations.
 * Primary next-actions + machine-readable status IR for human + agent clone.
 */
import fs from 'node:fs';
import path from 'node:path';
import { physicalPickups, enrichWithRealm } from './realm.js';
import {
  upsertPendingFromActions,
  upsertPhysicalFromActions,
  applyDecision,
  applyPhysicalDecision,
  listPending,
  physicalStatusMap,
  emptySnapshot,
  parseSnapshot,
  serializeSnapshot,
} from './approvals.js';
import { classifyItem } from './privacy.js';
import { buildDayPlan } from './day.js';
import { loadPersona, loadLocalState, resolveRoot, loadJson } from './ingest.js';
import { buildGameGraph, graphToMermaid, graphToWatchHtml } from './graph.js';
import { syncPublicDashboard } from './dashboard.js';

/**
 * Parse HH:MM to minutes from midnight. Returns null if invalid.
 * @param {string} hm
 */
export function parseClockMinutes(hm) {
  if (!hm || typeof hm !== 'string') return null;
  const m = hm.trim().match(/^(\d{1,2}):(\d{2})$/);
  if (!m) return null;
  const h = Number(m[1]);
  const min = Number(m[2]);
  if (h > 23 || min > 59) return null;
  return h * 60 + min;
}

/**
 * Whether `nowMin` is inside [start, end), handling overnight windows.
 */
export function inScheduleWindow(nowMin, startMin, endMin) {
  if (startMin == null || endMin == null || nowMin == null) return false;
  if (endMin > startMin) return nowMin >= startMin && nowMin < endMin;
  // overnight e.g. 22:00–06:00
  return nowMin >= startMin || nowMin < endMin;
}

/**
 * Select single primary next physical act (schedule-aware when schedule present).
 * Pure. Pass opts.now (Date | ISO | ms) for deterministic tests.
 *
 * @param {Array<object>} pickups open physical actions
 * @param {Array<object>} schedule day schedule slots
 * @param {{ now?: Date|string|number }} [opts]
 * @returns {object|null}
 */
export function selectNextPhysical(pickups = [], schedule = [], opts = {}) {
  const open = (pickups || []).filter(Boolean);
  if (!open.length) return null;

  const now = coerceDate(opts.now);
  const nowMin = now.getHours() * 60 + now.getMinutes();
  const byId = new Map(open.map((p) => [p.id, p]));

  /** @type {{ pickup: object, score: number, window: object|null, reason: string }|null} */
  let best = null;

  for (const slot of schedule || []) {
    const assignedId = slot.assigned?.id;
    if (!assignedId || !byId.has(assignedId)) continue;
    const startMin = parseClockMinutes(slot.start);
    const endMin = parseClockMinutes(slot.end);
    if (startMin == null || endMin == null) continue;

    let score = 0;
    let reason = 'scheduled';
    if (inScheduleWindow(nowMin, startMin, endMin)) {
      score = 10000;
      reason = 'current_window';
    } else if (endMin > startMin && nowMin < startMin) {
      // upcoming same-day
      score = 5000 - (startMin - nowMin);
      reason = 'upcoming_window';
    } else if (endMin <= startMin && nowMin < startMin && nowMin >= endMin) {
      score = 5000 - (startMin - nowMin);
      reason = 'upcoming_window';
    } else {
      // past window or ambiguous — still better than unscheduled if assigned
      score = 1000;
      reason = 'scheduled_open';
    }

    if (!best || score > best.score) {
      best = {
        pickup: byId.get(assignedId),
        score,
        window: { start: slot.start, end: slot.end, label: slot.label || null },
        reason,
      };
    }
  }

  if (best) {
    return decorateNextPhysical(best.pickup, best.window, best.reason);
  }

  // Fallback: highest importance+urgency, stable id tie-break
  const sorted = [...open].sort((a, b) => {
    const sa = Number(a.urgency || 0) + Number(a.importance || 0);
    const sb = Number(b.urgency || 0) + Number(b.importance || 0);
    if (sb !== sa) return sb - sa;
    return String(a.id || '').localeCompare(String(b.id || ''));
  });
  return decorateNextPhysical(sorted[0], null, 'priority');
}

function decorateNextPhysical(pickup, scheduleWindow, reason) {
  return {
    id: pickup.id,
    title: pickup.title || pickup.id,
    area: pickup.area || null,
    claimStatus: pickup.claimStatus || 'open',
    realm: 'physical',
    scheduleWindow: scheduleWindow || null,
    reason,
    commands: {
      claim: `node bin/swarm.js claim ${pickup.id}`,
      complete: `node bin/swarm.js complete ${pickup.id}`,
    },
  };
}

/**
 * Select single primary pending authorization (oldest pending first).
 * @param {Array<object>} pending
 * @returns {object|null}
 */
export function selectNextAuth(pending = []) {
  const open = (pending || []).filter((p) => p && p.status === 'pending');
  if (!open.length) return null;
  const sorted = [...open].sort((a, b) => {
    const ca = String(a.createdAt || '');
    const cb = String(b.createdAt || '');
    if (ca !== cb) return ca.localeCompare(cb);
    return String(a.id || '').localeCompare(String(b.id || ''));
  });
  const a = sorted[0];
  return {
    id: a.id,
    title: a.title || a.id,
    kind: a.kind || 'hitl',
    actionId: a.actionId || null,
    reason: a.reason || null,
    area: a.area || null,
    commands: {
      approve: `node bin/swarm.js approve ${a.id}`,
      deny: `node bin/swarm.js deny ${a.id}`,
    },
  };
}

function coerceDate(now) {
  if (now instanceof Date) return now;
  if (typeof now === 'number') return new Date(now);
  if (typeof now === 'string' && now) return new Date(now);
  return new Date();
}

/**
 * Filter physical pickups: drop completed; annotate claimStatus from snapshot.
 */
export function applyPhysicalSnapshotFilter(physical = [], snapshot = null) {
  const status = physicalStatusMap(snapshot);
  return physical
    .filter((p) => status.get(p.id) !== 'completed')
    .map((p) => ({
      ...p,
      claimStatus: status.get(p.id) || 'open',
    }));
}

/**
 * Build turn view from actions + snapshot (pure).
 * @param {{
 *   actions?: Array<object>,
 *   snapshot?: object|null,
 *   date?: string|null,
 *   schedule?: Array<object>,
 *   now?: Date|string|number,
 * }} input
 */
export function buildTurnSurface({
  actions = [],
  snapshot = null,
  date = null,
  schedule = [],
  now = undefined,
} = {}) {
  const withClass = actions.map((a) => ({
    ...a,
    classification: a.classification || classifyItem(a),
  }));
  const withRealm = enrichWithRealm(withClass);
  let snap = upsertPendingFromActions(withRealm, snapshot, { now: nowIso(now) });
  snap = upsertPhysicalFromActions(withRealm, snap, { now: nowIso(now) });

  const rawPhysical = physicalPickups(withRealm);
  const physical = applyPhysicalSnapshotFilter(rawPhysical, snap);
  const pending = listPending(snap);
  const nextPhysical = selectNextPhysical(physical, schedule, { now });
  const nextAuth = selectNextAuth(pending);

  const turn = {
    date: date || new Date().toISOString().slice(0, 10),
    physicalPickups: physical,
    pendingAuthorizations: pending,
    nextPhysical,
    nextAuth,
    snapshot: snap,
    schedule: schedule || [],
    summary: {
      physicalCount: physical.length,
      pendingCount: pending.length,
      snapshotStatus: snap.status,
      phase: snap.phase,
      hasNextPhysical: Boolean(nextPhysical),
      hasNextAuth: Boolean(nextAuth),
    },
  };
  turn.status = buildTurnStatus(turn);
  return turn;
}

function nowIso(now) {
  if (now === undefined || now === null) return new Date().toISOString();
  return coerceDate(now).toISOString();
}

/**
 * Machine-readable turn/status IR for agents and tooling.
 * @param {object} turn from buildTurnSurface
 */
export function buildTurnStatus(turn) {
  const nextPhysical = turn.nextPhysical || null;
  const nextAuth = turn.nextAuth || null;
  return {
    version: 1,
    date: turn.date,
    snapshotStatus: turn.summary?.snapshotStatus ?? turn.snapshot?.status ?? null,
    phase: turn.summary?.phase ?? turn.snapshot?.phase ?? null,
    next: {
      physical: nextPhysical
        ? {
            id: nextPhysical.id,
            title: nextPhysical.title,
            area: nextPhysical.area,
            claimStatus: nextPhysical.claimStatus,
            scheduleWindow: nextPhysical.scheduleWindow,
            reason: nextPhysical.reason,
            commands: nextPhysical.commands,
          }
        : null,
      authorization: nextAuth
        ? {
            id: nextAuth.id,
            title: nextAuth.title,
            kind: nextAuth.kind,
            actionId: nextAuth.actionId,
            commands: nextAuth.commands,
          }
        : null,
    },
    physical: (turn.physicalPickups || []).map((p) => ({
      id: p.id,
      title: p.title || p.id,
      area: p.area || null,
      claimStatus: p.claimStatus || 'open',
    })),
    pending: (turn.pendingAuthorizations || []).map((a) => ({
      id: a.id,
      title: a.title,
      kind: a.kind,
      actionId: a.actionId || null,
      status: a.status,
    })),
    counts: {
      physical: turn.summary?.physicalCount ?? (turn.physicalPickups || []).length,
      pending: turn.summary?.pendingCount ?? (turn.pendingAuthorizations || []).length,
    },
  };
}

/**
 * Format turn as operator-facing markdown (primary next first).
 */
export function formatTurnMarkdown(turn) {
  const lines = [];
  lines.push(`# Operator turn — ${turn.date}`);
  lines.push('');
  lines.push(
    '> Digital work is automated. You pick up the **physical world** and grant **authorizations**.',
  );
  lines.push('');
  lines.push('## Snapshot');
  lines.push('');
  lines.push(`- **status:** ${turn.summary.snapshotStatus}`);
  lines.push(`- **phase:** ${turn.summary.phase}`);
  lines.push(`- **physical pickups:** ${turn.summary.physicalCount}`);
  lines.push(`- **pending authorizations:** ${turn.summary.pendingCount}`);
  lines.push('');

  lines.push('## Next action (primary)');
  lines.push('');
  if (turn.nextPhysical) {
    const p = turn.nextPhysical;
    lines.push(`### Body — ${p.title}`);
    lines.push('');
    lines.push(`- **id:** \`${p.id}\``);
    lines.push(`- **area:** ${p.area || '—'}`);
    lines.push(`- **claim:** ${p.claimStatus || 'open'}`);
    if (p.scheduleWindow) {
      lines.push(
        `- **window:** ${p.scheduleWindow.start}–${p.scheduleWindow.end}${p.scheduleWindow.label ? ` · ${p.scheduleWindow.label}` : ''}`,
      );
    }
    lines.push(`- **why:** ${p.reason}`);
    lines.push('');
    lines.push('```bash');
    lines.push(p.commands.claim);
    lines.push(p.commands.complete);
    lines.push('```');
    lines.push('');
  } else {
    lines.push('_No open physical pickup — body queue clear._');
    lines.push('');
  }

  if (turn.nextAuth) {
    const a = turn.nextAuth;
    lines.push(`### Auth — ${a.title}`);
    lines.push('');
    lines.push(`- **id:** \`${a.id}\``);
    lines.push(`- **kind:** ${a.kind}`);
    if (a.reason) lines.push(`- **reason:** ${a.reason}`);
    lines.push('');
    lines.push('```bash');
    lines.push(a.commands.approve);
    lines.push(a.commands.deny);
    lines.push('```');
    lines.push('');
  } else {
    lines.push('_No pending authorization — digital path clear of HITL._');
    lines.push('');
  }

  lines.push('## Physical world pickups');
  lines.push('');
  if (!turn.physicalPickups.length) {
    lines.push('_None right now — all queued work is digital or done._');
  } else {
    for (const p of turn.physicalPickups) {
      const claim = p.claimStatus && p.claimStatus !== 'open' ? ` · ${p.claimStatus}` : '';
      lines.push(`- [ ] **${p.title || p.id}** (${p.area || '—'})${claim} · \`${p.id}\``);
    }
  }
  lines.push('');

  lines.push('## Pending authorizations');
  lines.push('');
  if (!turn.pendingAuthorizations.length) {
    lines.push('_None pending — swarm may proceed on cleared digital path._');
  } else {
    for (const a of turn.pendingAuthorizations) {
      lines.push(
        `- **PAUSE** \`${a.id}\` — ${a.title} (${a.kind}) · ${a.reason || ''}`.trim(),
      );
    }
    if (turn.nextAuth) {
      lines.push('');
      lines.push('Resume primary with:');
      lines.push('```bash');
      lines.push(turn.nextAuth.commands.approve);
      lines.push(turn.nextAuth.commands.deny);
      lines.push('```');
    }
  }
  lines.push('');
  lines.push('---');
  lines.push('_Game of Peram · operator turn · frictionless digital_');
  lines.push('');
  return lines.join('\n');
}

/**
 * Resolve paths for durable wait snapshot (private preferred).
 */
export function snapshotPath(root, opts = {}) {
  if (opts.snapshotFile) return path.resolve(opts.snapshotFile);
  return path.join(root, 'private', 'state', 'wait-snapshot.json');
}

/**
 * Durable machine-readable turn status path (sidecar of snapshot).
 */
export function turnStatusPath(root, opts = {}) {
  if (opts.statusFile) return path.resolve(opts.statusFile);
  if (opts.snapshotFile) {
    const dir = path.dirname(path.resolve(opts.snapshotFile));
    return path.join(dir, 'turn-status.json');
  }
  return path.join(root, 'private', 'state', 'turn-status.json');
}

export function loadSnapshot(filePath) {
  if (!fs.existsSync(filePath)) return emptySnapshot();
  return parseSnapshot(fs.readFileSync(filePath, 'utf8'));
}

export function saveSnapshot(filePath, snapshot) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${serializeSnapshot(snapshot)}\n`, 'utf8');
}

export function saveTurnStatus(filePath, status) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(status, null, 2)}\n`, 'utf8');
}

/**
 * Public watch dir (static HTML + status IR for file:// viewing).
 */
export function publicWatchDir(root) {
  return path.join(root, 'public', 'watch');
}

/**
 * Keep public/watch in sync with durable turn status so claim/complete
 * immediately updates the action panel (not only after `graph --html`).
 * @param {string} root
 * @param {object} turn from runOperatorTurn / buildTurnSurface + plan
 * @returns {{ htmlPath: string, statusPath: string, irPath: string }|null}
 */
export function syncPublicWatch(root, turn) {
  if (!turn?.plan || !turn?.status) return null;
  const watchDir = publicWatchDir(root);
  fs.mkdirSync(watchDir, { recursive: true });

  const graph = buildGameGraph({
    date: turn.date,
    card: { phase: turn.summary?.phase, goal: 'Game of Peram day' },
    trail: [
      { phase: 'ORIENT' },
      { phase: 'PLAN' },
      { phase: turn.summary?.phase || 'HITL_WAIT' },
    ],
    actions: turn.plan.actions,
    projects: turn.plan.projects,
    schedule: turn.plan.schedule,
    snapshot: turn.snapshot,
  });
  // Stamp status freshness on graph meta for the HTML header
  graph.meta = {
    ...graph.meta,
    snapshotStatus: turn.summary?.snapshotStatus ?? graph.meta?.snapshotStatus,
    turnStatusAt: turn.snapshot?.updatedAt || new Date().toISOString(),
    nextPhysicalId: turn.nextPhysical?.id || null,
    nextAuthId: turn.nextAuth?.id || null,
  };
  const mermaid = graphToMermaid(graph);
  const htmlPath = path.join(watchDir, 'index.html');
  const irPath = path.join(watchDir, 'graph.json');
  const statusPath = path.join(watchDir, 'turn-status.json');

  fs.writeFileSync(htmlPath, graphToWatchHtml(graph, mermaid, { status: turn.status }), 'utf8');
  fs.writeFileSync(irPath, `${JSON.stringify(graph, null, 2)}\n`, 'utf8');
  fs.writeFileSync(statusPath, `${JSON.stringify(turn.status, null, 2)}\n`, 'utf8');
  // Best-effort life dashboard (status/plan only; full activity merge via `dashboard` CLI)
  const dashPaths = syncPublicDashboard(root, turn);

  return { htmlPath, statusPath, irPath, dashboardHtml: dashPaths?.htmlPath || null };
}

/**
 * Full operator turn from repo state (IO).
 */
export function runOperatorTurn(opts = {}) {
  const root = opts.root || resolveRoot();
  const { persona } = loadPersona(root);
  let state;
  if (opts.fixture) {
    state = loadJson(path.resolve(opts.fixture));
  } else {
    state = loadLocalState(root).state;
  }
  const plan = buildDayPlan(persona, state, { date: opts.date || state.date });
  const snapFile = snapshotPath(root, opts);
  const existing = opts.snapshot || (opts.noLoad ? emptySnapshot() : loadSnapshot(snapFile));
  const turn = buildTurnSurface({
    actions: plan.actions,
    snapshot: existing,
    date: plan.date,
    schedule: plan.schedule,
    now: opts.now,
  });

  turn.plan = plan;

  if (opts.write !== false) {
    saveSnapshot(snapFile, turn.snapshot);
    turn.snapshotPath = snapFile;
    const statusFile = turnStatusPath(root, opts);
    saveTurnStatus(statusFile, turn.status);
    turn.statusPath = statusFile;
    // Always refresh public watch so complete is visible without re-running graph
    if (opts.syncWatch !== false) {
      const watch = syncPublicWatch(root, turn);
      if (watch) turn.watchPaths = watch;
    }
  }

  const markdown = formatTurnMarkdown(turn);
  turn.markdown = markdown;
  return turn;
}

/**
 * Apply approval decision via real entry path.
 */
export function runApprovalDecision(decision, approvalId, opts = {}) {
  const root = opts.root || resolveRoot();
  const snapFile = snapshotPath(root, opts);
  let snap = opts.snapshot || loadSnapshot(snapFile);

  // Ensure pending exists: rebuild from day if empty
  if (!(snap.pending || []).length || opts.refresh) {
    const turn = runOperatorTurn({ ...opts, root, write: false, noLoad: false });
    snap = turn.snapshot;
  }

  const next = applyDecision(snap, approvalId, decision, { actor: opts.actor || 'operator' });
  if (opts.write !== false) {
    saveSnapshot(snapFile, next);
    // Refresh status IR after decision
    const refreshed = runOperatorTurn({ ...opts, root, write: true, snapshot: next });
    return {
      decision,
      approvalId,
      snapshot: refreshed.snapshot,
      snapshotPath: snapFile,
      status: refreshed.snapshot.status,
      phase: refreshed.snapshot.phase,
      pendingRemaining: listPending(refreshed.snapshot),
      turnStatus: refreshed.status,
      statusPath: refreshed.statusPath,
    };
  }
  return {
    decision,
    approvalId,
    snapshot: next,
    snapshotPath: snapFile,
    pendingRemaining: listPending(next),
    status: next.status,
    phase: next.phase,
  };
}

/**
 * Apply physical claim | complete | release via real entry path.
 * Always refreshes public/watch when write is enabled so the next-action
 * panel stops offering a completed id.
 */
export function runPhysicalDecision(decision, actionId, opts = {}) {
  const root = opts.root || resolveRoot();
  const snapFile = snapshotPath(root, opts);
  let snap = opts.snapshot || loadSnapshot(snapFile);

  // Ensure physical rows exist from day plan when possible
  if (!(snap.physical || []).length || opts.refresh) {
    const turn = runOperatorTurn({ ...opts, root, write: false, syncWatch: false });
    snap = turn.snapshot;
  }

  const prior = (snap.physical || []).find((p) => p.id === actionId);
  const alreadyCompleted = prior?.status === 'completed' && decision === 'complete';

  const next = applyPhysicalDecision(snap, actionId, decision, {
    actor: opts.actor || 'operator',
    title: opts.title,
    area: opts.area,
  });

  if (opts.write !== false) {
    saveSnapshot(snapFile, next);
    const refreshed = runOperatorTurn({ ...opts, root, write: true, snapshot: next });
    const row = (refreshed.snapshot.physical || []).find((p) => p.id === actionId);
    return {
      decision,
      actionId,
      ok: true,
      alreadyCompleted: Boolean(alreadyCompleted),
      physicalStatus: row?.status || null,
      leftOpenQueue: !(refreshed.physicalPickups || []).some((p) => p.id === actionId),
      snapshot: refreshed.snapshot,
      snapshotPath: snapFile,
      physical: refreshed.snapshot.physical,
      openPhysical: refreshed.physicalPickups,
      turnStatus: refreshed.status,
      statusPath: refreshed.statusPath,
      watchPaths: refreshed.watchPaths || null,
      next: refreshed.status?.next || null,
    };
  }
  return {
    decision,
    actionId,
    ok: true,
    alreadyCompleted: Boolean(alreadyCompleted),
    snapshot: next,
    snapshotPath: snapFile,
    physical: next.physical,
  };
}

/**
 * Build graph from current day + snapshot for watch.
 */
export function runGraphExport(opts = {}) {
  const root = opts.root || resolveRoot();
  const turn = runOperatorTurn({ ...opts, root, write: opts.write === true });
  const graph = buildGameGraph({
    date: turn.date,
    card: { phase: turn.summary.phase, goal: 'Game of Peram day' },
    trail: [{ phase: 'ORIENT' }, { phase: 'PLAN' }, { phase: turn.summary.phase }],
    actions: turn.plan.actions,
    projects: turn.plan.projects,
    schedule: turn.plan.schedule,
    snapshot: turn.snapshot,
  });
  const mermaid = graphToMermaid(graph);
  return { graph, mermaid, turn };
}
