#!/usr/bin/env node
/**
 * Headless-ish smoke: boot static server, fetch game page + modules, drive session pure path.
 */
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawn } from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const scratch =
  process.env.SCRATCH ||
  path.join(process.env.HOME || '', 'tmp/grok-goal-8a121ab209b0/implementer');
fs.mkdirSync(scratch, { recursive: true });

const port = 4177;
const server = spawn(process.execPath, [path.join(root, 'scripts/serve-game.mjs'), String(port)], {
  cwd: root,
  stdio: ['ignore', 'pipe', 'pipe'],
});

function fetchText(url) {
  return new Promise((resolve, reject) => {
    http
      .get(url, (res) => {
        let d = '';
        res.on('data', (c) => (d += c));
        res.on('end', () => resolve({ status: res.statusCode, body: d }));
      })
      .on('error', reject);
  });
}

async function main() {
  await new Promise((r) => setTimeout(r, 400));
  const base = `http://127.0.0.1:${port}`;
  const page = await fetchText(`${base}/game/`);
  const mainJs = await fetchText(`${base}/game/main.js`);
  const css = await fetchText(`${base}/game/styles.css`);
  const sessionJs = await fetchText(`${base}/src/game/session.js`);
  const wasmJs = await fetchText(`${base}/game/pkg/peram_core.js`);
  const log1 = [
    `page status=${page.status} len=${page.body.length}`,
    `has canvas=${/id="stage"/.test(page.body)}`,
    `has hud=${/Game of Peram/.test(page.body)}`,
    `has cockpit=${/cockpit/.test(page.body)}`,
    `has $SPN ticker=${/id="spn-ticker"/.test(page.body) || /\$SPN/.test(page.body)}`,
    `has spn-chart=${/id="spn-chart"/.test(page.body)}`,
    `main.js status=${mainJs.status}`,
    `main paints spn=${/paintSpn|spn-path|\$SPN/.test(mainJs.body)}`,
    `styles.css status=${css.status}`,
    `css has spn-ticker=${/\.spn-ticker/.test(css.body)}`,
    `session.js status=${sessionJs.status}`,
    `wasm js status=${wasmJs.status}`,
  ].join('\n');
  fs.writeFileSync(path.join(scratch, 'game-launch-1.txt'), `${log1}\n`);

  // Drive pure session (same as browser) via dynamic import
  const mod = await import(pathToFileURL(path.join(root, 'src/game/index.js')).href);
  const graph = JSON.parse(
    fs.readFileSync(path.join(root, 'public/game/sample-graph.json'), 'utf8'),
  );
  let s = mod.createSession(graph, { pending: graph.pending });
  const before = mod.sessionView(s);
  s = mod.dispatch(s, mod.mapKeyEvent({ key: 'Tab' }));
  s = mod.dispatch(s, mod.mapKeyEvent({ key: 'a' }));
  // Claim a physical if present to move $SPN
  const physIdx = (s.nodes || []).findIndex(
    (n) => n.type === 'physical' || n.realm === 'physical',
  );
  if (physIdx >= 0) {
    s = mod.dispatch(s, { type: 'FOCUS_INDEX', payload: { index: physIdx } });
    s = mod.dispatch(s, { type: 'COMPLETE' });
  }
  const after = mod.sessionView(s);
  const spnBefore = before.growth?.spn;
  const spnAfter = after.growth?.spn;
  const log2 = JSON.stringify(
    {
      before: { focusIndex: before.focusIndex, pendingOpen: before.pendingOpen, spn: spnBefore },
      after: { focusIndex: after.focusIndex, pendingOpen: after.pendingOpen, spn: spnAfter },
      changed:
        before.focusIndex !== after.focusIndex ||
        before.pendingOpen !== after.pendingOpen ||
        (spnBefore?.price !== spnAfter?.price),
      spnMovedUp:
        spnAfter && spnBefore ? spnAfter.price >= spnBefore.price : null,
    },
    null,
    2,
  );
  fs.writeFileSync(path.join(scratch, 'game-launch-2.txt'), `${log2}\n`);

  // /game without slash should redirect or still work
  const page2 = await fetchText(`${base}/game`);
  fs.appendFileSync(
    path.join(scratch, 'game-launch-1.txt'),
    `\nlaunch2 /game status=${page2.status}\n`,
  );

  if (
    page.status !== 200 ||
    mainJs.status !== 200 ||
    css.status !== 200 ||
    sessionJs.status !== 200
  ) {
    throw new Error('game assets failed to serve');
  }
  if (!(before.focusIndex !== after.focusIndex || before.pendingOpen !== after.pendingOpen)) {
    throw new Error('session did not change after key actions');
  }
  console.log('SMOKE_OK', { page: page.status, focus: after.focusIndex, pending: after.pendingOpen });
}

main()
  .catch((e) => {
    console.error(e);
    process.exitCode = 1;
  })
  .finally(() => {
    server.kill('SIGTERM');
  });
