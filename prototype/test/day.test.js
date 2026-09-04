import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildDayPlan, selectDailyActions, verifyPlanShape, runDailySwarm } from '../src/day.js';
import { loadJson } from '../src/ingest.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

describe('day cycle integration (shipped entry path units)', () => {
  it('buildDayPlan produces projects, actions, schedule, balance, privacy split', () => {
    const persona = loadJson(path.join(root, 'public', 'persona', 'projection.json'));
    const state = loadJson(path.join(root, 'fixtures', 'state-sample.json'));
    const plan = buildDayPlan(persona, state, { date: '2026-07-12' });
    assert.ok(plan.projects.length > 0);
    assert.ok(plan.actions.length > 0);
    assert.ok(plan.actions.length <= 7, 'bounded action set');
    assert.ok(plan.schedule.length > 0);
    assert.equal(typeof plan.balance, 'number');
    assert.ok(plan.actions.every((a) => a.classification));
    // Starved relationships/health should be represented after ensureBalance
    const areas = new Set(plan.actions.map((a) => a.area));
    assert.ok(areas.has('Relationships') || areas.has('Health'));
  });

  it('selectDailyActions stays bounded and skips ELIMINATE noise', () => {
    const ranked = [
      { id: 'noise', title: 'scroll', area: 'Learning', quadrant: 'ELIMINATE', score: 1 },
      { id: 'c', title: 'career', area: 'Career', quadrant: 'DO_FIRST', score: 40 },
      { id: 'h', title: 'health', area: 'Health', quadrant: 'DO_FIRST', score: 35, source: 'balance_inject' },
    ];
    const selected = selectDailyActions(ranked, {});
    assert.ok(selected.length <= 7);
    assert.ok(selected.some((s) => s.id === 'h'));
    assert.ok(!selected.some((s) => s.id === 'noise'));
  });

  it('verifyPlanShape requires sections', () => {
    const bad = verifyPlanShape({ artifact: 'empty', actions: [] });
    assert.equal(bad.ok, false);
    assert.ok(bad.errors.length >= 1);

    const good = verifyPlanShape({
      artifact: '## Projects\n## Actions\n## Schedule\n',
      actions: [{ id: 'a' }],
      balance: 0.9,
    });
    assert.equal(good.ok, true);
  });

  it('runDailySwarm real entry produces DONE plan with required sections', () => {
    const result = runDailySwarm({
      root,
      date: '2026-07-12',
      write: false,
    });
    assert.equal(result.status, 'DONE');
    assert.ok(result.artifact);
    assert.match(result.artifact, /## Projects/i);
    assert.match(result.artifact, /## Actions/i);
    assert.match(result.artifact, /## Schedule/i);
    assert.ok(result.context.actions.length > 0);
  });
});
