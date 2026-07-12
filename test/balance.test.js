import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  countByArea,
  balanceScore,
  ensureBalance,
  proposeSchedule,
} from '../src/balance.js';

describe('balance (shipped)', () => {
  it('countByArea tallies known areas', () => {
    const counts = countByArea([
      { area: 'Career' },
      { area: 'Career' },
      { area: 'Health' },
    ]);
    assert.equal(counts.Career, 2);
    assert.equal(counts.Health, 1);
    assert.equal(counts.Relationships, 0);
  });

  it('balanceScore penalizes missing Relationships/Health', () => {
    const careerOnly = balanceScore([
      { area: 'Career' },
      { area: 'Career' },
      { area: 'Systems' },
    ]);
    const balanced = balanceScore([
      { area: 'Career' },
      { area: 'Health' },
      { area: 'Relationships' },
    ]);
    assert.ok(balanced > careerOnly);
    assert.ok(balanced >= 0.8);
  });

  it('ensureBalance injects Relationships and Health when absent', () => {
    const ranked = [
      { id: 'c1', title: 'career A', area: 'Career', importance: 4, urgency: 4, score: 30, quadrant: 'DO_FIRST' },
      { id: 'c2', title: 'career B', area: 'Career', importance: 3, urgency: 3, score: 25, quadrant: 'DO_FIRST' },
      { id: 's1', title: 'systems', area: 'Systems', importance: 3, urgency: 2, score: 20, quadrant: 'SCHEDULE' },
    ];
    const persona = {
      project_seeds: [
        { id: 'family-presence', title: 'Protected family presence', area: 'Relationships', public: false },
        { id: 'healthy-self-energy', title: 'Energy foundation', area: 'Health', public: false },
      ],
    };
    const { items, injections, balance } = ensureBalance(ranked, persona);
    assert.ok(injections.length >= 2);
    assert.ok(items.some((i) => i.area === 'Relationships'));
    assert.ok(items.some((i) => i.area === 'Health'));
    assert.ok(balance > 0);
  });

  it('proposeSchedule assigns deep work and keeps non-negotiable labels', () => {
    const persona = {
      daily_rhythm: [
        {
          id: 'outdoor_family_light',
          start: '06:30',
          end: '07:30',
          label: 'Morning outdoor family + light',
          areas: ['Health', 'Relationships'],
          non_negotiable: true,
        },
        {
          id: 'deep_focus_1',
          start: '10:00',
          end: '13:00',
          label: 'Deep focus block 1',
          areas: ['Career', 'Systems'],
          capacity: 'deep',
        },
      ],
    };
    const ranked = [
      { id: 'ship', title: 'Ship swarm', area: 'Systems', quadrant: 'DO_FIRST' },
    ];
    const slots = proposeSchedule(persona, ranked);
    assert.equal(slots.length, 2);
    assert.equal(slots[0].non_negotiable, true);
    assert.ok(slots[0].assigned);
    assert.equal(slots[1].assigned?.id, 'ship');
  });
});
