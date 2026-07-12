#!/usr/bin/env node
/**
 * ensembly swarm CLI
 * Usage:
 *   node bin/swarm.js day [--date YYYY-MM-DD] [--stdout] [--no-write]
 *   npm run swarm:day
 */
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runDailySwarm } from '../src/day.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');

function parseArgs(argv) {
  const args = { cmd: argv[2] || 'day', date: null, stdout: false, write: true };
  for (let i = 3; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--date') args.date = argv[++i];
    else if (a === '--stdout') args.stdout = true;
    else if (a === '--no-write') args.write = false;
    else if (a === '--help' || a === '-h') args.help = true;
  }
  return args;
}

function main() {
  const args = parseArgs(process.argv);
  if (args.help || args.cmd === 'help') {
    console.log(`ensembly swarm — persona-driven daily self-organization

Commands:
  day     Run the bounded day cycle; emit daily swarm plan

Options:
  --date YYYY-MM-DD   Plan date (default: today / state.date)
  --stdout            Print plan markdown to stdout
  --no-write          Do not write private/state/plans/

Examples:
  npm run swarm:day
  node bin/swarm.js day --stdout
  node bin/swarm.js day --date 2026-07-12 --stdout --no-write
`);
    process.exit(0);
  }

  if (args.cmd !== 'day') {
    console.error(`Unknown command: ${args.cmd}. Try: node bin/swarm.js day`);
    process.exit(1);
  }

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
    console.log(`public: ${result.context?.publicActions?.length ?? 0} private: ${result.context?.privateActions?.length ?? 0}`);
  }

  if (result.status !== 'DONE' && result.status !== 'HITL_PLAN_GATE') {
    process.exit(2);
  }
}

main();
