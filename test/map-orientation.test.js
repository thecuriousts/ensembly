/**
 * Orientation contract: docs/MAP.md must stay honest against shipped tree + CLI + IR.
 * Drives real files and real buildTurnStatus — not a reimplementation of the map.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { buildTurnStatus, buildTurnSurface } from '../src/turn.js';
import { emptySnapshot } from '../src/approvals.js';
import { buildGameGraph } from '../src/graph.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const mapPath = path.join(root, 'docs', 'MAP.md');

function read(rel) {
  return fs.readFileSync(path.join(root, rel), 'utf8');
}

describe('docs/MAP orientation (shipped truth)', () => {
  it('MAP.md exists with capabilities, hosts, layers, and IR sections', () => {
    assert.ok(fs.existsSync(mapPath), 'docs/MAP.md must exist');
    const map = read('docs/MAP.md');

    // Section headings the plan requires (coverage gate)
    assert.match(map, /## 1\. Live capabilities/i);
    assert.match(map, /## 2\. Host surfaces/i);
    assert.match(map, /## 3\. Layer ownership/i);
    assert.match(map, /## 4\. What ["“]IR["”] means/i);

    // Live CLI surface inventory (must not invent hosts as live)
    for (const cmd of ['day', 'turn', 'approve', 'deny', 'claim', 'complete', 'graph']) {
      assert.match(map, new RegExp(`\\b${cmd}\\b`), `MAP should mention CLI command ${cmd}`);
    }
    assert.match(map, /npm run game/);
    assert.match(map, /npm run build:wasm/);
    assert.match(map, /npm test/);

    // Eve is trajectory, not claimed as shipped host
    assert.match(map, /Eve.*trajectory|trajectory.*Eve/i);
    assert.doesNotMatch(
      map,
      /## 1\.[\s\S]*?\|\s*\*\*Eve\b[^*]*\*\*\s*\|\s*`npm /,
      'Eve must not appear as a live npm entry in capabilities',
    );

    // Explicit IR definition (not bare acronym only)
    assert.match(
      map,
      /IR\s*=\s*Intermediate Representation|Intermediate Representation/i,
    );
    assert.match(map, /Turn status IR/i);
    assert.match(map, /Graph IR/i);
    assert.match(map, /Wait snapshot IR/i);
    assert.match(map, /turn --json/);

    // Layer ownership language: pkg is build output; session owns focus
    assert.match(map, /public\/game\/pkg/);
    assert.match(map, /Build output|build output|wasm-pack|checked-in/i);
    assert.match(map, /session store|session\/store|Focus.*source of truth|source of truth.*focus/i);
    assert.match(map, /mirror/i);
    assert.match(map, /crates\/peram-core/);
    assert.match(map, /src\/game\//);
  });

  it('README and AGENTS link to docs/MAP.md and targets exist', () => {
    const readme = read('README.md');
    const agents = read('AGENTS.md');
    assert.match(readme, /docs\/MAP\.md/);
    assert.match(agents, /docs\/MAP\.md/);
    assert.ok(fs.existsSync(mapPath));
  });

  it('layer paths referenced by MAP exist on disk', () => {
    const required = [
      'src',
      'src/game',
      'src/turn.js',
      'src/graph.js',
      'src/approvals.js',
      'public/game',
      'public/game/pkg',
      'public/game/pkg/peram_core_bg.wasm',
      'public/game/main.js',
      'public/game/engine.js',
      'public/watch',
      'crates/peram-core',
      'crates/peram-core/src/lib.rs',
      'bin/swarm.js',
    ];
    for (const rel of required) {
      assert.ok(fs.existsSync(path.join(root, rel)), `missing required path: ${rel}`);
    }
  });

  it('package.json scripts include live dogfood entries MAP documents', () => {
    const pkg = JSON.parse(read('package.json'));
    const scripts = pkg.scripts || {};
    for (const name of [
      'swarm:day',
      'swarm:turn',
      'swarm:graph',
      'game',
      'build:wasm',
      'test',
      'game:smoke',
    ]) {
      assert.ok(scripts[name], `package.json missing script: ${name}`);
    }
  });

  it('CLI help lists the live commands documented in MAP', () => {
    const r = spawnSync(process.execPath, ['bin/swarm.js', 'help'], {
      cwd: root,
      encoding: 'utf8',
    });
    assert.equal(r.status, 0, r.stderr || r.stdout);
    const out = r.stdout;
    for (const cmd of [
      'day',
      'turn',
      'approve',
      'deny',
      'claim',
      'complete',
      'release',
      'graph',
    ]) {
      assert.match(out, new RegExp(`^\\s*${cmd}\\b`, 'm'), `CLI help missing ${cmd}`);
    }
    assert.match(out, /turn status IR|--json/i);
  });

  it('turn status IR from shipped buildTurnStatus matches MAP contract (version 1 + next queues)', () => {
    const turn = buildTurnSurface({
      date: '2026-07-13',
      actions: [
        {
          id: 'grocery-errand',
          title: 'Grocery errand',
          realm: 'physical',
          area: 'Relationships',
          importance: 4,
          urgency: 3,
        },
        {
          id: 'apply-high-signal',
          title: 'FT application',
          realm: 'digital',
          area: 'Career',
          hitl: true,
          kind: 'job_application_submit',
          importance: 5,
          urgency: 4,
        },
      ],
      snapshot: emptySnapshot({ now: '2026-07-13T12:00:00.000Z' }),
      schedule: [],
      now: new Date('2026-07-13T12:00:00.000Z'),
    });
    const status = buildTurnStatus(turn);
    assert.equal(status.version, 1);
    assert.ok(status.next);
    assert.ok('physical' in status.next);
    assert.ok('authorization' in status.next);
    assert.ok(Array.isArray(status.physical));
    assert.ok(Array.isArray(status.pending));
    assert.ok(status.counts);
    // MAP documents these top-level keys as the agent contract
    const map = read('docs/MAP.md');
    for (const key of ['next.physical', 'next.authorization', 'version: 1', 'counts']) {
      assert.match(map, new RegExp(key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
    }
  });

  it('graph IR from shipped buildGameGraph is versioned nodes/edges as MAP describes', () => {
    const graph = buildGameGraph({
      date: '2026-07-13',
      projects: [{ id: 'p1', title: 'Ship', area: 'Systems' }],
      actions: [
        { id: 'a1', title: 'Ship', realm: 'digital', area: 'Systems' },
        { id: 'phys1', title: 'Walk', realm: 'physical', area: 'Health' },
      ],
      schedule: [],
      snapshot: emptySnapshot({ now: '2026-07-13T12:00:00.000Z' }),
      phases: ['ORIENT', 'PLAN'],
    });
    assert.equal(graph.version, 1);
    assert.ok(Array.isArray(graph.nodes) && graph.nodes.length > 0);
    assert.ok(Array.isArray(graph.edges));
    assert.ok(graph.meta);
    const map = read('docs/MAP.md');
    assert.match(map, /nodes\[\]/);
    assert.match(map, /edges\[\]/);
  });
});
