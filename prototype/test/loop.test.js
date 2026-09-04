import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  createLoopCard,
  transition,
  runDayLoop,
  needsHitl,
  PHASES,
} from '../src/loop.js';

describe('loop (shipped looper-shaped day cycle)', () => {
  it('createLoopCard starts IDLE with budgets', () => {
    const card = createLoopCard({ date: '2026-07-12' });
    assert.equal(card.phase, 'IDLE');
    assert.equal(card.date, '2026-07-12');
    assert.equal(card.budgets.max_loop_iters, 8);
    assert.ok(card.success_criteria.length >= 3);
  });

  it('transition records phase_log', () => {
    let card = createLoopCard();
    card = transition(card, 'ORIENT', { progress: 'go', model_role: 'fast' });
    assert.equal(card.phase, 'ORIENT');
    assert.equal(card.phase_log.length, 1);
    assert.equal(card.phase_log[0].to, 'ORIENT');
  });

  it('needsHitl detects classification.hitl', () => {
    assert.equal(needsHitl([{ classification: { hitl: false } }]), false);
    assert.equal(needsHitl([{ classification: { hitl: true } }]), true);
  });

  it('runDayLoop reaches DONE with valid handlers and required plan sections', () => {
    const result = runDayLoop(
      {
        orient: () => ({ ok: true }),
        plan: () => ({ requiresHitl: false }),
        execute: () => ({
          artifact: [
            '# plan',
            '## Projects',
            '- p',
            '## Actions',
            '- a',
            '## Schedule & balance',
            '- s',
          ].join('\n'),
          actions: [{ id: 'a', title: 'A', classification: { hitl: false } }],
        }),
        verify: (ctx) => {
          const md = ctx.artifact || '';
          const errors = [];
          if (!/## Projects/.test(md)) errors.push('projects');
          if (!/## Actions/.test(md)) errors.push('actions');
          if (!/## Schedule/.test(md)) errors.push('schedule');
          if (!ctx.actions?.length) errors.push('no actions');
          return { ok: errors.length === 0, errors };
        },
        integrate: (ctx) => ({ artifact: ctx.artifact, artifacts: ['x'] }),
      },
      { date: '2026-07-12' },
    );
    assert.equal(result.status, 'DONE');
    assert.equal(result.card.phase, 'DONE');
    assert.ok(result.trail.some((t) => t.phase === 'ORIENT'));
    assert.ok(result.trail.some((t) => t.phase === 'EXECUTE'));
    assert.ok(result.trail.some((t) => t.phase === 'VERIFY'));
    assert.ok(result.artifact.includes('## Projects'));
  });

  it('PHASES includes HITL and terminal states', () => {
    assert.ok(PHASES.includes('HITL_PLAN_GATE'));
    assert.ok(PHASES.includes('DONE'));
    assert.ok(PHASES.includes('BLOCKED'));
  });
});
