#!/usr/bin/env node
/**
 * ensembly swarm CLI — Game of Peram control plane
 *
 *   node bin/swarm.js day|turn|…|dashboard|flow|activity|log [options]
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runDailySwarm } from '../src/day.js';
import {
  runOperatorTurn,
  runApprovalDecision,
  runPhysicalDecision,
  runGraphExport,
  snapshotPath,
} from '../src/turn.js';
import { graphToWatchHtml } from '../src/graph.js';
import { openActivityStore } from '../src/activity/index.js';
import { runDashboard } from '../src/dashboard.js';
import {
  runPremflow,
  sharedCaptureStatus,
  ensureLifeOsCaptureLink,
  readSharedCaptureSnapshot,
  projectCaptureForShare,
} from '../src/premflow/index.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');

function parseArgs(argv) {
  const args = {
    cmd: argv[2] || 'day',
    date: null,
    stdout: false,
    json: false,
    write: true,
    fixture: null,
    snapshotFile: null,
    html: false,
    id: null,
    help: false,
    sub: null,
    kind: null,
    message: null,
    actor: 'operator',
    limit: 50,
    dbPath: null,
    positional: [],
  };
  for (let i = 3; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--date') args.date = argv[++i];
    else if (a === '--stdout') args.stdout = true;
    else if (a === '--json') args.json = true;
    else if (a === '--no-write') args.write = false;
    else if (a === '--fixture') args.fixture = argv[++i];
    else if (a === '--snapshot') args.snapshotFile = argv[++i];
    else if (a === '--html') args.html = true;
    else if (a === '--kind') args.kind = argv[++i];
    else if (a === '--message' || a === '-m') args.message = argv[++i];
    else if (a === '--actor') args.actor = argv[++i];
    else if (a === '--limit') args.limit = Number(argv[++i]) || 50;
    else if (a === '--db') args.dbPath = argv[++i];
    else if (a === '--help' || a === '-h') args.help = true;
    else if (
      !a.startsWith('-') &&
      ['approve', 'deny', 'claim', 'complete', 'release'].includes(args.cmd) &&
      !args.id
    ) {
      args.id = a;
    } else if (!a.startsWith('-')) {
      args.positional.push(a);
    }
  }
  if (['activity', 'log'].includes(args.cmd)) {
    args.sub = args.positional[0] || 'list';
    if (args.sub === 'append' && !args.message && args.positional[1]) {
      args.message = args.positional.slice(1).join(' ');
    }
  }
  if (['flow', 'pf', 'premflow'].includes(args.cmd)) {
    args.flowArgs = args.positional.slice();
  }
  return args;
}

function help() {
  console.log(`ensembly — Game of Peram (persona life swarm)

Commands:
  day                 Bounded day cycle → daily plan
  turn                Operator turn: next physical + next auth (+ lists)
  approve <id>        Approve a pending authorization (resume wait snapshot)
  deny <id>           Deny a pending authorization
  claim <id>          Claim a physical pickup (body work in progress)
  complete <id>       Complete a physical pickup (leave open queue)
  release <id>        Release a claimed physical back to open
  graph               Export serializable game graph (+ mermaid / HTML watch)
  dashboard           Life progress: stats, insights, overview → public/watch/dashboard.*
  flow|pf …           Shared micro-capture via premflow (notes/tasks/pomo/review) — one SoT ~/.premflow
  activity list|append   Durable swarm activity audit (SQLite under data/local/) — not the note inbox
  log list|append        Convenience for log.* kinds (same store)

Options:
  --date YYYY-MM-DD   Plan date
  --stdout            Print primary artifact to stdout
  --json              Print machine-readable turn status IR (turn command)
  --no-write          Do not write private/state artifacts
  --fixture <path>    Load state JSON fixture (turn/graph)
  --snapshot <path>   Wait-snapshot JSON path (default private/state/wait-snapshot.json)
  --html              Write public/watch/index.html (graph command)
  --kind <type>       Filter/append kind (activity/log)
  --message|-m <text> Append message payload (activity/log)
  --actor <name>      Actor label (default operator)
  --limit <n>         List limit (default 50)
  --db <path>         Override activity SQLite path

Examples:
  npm run swarm:day
  npm run swarm:turn
  node bin/swarm.js turn --fixture fixtures/state-sample.json --stdout
  node bin/swarm.js turn --fixture fixtures/state-sample.json --json --no-write
  node bin/swarm.js approve auth-apply-high-signal
  node bin/swarm.js claim grocery-errand
  node bin/swarm.js complete grocery-errand
  node bin/swarm.js graph --html
  node bin/swarm.js activity append --kind activity.claim -m "claimed healthy-self-energy"
  node bin/swarm.js log append -m "post-lunch steer"
  node bin/swarm.js activity list --json --limit 20
  node bin/swarm.js dashboard --json
  npm run swarm:dashboard
  node bin/swarm.js flow path          # shared capture root + life-os link
  node bin/swarm.js flow link          # symlink life-os Projects/premflow/capture → ~/.premflow
  node bin/swarm.js flow task list     # wrapper → premflow (same files)
  node bin/swarm.js flow note "idea"
  node bin/swarm.js flow review
`);
}

function runFlowCmd(args) {
  const flowArgs = args.flowArgs || [];
  const head = flowArgs[0] || '';

  if (head === 'path' || head === 'status') {
    // --json defaults to share-safe projection (no raw private lines) for agents/logs
    const st = sharedCaptureStatus({ shareSafe: Boolean(args.json) });
    if (args.json) {
      process.stdout.write(`${JSON.stringify(st, null, 2)}\n`);
    } else {
      console.log(`shared capture SoT (premflow data): ${st.sharedRoot}`);
      console.log(`life-os capture link: ${st.lifeOsCaptureLink} (${st.lifeOsLinkHealthy ? 'OK → same tree' : 'MISSING/broken — run: node bin/swarm.js flow link'})`);
      console.log(`premflow bin: ${st.premflowBin || '(not found)'}`);
      console.log(`privacy: capture is PRIVATE by default — do not commit capture/ or paste raw todos into public IR / Eve`);
      if (st.privacy?.shareableProjection) {
        const p = st.privacy.shareableProjection;
        console.log(
          `counts: todos public=${p.todoPublicCount} private=${p.todoPrivateCount} · log public=${p.logPublicCount} private=${p.logPrivateCount}`,
        );
      }
      console.log(`open todos (local sample up to 5 of ${st.todoCount}):`);
      for (const t of st.sampleTodos) console.log(`  ${t}`);
      console.log(`law: ${st.law}`);
    }
    return;
  }

  if (head === 'link' || head === 'setup') {
    const r = ensureLifeOsCaptureLink();
    if (args.json) {
      process.stdout.write(`${JSON.stringify(r, null, 2)}\n`);
    } else {
      console.log(r.ok ? `LINK_OK action=${r.action}` : `LINK_FAIL ${r.error}`);
      console.log(`link: ${r.linkPath}`);
      console.log(`shared: ${r.sharedRoot}`);
    }
    if (!r.ok) process.exit(1);
    return;
  }

  if (head === 'snapshot' || head === 'show') {
    const snap = readSharedCaptureSnapshot({ todoLimit: args.limit || 50, logLimit: 40 });
    // --json is share-safe (redacted); human TTY gets full local view
    if (args.json) {
      const proj = projectCaptureForShare(snap);
      process.stdout.write(
        `${JSON.stringify({ root: snap.root, ...proj, note: 'share-safe; use bare snapshot for local full text' }, null, 2)}\n`,
      );
    } else {
      console.log(`# Shared capture @ ${snap.root} (LOCAL — may include finance/PII; do not paste to cloud)`);
      console.log(`## todos (${snap.todos.length})`);
      for (const t of snap.todos) console.log(t);
      console.log(`## log tail (${snap.logTail.length})`);
      for (const l of snap.logTail) console.log(l);
      console.log(`## journal files: ${snap.journalFiles.join(', ') || '(none)'}`);
    }
    return;
  }

  // Passthrough to real premflow (empty → help)
  const forward =
    head === 'help' || head === '--help' ? [] : flowArgs;
  const result = runPremflow(forward);
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.error) console.error(result.error);
  if (!result.ok && result.plan?.missingBin) {
    console.error(`Shared data root remains: ${result.plan.dataDir}`);
    console.error('Install premflow or set PREMFLOW_BIN.');
  }
  process.exit(result.status || (result.ok ? 0 : 1));
}

async function runActivityCmd(args) {
  const store = await openActivityStore({
    backend: 'sqlite',
    root,
    dbPath: args.dbPath || undefined,
  });
  try {
    if (args.sub === 'append') {
      const isLog = args.cmd === 'log';
      const kind =
        args.kind ||
        (isLog ? 'log.info' : 'activity');
      const message = args.message || '';
      if (!message && !args.kind) {
        console.error('Usage: node bin/swarm.js activity|log append -m "text" [--kind type]');
        process.exit(1);
      }
      const entry = store.append({
        kind,
        actor: args.actor || 'operator',
        payload: { message, source: 'cli' },
      });
      if (args.json) {
        process.stdout.write(`${JSON.stringify(entry, null, 2)}\n`);
      } else {
        console.log(`APPENDED id=${entry.id} kind=${entry.kind} ts=${entry.ts}`);
        console.log(`store: ${store.path}`);
      }
      return;
    }

    // list (default)
    const listQuery = { limit: args.limit };
    if (args.kind) listQuery.kind = args.kind;
    else if (args.cmd === 'log') listQuery.kinds = ['log.info', 'log.warn', 'log.error', 'log'];
    const rows = store.list(listQuery);
    if (args.json) {
      process.stdout.write(`${JSON.stringify({ path: store.path, count: rows.length, entries: rows }, null, 2)}\n`);
    } else {
      console.log(`activity store: ${store.path}`);
      console.log(`count: ${rows.length}`);
      for (const e of rows) {
        const msg = e.payload?.message != null ? ` · ${e.payload.message}` : '';
        console.log(`${e.ts}  ${e.kind}  ${e.id}${msg}`);
      }
    }
  } finally {
    store.close();
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help || args.cmd === 'help') {
    help();
    process.exit(0);
  }

  if (args.cmd === 'flow' || args.cmd === 'pf' || args.cmd === 'premflow') {
    runFlowCmd(args);
    return;
  }

  if (args.cmd === 'activity' || args.cmd === 'log') {
    await runActivityCmd(args);
    return;
  }

  const common = {
    root,
    date: args.date || undefined,
    write: args.write,
    fixture: args.fixture || undefined,
    snapshotFile: args.snapshotFile || undefined,
  };

  if (args.cmd === 'day') {
    const result = runDailySwarm({
      root,
      date: args.date || undefined,
      write: args.write,
    });
    if (args.stdout || !result.outPath) {
      process.stdout.write(result.artifact || '');
      if (result.artifact && !result.artifact.endsWith('\n')) process.stdout.write('\n');
    } else {
      console.log(`status: ${result.status}`);
      console.log(`phase: ${result.card.phase}`);
      console.log(`wrote: ${result.outPath}`);
      console.log(`actions: ${result.context?.actions?.length ?? 0}`);
      console.log(`balance: ${result.context?.balance}`);
    }
    if (result.status !== 'DONE' && result.status !== 'HITL_PLAN_GATE') process.exit(2);
    return;
  }

  if (args.cmd === 'turn') {
    const turn = runOperatorTurn(common);
    if (args.json) {
      process.stdout.write(`${JSON.stringify(turn.status, null, 2)}\n`);
    } else if (args.stdout || args.write === false) {
      process.stdout.write(turn.markdown);
    } else {
      console.log(turn.markdown);
      if (turn.snapshotPath) console.log(`\nsnapshot: ${turn.snapshotPath}`);
      if (turn.statusPath) console.log(`status: ${turn.statusPath}`);
    }
    // Always print machine summary line for tests / agents on stderr
    const np = turn.nextPhysical?.id || '-';
    const na = turn.nextAuth?.id || '-';
    console.error(
      `TURN_OK physical=${turn.summary.physicalCount} pending=${turn.summary.pendingCount} nextPhysical=${np} nextAuth=${na} status=${turn.summary.snapshotStatus} phase=${turn.summary.phase}`,
    );
    return;
  }

  if (args.cmd === 'approve' || args.cmd === 'deny') {
    if (!args.id) {
      console.error(`Usage: node bin/swarm.js ${args.cmd} <approval-id>`);
      process.exit(1);
    }
    const result = runApprovalDecision(args.cmd === 'approve' ? 'approve' : 'deny', args.id, common);
    console.log(
      JSON.stringify(
        {
          ok: true,
          decision: result.decision,
          id: args.id,
          status: result.status,
          phase: result.phase,
          pendingRemaining: result.pendingRemaining.map((p) => p.id),
          next: result.turnStatus?.next || null,
          snapshotPath: result.snapshotPath,
          statusPath: result.statusPath || null,
        },
        null,
        2,
      ),
    );
    return;
  }

  if (args.cmd === 'claim' || args.cmd === 'complete' || args.cmd === 'release') {
    if (!args.id) {
      console.error(`Usage: node bin/swarm.js ${args.cmd} <physical-action-id>`);
      process.exit(1);
    }
    const result = runPhysicalDecision(args.cmd, args.id, common);
    const nextPhys = result.next?.physical || result.turnStatus?.next?.physical || null;
    console.log(
      JSON.stringify(
        {
          ok: true,
          decision: result.decision,
          id: args.id,
          physicalStatus: result.physicalStatus || null,
          alreadyCompleted: Boolean(result.alreadyCompleted),
          leftOpenQueue: result.leftOpenQueue !== false && args.cmd === 'complete',
          physical: (result.snapshot?.physical || []).map((p) => ({
            id: p.id,
            status: p.status,
          })),
          openPhysicalIds: (result.openPhysical || []).map((p) => p.id),
          next: result.next || result.turnStatus?.next || null,
          snapshotPath: result.snapshotPath,
          statusPath: result.statusPath || null,
          watchHtml: result.watchPaths?.htmlPath || null,
        },
        null,
        2,
      ),
    );
    if (args.cmd === 'complete') {
      if (result.alreadyCompleted) {
        console.error(
          `COMPLETE_ALREADY id=${args.id} (still completed; next body=${nextPhys?.id || '—'})`,
        );
      } else {
        console.error(
          `COMPLETE_OK id=${args.id} leftOpen=${result.leftOpenQueue !== false} nextBody=${nextPhys?.id || '—'} watch=${result.watchPaths?.htmlPath || 'n/a'}`,
        );
      }
    }
    return;
  }

  if (args.cmd === 'graph') {
    const { graph, mermaid, turn } = runGraphExport({ ...common, write: false });
    if (args.html) {
      const htmlPath = path.join(root, 'public', 'watch', 'index.html');
      fs.mkdirSync(path.dirname(htmlPath), { recursive: true });
      fs.writeFileSync(
        htmlPath,
        graphToWatchHtml(graph, mermaid, { status: turn.status }),
        'utf8',
      );
      const irPath = path.join(root, 'public', 'watch', 'graph.json');
      fs.writeFileSync(irPath, `${JSON.stringify(graph, null, 2)}\n`, 'utf8');
      const statusPath = path.join(root, 'public', 'watch', 'turn-status.json');
      fs.writeFileSync(statusPath, `${JSON.stringify(turn.status, null, 2)}\n`, 'utf8');
      console.log(`wrote: ${htmlPath}`);
      console.log(`wrote: ${irPath}`);
      console.log(`wrote: ${statusPath}`);
    }
    if (args.stdout || !args.html) {
      process.stdout.write(`${mermaid}\n`);
      if (!args.stdout) {
        console.error(
          `GRAPH_OK nodes=${graph.meta.nodeCount} edges=${graph.meta.edgeCount} nextPhysical=${turn.nextPhysical?.id || '-'} nextAuth=${turn.nextAuth?.id || '-'}`,
        );
      }
    }
    return;
  }

  if (args.cmd === 'dashboard') {
    const turn = runOperatorTurn({
      ...common,
      // turn may write snapshot; dashboard write is separate
      write: args.write,
    });
    const result = await runDashboard({
      root,
      turn,
      write: args.write,
      dbPath: args.dbPath || undefined,
    });
    if (args.json) {
      process.stdout.write(`${JSON.stringify(result.dashboard, null, 2)}\n`);
    } else if (args.stdout || args.write === false) {
      process.stdout.write(result.markdown);
    } else {
      process.stdout.write(result.markdown);
      if (result.paths?.htmlPath) console.log(`\nwrote: ${result.paths.htmlPath}`);
      if (result.paths?.jsonPath) console.log(`wrote: ${result.paths.jsonPath}`);
    }
    console.error(
      `DASHBOARD_OK version=${result.dashboard.version} insights=${result.dashboard.insights?.length ?? 0} activity=${result.dashboard.stats?.activityTotal ?? 0} nextPhysical=${result.dashboard.next?.physical?.id || '-'} nextAuth=${result.dashboard.next?.authorization?.id || '-'}`,
    );
    return;
  }

  console.error(`Unknown command: ${args.cmd}`);
  help();
  process.exit(1);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
