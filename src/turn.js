/**
 * Operator turn surface: physical pickups + pending authorizations.
 */
import fs from 'node:fs';
import path from 'node:path';
import { physicalPickups, enrichWithRealm } from './realm.js';
import {
  upsertPendingFromActions,
  applyDecision,
  listPending,
  emptySnapshot,
  parseSnapshot,
  serializeSnapshot,
} from './approvals.js';
import { classifyItem } from './privacy.js';
import { buildDayPlan } from './day.js';
import { loadPersona, loadLocalState, resolveRoot, loadJson } from './ingest.js';
import { buildGameGraph, graphToMermaid } from './graph.js';

/**
 * Build turn view from actions + snapshot (pure).
 */
export function buildTurnSurface({ actions = [], snapshot = null, date = null, schedule = [] } = {}) {
  const withClass = actions.map((a) => ({
    ...a,
    classification: a.classification || classifyItem(a),
  }));
  const withRealm = enrichWithRealm(withClass);
  const physical = physicalPickups(withRealm);
  const snap = upsertPendingFromActions(withRealm, snapshot);
  const pending = listPending(snap);

  return {
    date: date || new Date().toISOString().slice(0, 10),
    physicalPickups: physical,
    pendingAuthorizations: pending,
    snapshot: snap,
    schedule: schedule || [],
    summary: {
      physicalCount: physical.length,
      pendingCount: pending.length,
      snapshotStatus: snap.status,
      phase: snap.phase,
    },
  };
}

/**
 * Format turn as operator-facing markdown.
 */
export function formatTurnMarkdown(turn) {
  const lines = [];
  lines.push(`# Operator turn — ${turn.date}`);
  lines.push('');
  lines.push('> Digital work is automated. You pick up the **physical world** and grant **authorizations**.');
  lines.push('');
  lines.push('## Snapshot');
  lines.push('');
  lines.push(`- **status:** ${turn.summary.snapshotStatus}`);
  lines.push(`- **phase:** ${turn.summary.phase}`);
  lines.push(`- **physical pickups:** ${turn.summary.physicalCount}`);
  lines.push(`- **pending authorizations:** ${turn.summary.pendingCount}`);
  lines.push('');

  lines.push('## Physical world pickups');
  lines.push('');
  if (!turn.physicalPickups.length) {
    lines.push('_None right now — all queued work is digital or done._');
  } else {
    for (const p of turn.physicalPickups) {
      lines.push(`- [ ] **${p.title || p.id}** (${p.area || '—'}) · \`${p.id}\``);
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
    lines.push('');
    lines.push('Resume with:');
    lines.push('```bash');
    lines.push(`node bin/swarm.js approve ${turn.pendingAuthorizations[0].id}`);
    lines.push(`node bin/swarm.js deny ${turn.pendingAuthorizations[0].id}`);
    lines.push('```');
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

export function loadSnapshot(filePath) {
  if (!fs.existsSync(filePath)) return emptySnapshot();
  return parseSnapshot(fs.readFileSync(filePath, 'utf8'));
}

export function saveSnapshot(filePath, snapshot) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${serializeSnapshot(snapshot)}\n`, 'utf8');
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
  });

  if (opts.write !== false) {
    saveSnapshot(snapFile, turn.snapshot);
    turn.snapshotPath = snapFile;
  }

  const markdown = formatTurnMarkdown(turn);
  turn.markdown = markdown;
  turn.plan = plan;
  return turn;
}

/**
 * Apply decision via real entry path.
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
