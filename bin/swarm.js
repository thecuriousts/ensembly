#!/usr/bin/env node
/**
 * ensembly swarm CLI — Game of Peram control plane
 *
 *   node bin/swarm.js day|turn|approve|deny|claim|complete|graph [options]
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
    else if (a === '--help' || a === '-h') args.help = true;
    else if (
      !a.startsWith('-') &&
      ['approve', 'deny', 'claim', 'complete', 'release'].includes(args.cmd) &&
      !args.id
    ) {
      args.id = a;
    }
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

Options:
  --date YYYY-MM-DD   Plan date
  --stdout            Print primary artifact to stdout
  --json              Print machine-readable turn status IR (turn command)
  --no-write          Do not write private/state artifacts
  --fixture <path>    Load state JSON fixture (turn/graph)
  --snapshot <path>   Wait-snapshot JSON path (default private/state/wait-snapshot.json)
  --html              Write public/watch/index.html (graph command)

Examples:
  npm run swarm:day
  npm run swarm:turn
  node bin/swarm.js turn --fixture fixtures/state-sample.json --stdout
  node bin/swarm.js turn --fixture fixtures/state-sample.json --json --no-write
  node bin/swarm.js approve auth-apply-high-signal
  node bin/swarm.js claim grocery-errand
  node bin/swarm.js complete grocery-errand
  node bin/swarm.js graph --html
`);
}

function main() {
  const args = parseArgs(process.argv);
  if (args.help || args.cmd === 'help') {
    help();
    process.exit(0);
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

  console.error(`Unknown command: ${args.cmd}`);
  help();
  process.exit(1);
}

main();
