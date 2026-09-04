import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  eisenhowerScore,
  eisenhowerQuadrant,
  balancedScore,
  prioritize,
  mergeCandidates,
} from '../src/prioritize.js';

describe('prioritize (shipped)', () => {
  it('eisenhowerScore weights importance over urgency', () => {
    const highImp = eisenhowerScore({ importance: 4, urgency: 1 });
    const highUrg = eisenhowerScore({ importance: 1, urgency: 4 });
    assert.ok(highImp > highUrg);
  });

  it('eisenhowerQuadrant maps DO_FIRST / SCHEDULE / DELEGATE / ELIMINATE', () => {
    assert.equal(eisenhowerQuadrant({ importance: 4, urgency: 4 }), 'DO_FIRST');
    assert.equal(eisenhowerQuadrant({ importance: 4, urgency: 1 }), 'SCHEDULE');
    assert.equal(eisenhowerQuadrant({ importance: 1, urgency: 4 }), 'DELEGATE');
    assert.equal(eisenhowerQuadrant({ importance: 1, urgency: 1 }), 'ELIMINATE');
  });

  it('balancedScore boosts starved areas vs saturated', () => {
    const item = { importance: 3, urgency: 3, area: 'Health' };
    const starved = balancedScore(item, { Health: 1.3 }, { Health: 0 }, ['health']);
    const saturated = balancedScore(item, { Health: 1.3 }, { Health: 4 }, ['health']);
    assert.ok(starved > saturated);
  });

  it('prioritize ranks higher score first and respects limit', () => {
    const items = [
      { id: 'a', importance: 2, urgency: 2, area: 'Learning' },
      { id: 'b', importance: 4, urgency: 4, area: 'Career' },
      { id: 'c', importance: 3, urgency: 2, area: 'Systems' },
    ];
    const ranked = prioritize(items, {
      balanceWeights: { Career: 1.15, Systems: 1, Learning: 0.9 },
      areaCounts: {},
      priorityStack: ['relationships', 'health', 'career', 'systems', 'learning'],
      limit: 2,
    });
    assert.equal(ranked.length, 2);
    assert.equal(ranked[0].id, 'b');
    assert.ok(ranked[0].score >= ranked[1].score);
    assert.equal(ranked[0].quadrant, 'DO_FIRST');
  });

  it('mergeCandidates de-dupes by id with later override', () => {
    const merged = mergeCandidates(
      [{ id: 'x', title: 'old', importance: 2 }],
      [{ id: 'x', title: 'new', urgency: 4 }, { id: 'y', title: 'y' }],
    );
    assert.equal(merged.length, 2);
    const x = merged.find((m) => m.id === 'x');
    assert.equal(x.title, 'new');
    assert.equal(x.importance, 2);
    assert.equal(x.urgency, 4);
  });
});
