#!/usr/bin/env node
/**
 * ensembly swarm CLI — Game of Peram control plane
 *
 *   node bin/swarm.js day|turn|approve|deny|graph [options]
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runDailySwarm } from '../src/day.js';
import {
  runOperatorTurn,
  runApprovalDecision,
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
    else if (a === '--no-write') args.write = false;
    else if (a === '--fixture') args.fixture = argv[++i];
    else if (a === '--snapshot') args.snapshotFile = argv[++i];
    else if (a === '--html') args.html = true;
    else if (a === '--help' || a === '-h') args.help = true;
    else if (!a.startsWith('-') && (args.cmd === 'approve' || args.cmd === 'deny') && !args.id) {
      args.id = a;
    }
  }
  return args;
}

function help() {
  console.log(`ensembly — Game of Peram (persona life swarm)

Commands:
  day                 Bounded day cycle → daily plan
  turn                Operator turn: physical pickups + pending authorizations
  approve <id>        Approve a pending authorization (resume wait snapshot)
  deny <id>           Deny a pending authorization
  graph               Export serializable game graph (+ mermaid / HTML watch)

Options:
  --date YYYY-MM-DD   Plan date
  --stdout            Print primary artifact to stdout
  --no-write          Do not write private/state artifacts
  --fixture <path>    Load state JSON fixture (turn/graph)
  --snapshot <path>   Wait-snapshot JSON path (default private/state/wait-snapshot.json)
  --html              Write public/watch/index.html (graph command)

Examples:
  npm run swarm:day
  npm run swarm:turn
  node bin/swarm.js turn --fixture fixtures/state-sample.json --stdout
  node bin/swarm.js approve auth-apply-high-signal
  node bin/swarm.js graph --stdout
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
    if (args.stdout || args.write === false) {
      process.stdout.write(turn.markdown);
    } else {
      console.log(turn.markdown);
      if (turn.snapshotPath) console.log(`\nsnapshot: ${turn.snapshotPath}`);
    }
    // Always print machine summary line for tests
    console.error(
      `TURN_OK physical=${turn.summary.physicalCount} pending=${turn.summary.pendingCount} status=${turn.summary.snapshotStatus} phase=${turn.summary.phase}`,
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
          snapshotPath: result.snapshotPath,
        },
        null,
        2,
      ),
    );
    return;
  }

  if (args.cmd === 'graph') {
    const { graph, mermaid } = runGraphExport({ ...common, write: false });
    if (args.html) {
      const htmlPath = path.join(root, 'public', 'watch', 'index.html');
      fs.mkdirSync(path.dirname(htmlPath), { recursive: true });
      fs.writeFileSync(htmlPath, graphToWatchHtml(graph, mermaid), 'utf8');
      const irPath = path.join(root, 'public', 'watch', 'graph.json');
      fs.writeFileSync(irPath, `${JSON.stringify(graph, null, 2)}\n`, 'utf8');
      console.log(`wrote: ${htmlPath}`);
      console.log(`wrote: ${irPath}`);
    }
    if (args.stdout || !args.html) {
      process.stdout.write(`${mermaid}\n`);
      if (!args.stdout) {
        console.error(
          `GRAPH_OK nodes=${graph.meta.nodeCount} edges=${graph.meta.edgeCount}`,
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
