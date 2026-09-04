/**
 * Life progress dashboard — pure Dashboard IR v1 + projectors + optional IO runner.
 * Glance host (watch-family), not the game product center.
 *
 * Inputs: turn status IR, wait snapshot, optional day plan, activity entries.
 * Insights are rule-based with explicit evidence (no LLM theater).
 */
import fs from 'node:fs';
import path from 'node:path';
import { balanceScore, countByArea } from './balance.js';
import { resolveRoot } from './ingest.js';
import { openActivityStore } from './activity/index.js';

export const DASHBOARD_IR_VERSION = 1;

/**
 * @typedef {object} DashboardInsight
 * @property {string} id
 * @property {'info'|'steer'|'warn'|'ok'} severity
 * @property {string} message
 * @property {object} evidence
 * @property {string|null} [steer]
 */

/**
 * Build Dashboard IR from kernel inputs (pure).
 *
 * @param {{
 *   status?: object|null,
 *   snapshot?: object|null,
 *   plan?: object|null,
 *   activities?: object[],
 *   now?: string|Date,
 *   generatedAt?: string,
 * }} [input]
 */
export function buildDashboard(input = {}) {
  const now = coerceDate(input.now ?? input.generatedAt ?? new Date());
  const nowIso = now.toISOString();
  const status = input.status || null;
  const snapshot = input.snapshot || null;
  const plan = input.plan || null;
  const activities = Array.isArray(input.activities) ? input.activities : [];

  const physicalRows = snapshot?.physical || status?.physical || [];
  const pendingRows =
    (snapshot?.pending || []).filter((p) => p.status === 'pending') ||
    status?.pending ||
    [];
  // status.pending is already list of pending-ish summaries
  const pendingList = status?.pending?.length
    ? status.pending
    : (snapshot?.pending || []).filter((p) => p.status === 'pending');

  const physOpen = physicalRows.filter((p) => (p.claimStatus || p.status || 'open') === 'open');
  const physClaimed = physicalRows.filter(
    (p) => (p.claimStatus || p.status) === 'claimed',
  );
  const physCompleted = physicalRows.filter(
    (p) => (p.claimStatus || p.status) === 'completed',
  );

  const planActions = plan?.actions || [];
  const bal =
    planActions.length > 0
      ? balanceScore(planActions)
      : status?.counts
        ? null
        : null;
  const areas = planActions.length ? countByArea(planActions) : {};

  const byKind = countActivityByKind(activities);
  const recent = activities
    .slice()
    .sort((a, b) => String(a.ts).localeCompare(String(b.ts)))
    .slice(-20)
    .reverse()
    .map((e) => ({
      id: e.id,
      ts: e.ts,
      kind: e.kind,
      actor: e.actor ?? null,
      message: e.payload?.message ?? null,
    }));

  const since24 = new Date(now.getTime() - 24 * 3600 * 1000).toISOString();
  const since7d = new Date(now.getTime() - 7 * 24 * 3600 * 1000).toISOString();
  const last24 = activities.filter((e) => e.ts && e.ts >= since24).length;
  const last7d = activities.filter((e) => e.ts && e.ts >= since7d).length;

  const next = status?.next || {
    physical: null,
    authorization: null,
  };

  const overview = {
    date: status?.date || plan?.date || nowIso.slice(0, 10),
    phase: status?.phase ?? snapshot?.phase ?? null,
    snapshotStatus: status?.snapshotStatus ?? snapshot?.status ?? null,
    physicalOpen: physOpen.length || status?.counts?.physical || 0,
    physicalClaimed: physClaimed.length,
    physicalCompleted: physCompleted.length,
    pendingAuth: pendingList.length || status?.counts?.pending || 0,
    balanceScore: bal,
    historyLen: Array.isArray(snapshot?.history) ? snapshot.history.length : 0,
  };

  // Prefer explicit open counts from status.physical when snapshot thin
  if (status?.counts) {
    if (overview.physicalOpen === 0 && status.counts.physical) {
      overview.physicalOpen = status.counts.physical;
    }
    if (overview.pendingAuth === 0 && status.counts.pending) {
      overview.pendingAuth = status.counts.pending;
    }
  }

  const stats = {
    activityTotal: activities.length,
    activityLast24h: last24,
    activityLast7d: last7d,
    activityByKind: byKind,
    scheduleBlocks: Array.isArray(plan?.schedule) ? plan.schedule.length : 0,
    areaCounts: areas,
    graphHint: null,
  };

  const activity = {
    recent,
    empty: activities.length === 0,
  };

  const sources = {
    turnStatus: Boolean(status),
    snapshot: Boolean(snapshot),
    plan: Boolean(plan),
    activityStore: activities.length > 0 || input.activityStoreOpen === true,
    collabFinder: false, // day-0 deferred read-only
  };

  const base = {
    version: DASHBOARD_IR_VERSION,
    generatedAt: nowIso,
    overview,
    next: {
      physical: next.physical || null,
      authorization: next.authorization || null,
    },
    stats,
    activity,
    sources,
    insights: [],
  };

  base.insights = deriveInsights(base, { activities, planActions, now });
  return base;
}

/**
 * Rule-based insights with explicit evidence.
 * @param {object} dash partial dashboard
 * @param {{ activities?: object[], planActions?: object[], now?: Date }} [ctx]
 * @returns {DashboardInsight[]}
 */
export function deriveInsights(dash, ctx = {}) {
  /** @type {DashboardInsight[]} */
  const out = [];
  const overview = dash.overview || {};
  const next = dash.next || {};
  const activities = ctx.activities || [];
  const planActions = ctx.planActions || [];
  const byKind = dash.stats?.activityByKind || {};

  const pending = overview.pendingAuth || 0;
  const physOpen = overview.physicalOpen || 0;
  const auth = next.authorization;
  const phys = next.physical;

  if (pending > 0 && auth) {
    out.push({
      id: 'clear_auth_gate',
      severity: 'steer',
      message: 'Clear the open authorization gate before more digital thrash.',
      evidence: {
        pendingAuth: pending,
        nextAuthId: auth.id,
        kind: auth.kind || null,
      },
      steer:
        (auth.commands && (auth.commands.approve || auth.commands.deny)) ||
        `node bin/swarm.js approve ${auth.id}`,
    });
  }

  const claimKinds = countKindsMatching(byKind, (k) => /claim/i.test(k));
  if (physOpen > 0 && phys && claimKinds === 0) {
    out.push({
      id: 'body_work_open',
      severity: 'steer',
      message: 'Body work is still open — claim the next physical pickup.',
      evidence: {
        physicalOpen: physOpen,
        nextPhysicalId: phys.id,
        claimEvents: claimKinds,
      },
      steer:
        (phys.commands && phys.commands.claim) ||
        `node bin/swarm.js claim ${phys.id}`,
    });
  }

  const bal = overview.balanceScore;
  const areas = dash.stats?.areaCounts || {};
  const health = areas.Health || 0;
  const rel = areas.Relationships || 0;
  if (planActions.length > 0 && (bal != null && bal < 0.55 || health === 0 || rel === 0)) {
    out.push({
      id: 'balance_risk',
      severity: 'warn',
      message: 'Balance risk — health and/or relationships coverage is thin on the plan.',
      evidence: {
        balanceScore: bal,
        health,
        relationships: rel,
        planActionCount: planActions.length,
      },
      steer: 'npm run swarm:day  # rebalance; prefer body/presence claims',
    });
  }

  const physActs = countKindsMatching(byKind, (k) => /physical|claim|complete|body/i.test(k));
  const craftish = countKindsMatching(byKind, (k) => /craft|digital|approve|deny/i.test(k));
  if (activities.length >= 3 && craftish >= 2 && physActs === 0) {
    out.push({
      id: 'craft_heavy',
      severity: 'warn',
      message: 'Activity log is craft/auth-heavy with no body claims — pair a physical act.',
      evidence: { craftish, physicalActivity: physActs, activityTotal: activities.length },
      steer: phys
        ? phys.commands?.claim || `node bin/swarm.js claim ${phys.id}`
        : 'node bin/swarm.js turn --stdout',
    });
  }

  if (physOpen === 0 && pending === 0) {
    out.push({
      id: 'queues_clear',
      severity: 'ok',
      message: 'Physical and auth queues look clear — re-run day for a new plan or enjoy capacity.',
      evidence: { physicalOpen: physOpen, pendingAuth: pending },
      steer: 'npm run swarm:day && npm run swarm:turn',
    });
  }

  if (activities.length === 0) {
    out.push({
      id: 'activity_empty',
      severity: 'info',
      message:
        'No durable activity log yet — claim/complete/approve or append to seed the store.',
      evidence: { activityTotal: 0 },
      steer:
        'node bin/swarm.js activity append -m "seed" · or claim/complete body work',
    });
  }

  // Prefer actionable first
  const order = { steer: 0, warn: 1, info: 2, ok: 3 };
  out.sort((a, b) => (order[a.severity] ?? 9) - (order[b.severity] ?? 9));
  return out;
}

/**
 * Markdown for CLI stdout.
 * @param {object} dash
 */
export function formatDashboardMarkdown(dash) {
  const o = dash.overview || {};
  const lines = [];
  lines.push(`# Life progress dashboard — ${o.date || 'today'}`);
  lines.push('');
  lines.push(
    `> phase **${o.phase || '—'}** · snapshot **${o.snapshotStatus || '—'}** · balance **${o.balanceScore ?? 'n/a'}**`,
  );
  lines.push('');
  lines.push('## Overview');
  lines.push(
    `- physical open: **${o.physicalOpen ?? 0}** · claimed: **${o.physicalClaimed ?? 0}** · completed: **${o.physicalCompleted ?? 0}**`,
  );
  lines.push(`- pending auth: **${o.pendingAuth ?? 0}** · history: **${o.historyLen ?? 0}**`);
  lines.push(
    `- activity events: **${dash.stats?.activityTotal ?? 0}** (24h: ${dash.stats?.activityLast24h ?? 0} · 7d: ${dash.stats?.activityLast7d ?? 0})`,
  );
  lines.push('');
  lines.push('## Next action');
  const phys = dash.next?.physical;
  const auth = dash.next?.authorization;
  if (phys) {
    lines.push(`### Body — ${phys.title || phys.id}`);
    lines.push(`- id: \`${phys.id}\` · ${phys.area || '—'} · ${phys.claimStatus || 'open'}`);
    if (phys.commands?.claim) lines.push(`- \`${phys.commands.claim}\``);
  } else {
    lines.push('### Body — queue clear');
  }
  if (auth) {
    lines.push(`### Auth — ${auth.title || auth.id}`);
    lines.push(`- id: \`${auth.id}\` · ${auth.kind || 'hitl'}`);
    if (auth.commands?.approve) lines.push(`- \`${auth.commands.approve}\``);
  } else {
    lines.push('### Auth — no pending HITL');
  }
  lines.push('');
  lines.push('## Insights');
  if (!dash.insights?.length) {
    lines.push('_No insights — feed turn status + activity._');
  } else {
    for (const ins of dash.insights) {
      lines.push(`- **[${ins.severity}]** ${ins.message}`);
      if (ins.steer) lines.push(`  - steer: \`${ins.steer}\``);
    }
  }
  lines.push('');
  lines.push('## Recent activity');
  if (dash.activity?.empty) {
    lines.push('_Empty — `node bin/swarm.js activity list`_');
  } else {
    for (const e of dash.activity?.recent || []) {
      const msg = e.message ? ` · ${e.message}` : '';
      lines.push(`- ${e.ts} · \`${e.kind}\`${msg}`);
    }
  }
  lines.push('');
  lines.push('---');
  lines.push('_ensembly dashboard IR v' + (dash.version || 1) + ' · glance host · not the game_');
  return lines.join('\n') + '\n';
}

/**
 * Static HTML projector (watch-family styling).
 * @param {object} dash
 */
export function dashboardToHtml(dash) {
  const o = dash.overview || {};
  const phys = dash.next?.physical;
  const auth = dash.next?.authorization;
  const insights = dash.insights || [];
  const recent = dash.activity?.recent || [];

  const physCard = phys
    ? `<div class="card">
        <h3>Body — next physical</h3>
        <p><strong>${esc(phys.title)}</strong></p>
        <p>id: <code>${esc(phys.id)}</code> · ${esc(phys.area || '—')} · ${esc(phys.claimStatus || 'open')}</p>
        <pre>${esc((phys.commands && phys.commands.claim) || '')}\n${esc((phys.commands && phys.commands.complete) || '')}</pre>
      </div>`
    : `<div class="card"><h3>Body — next physical</h3><p class="empty">Queue clear — run <code>npm run swarm:turn</code></p></div>`;

  const authCard = auth
    ? `<div class="card auth">
        <h3>Auth — next authorization</h3>
        <p><strong>${esc(auth.title)}</strong></p>
        <p>id: <code>${esc(auth.id)}</code> · ${esc(auth.kind || 'hitl')}</p>
        <pre>${esc((auth.commands && auth.commands.approve) || '')}\n${esc((auth.commands && auth.commands.deny) || '')}</pre>
      </div>`
    : `<div class="card auth"><h3>Auth — next authorization</h3><p class="empty">No pending HITL</p></div>`;

  const insightHtml = insights.length
    ? insights
        .map(
          (ins) =>
            `<li class="ins ${esc(ins.severity)}"><strong>[${esc(ins.severity)}]</strong> ${esc(ins.message)}${
              ins.steer ? `<div class="steer"><code>${esc(ins.steer)}</code></div>` : ''
            }<div class="ev">evidence: <code>${esc(JSON.stringify(ins.evidence))}</code></div></li>`,
        )
        .join('\n')
    : '<li class="empty">No insights yet</li>';

  const recentHtml = recent.length
    ? recent
        .map(
          (e) =>
            `<li><span class="ts">${esc(e.ts)}</span> <code>${esc(e.kind)}</code>${
              e.message ? ` — ${esc(e.message)}` : ''
            }</li>`,
        )
        .join('\n')
    : '<li class="empty">No durable activity — seed with claim/complete or <code>activity append</code></li>';

  const kindRows = Object.entries(dash.stats?.activityByKind || {})
    .map(([k, v]) => `<tr><td>${esc(k)}</td><td>${v}</td></tr>`)
    .join('') || '<tr><td colspan="2" class="empty">none</td></tr>';

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Game of Peram — life dashboard</title>
  <style>
    :root {
      --bg: #0b1020; --panel: #141a2e; --card: #0f1528; --line: #2a3555;
      --text: #e8ecf4; --muted: #9aa3b8; --accent: #9ecbff; --auth: #ffb4a2;
      --ok: #8fd9a8; --warn: #ffd48a; --steer: #9ecbff;
    }
    * { box-sizing: border-box; }
    body {
      font-family: "Segoe UI", system-ui, sans-serif;
      margin: 0; padding: 1.25rem 1.5rem 2.5rem;
      background: radial-gradient(1200px 600px at 10% -10%, #1a2450 0%, var(--bg) 55%);
      color: var(--text); line-height: 1.45;
    }
    h1 { font-size: 1.35rem; margin: 0 0 0.35rem; letter-spacing: 0.02em; }
    h2 { font-size: 1.05rem; margin: 0 0 0.75rem; color: var(--accent); }
    .meta { color: var(--muted); font-size: 0.9rem; margin-bottom: 1rem; }
    .links a { color: var(--accent); margin-right: 0.85rem; font-size: 0.85rem; }
    .grid { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); }
    .panel {
      background: var(--panel); border: 1px solid var(--line);
      border-radius: 14px; padding: 1rem 1.1rem; margin-top: 1rem;
    }
    .stat {
      background: var(--card); border: 1px solid var(--line);
      border-radius: 12px; padding: 0.75rem 0.9rem;
    }
    .stat .n { font-size: 1.55rem; font-weight: 650; }
    .stat .l { font-size: 0.78rem; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }
    .next { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); }
    .card {
      background: var(--card); border: 1px solid var(--line);
      border-radius: 12px; padding: 0.85rem 1rem;
    }
    .card h3 { margin: 0 0 0.4rem; font-size: 0.95rem; color: var(--accent); }
    .card.auth h3 { color: var(--auth); }
    .card p { margin: 0.25rem 0; font-size: 0.9rem; }
    .card pre, pre { font-size: 0.78rem; background: #080c18; padding: 0.55rem 0.7rem; border-radius: 8px; overflow: auto; }
    .empty { opacity: 0.65; font-style: italic; }
    ul.ins { list-style: none; padding: 0; margin: 0; }
    ul.ins li {
      border-left: 3px solid var(--line); padding: 0.55rem 0.75rem; margin-bottom: 0.5rem;
      background: var(--card); border-radius: 0 10px 10px 0;
    }
    ul.ins li.steer { border-left-color: var(--steer); }
    ul.ins li.warn { border-left-color: var(--warn); }
    ul.ins li.ok { border-left-color: var(--ok); }
    ul.ins li.info { border-left-color: var(--muted); }
    .steer { margin-top: 0.35rem; font-size: 0.82rem; }
    .ev { margin-top: 0.25rem; font-size: 0.72rem; color: var(--muted); }
    table { width: 100%; border-collapse: collapse; font-size: 0.88rem; }
    td, th { text-align: left; padding: 0.35rem 0.4rem; border-bottom: 1px solid var(--line); }
    ul.tl { list-style: none; padding: 0; margin: 0; font-size: 0.88rem; }
    ul.tl .ts { color: var(--muted); font-size: 0.8rem; margin-right: 0.35rem; }
    footer { margin-top: 1.5rem; font-size: 0.78rem; color: var(--muted); }
  </style>
</head>
<body>
  <h1>Game of Peram — life progress</h1>
  <p class="meta">
    ${esc(o.date || '—')} · phase <strong>${esc(o.phase || '—')}</strong>
    · snapshot <strong>${esc(o.snapshotStatus || '—')}</strong>
    · balance <strong>${esc(o.balanceScore ?? 'n/a')}</strong>
    · generated ${esc(dash.generatedAt || '')}
  </p>
  <p class="links">
    <a href="./index.html">Watch map</a>
    <a href="http://127.0.0.1:4173/game/">Play game</a>
    <span>CLI: <code>npm run swarm:turn</code> · <code>npm run swarm:dashboard</code></span>
  </p>

  <div class="panel">
    <h2>Overview</h2>
    <div class="grid">
      <div class="stat"><div class="n">${o.physicalOpen ?? 0}</div><div class="l">Physical open</div></div>
      <div class="stat"><div class="n">${o.pendingAuth ?? 0}</div><div class="l">Pending auth</div></div>
      <div class="stat"><div class="n">${o.physicalClaimed ?? 0}</div><div class="l">Claimed</div></div>
      <div class="stat"><div class="n">${o.physicalCompleted ?? 0}</div><div class="l">Completed</div></div>
      <div class="stat"><div class="n">${dash.stats?.activityTotal ?? 0}</div><div class="l">Activity events</div></div>
      <div class="stat"><div class="n">${dash.stats?.activityLast24h ?? 0}</div><div class="l">Last 24h</div></div>
    </div>
  </div>

  <div class="panel">
    <h2>Next action</h2>
    <div class="next">${physCard}${authCard}</div>
  </div>

  <div class="panel">
    <h2>Intelligent insights</h2>
    <ul class="ins">${insightHtml}</ul>
  </div>

  <div class="panel">
    <h2>Stats</h2>
    <p class="meta">Activity by kind · schedule blocks: ${dash.stats?.scheduleBlocks ?? 0}</p>
    <table>
      <thead><tr><th>Kind</th><th>Count</th></tr></thead>
      <tbody>${kindRows}</tbody>
    </table>
  </div>

  <div class="panel">
    <h2>Recent activity</h2>
    <ul class="tl">${recentHtml}</ul>
  </div>

  <details class="panel">
    <summary>Dashboard IR (debug · agents)</summary>
    <p class="meta">Machine-readable export: <code>public/watch/dashboard.json</code></p>
    <pre>${esc(JSON.stringify(dash, null, 2))}</pre>
  </details>

  <footer>
    ensembly dashboard IR v${dash.version || 1} · glance host over pure kernel IR ·
    collab-finder read-only deferred · not a SPA · Game of Peram world remains primary
  </footer>
</body>
</html>
`;
}

function countActivityByKind(entries = []) {
  const m = {};
  for (const e of entries) {
    const k = e.kind || 'unknown';
    m[k] = (m[k] || 0) + 1;
  }
  return m;
}

function countKindsMatching(byKind, pred) {
  let n = 0;
  for (const [k, v] of Object.entries(byKind || {})) {
    if (pred(k)) n += Number(v) || 0;
  }
  return n;
}

function coerceDate(v) {
  if (v instanceof Date) return v;
  if (typeof v === 'number') return new Date(v);
  if (typeof v === 'string' && v) {
    const d = new Date(v);
    if (!Number.isNaN(d.getTime())) return d;
  }
  return new Date();
}

function esc(s) {
  return String(s ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/**
 * IO: build dashboard from turn result + activity store; optional write under public/watch.
 *
 * @param {{
 *   root?: string,
 *   turn?: object,
 *   status?: object,
 *   snapshot?: object,
 *   plan?: object,
 *   write?: boolean,
 *   activities?: object[],
 *   skipActivityStore?: boolean,
 *   dbPath?: string,
 *   now?: string|Date,
 * }} [opts]
 */
export async function runDashboard(opts = {}) {
  const root = opts.root || resolveRoot();
  const turn = opts.turn || null;
  const status = opts.status || turn?.status || null;
  const snapshot = opts.snapshot || turn?.snapshot || null;
  const plan = opts.plan || turn?.plan || null;

  let activities = Array.isArray(opts.activities) ? opts.activities : null;
  let activityStoreOpen = false;
  if (activities == null && !opts.skipActivityStore) {
    try {
      const store = await openActivityStore({
        backend: 'sqlite',
        root,
        dbPath: opts.dbPath,
      });
      activityStoreOpen = true;
      activities = store.list({ limit: 500 });
      store.close();
    } catch {
      activities = [];
    }
  }
  if (activities == null) activities = [];

  const dash = buildDashboard({
    status,
    snapshot,
    plan,
    activities,
    activityStoreOpen,
    now: opts.now,
  });

  const markdown = formatDashboardMarkdown(dash);
  const html = dashboardToHtml(dash);
  let paths = null;

  if (opts.write !== false) {
    const watchDir = path.join(root, 'public', 'watch');
    fs.mkdirSync(watchDir, { recursive: true });
    const htmlPath = path.join(watchDir, 'dashboard.html');
    const jsonPath = path.join(watchDir, 'dashboard.json');
    fs.writeFileSync(htmlPath, html, 'utf8');
    fs.writeFileSync(jsonPath, `${JSON.stringify(dash, null, 2)}\n`, 'utf8');
    paths = { htmlPath, jsonPath };
  }

  return { dashboard: dash, markdown, html, paths };
}

/**
 * Best-effort dashboard refresh from an existing turn object (syncPublicWatch hook).
 * Never throws into the turn path.
 */
export function syncPublicDashboard(root, turn) {
  if (!turn?.status) return null;
  const watchDir = path.join(root, 'public', 'watch');
  try {
    fs.mkdirSync(watchDir, { recursive: true });
    // Sync path: no activity store await — pure status/snapshot/plan only
    const dash = buildDashboard({
      status: turn.status,
      snapshot: turn.snapshot,
      plan: turn.plan,
      activities: [],
      activityStoreOpen: false,
    });
    const htmlPath = path.join(watchDir, 'dashboard.html');
    const jsonPath = path.join(watchDir, 'dashboard.json');
    fs.writeFileSync(htmlPath, dashboardToHtml(dash), 'utf8');
    fs.writeFileSync(jsonPath, `${JSON.stringify(dash, null, 2)}\n`, 'utf8');
    return { htmlPath, jsonPath };
  } catch {
    return null;
  }
}
