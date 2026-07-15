import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { buildTurnSurface } from '../src/turn.js';
import { emptySnapshot } from '../src/approvals.js';
import { buildPlayableGraphFromTurn, listBeacons } from '../src/play.js';
import { createDigitalFlow } from '../src/digital-flow.js';
import { createSession } from '../src/game/session.js';

describe('playable life-mirror graph (shipped pure path)', () => {
  const actions = [
    {
      id: 'grocery-errand',
      title: 'Grocery errand — family meals',
      realm: 'physical',
      area: 'Relationships',
      importance: 4,
      urgency: 4,
      classification: { hitl: false },
    },
    {
      id: 'apply-high-signal',
      title: 'Prepare high-signal FT application packet',
      kind: 'job_application_submit',
      realm: 'digital',
      area: 'Career',
      classification: { hitl: true, reason: 'HITL' },
    },
  ];

  it('stamps real next physical + next auth as beacons from turn IR', () => {
    const turn = buildTurnSurface({
      date: '2026-07-15',
      actions,
      snapshot: emptySnapshot(),
      schedule: [
        {
          start: '17:00',
          end: '18:00',
          label: 'Errand window',
          assigned: { id: 'grocery-errand', title: 'Grocery' },
        },
      ],
      now: new Date('2026-07-15T17:30:00'),
    });
    turn.plan = { actions, projects: [], schedule: turn.schedule };

    assert.equal(turn.nextPhysical?.id, 'grocery-errand');
    assert.equal(turn.nextAuth?.id, 'auth-apply-high-signal');

    const graph = buildPlayableGraphFromTurn({ turn });
    assert.equal(graph.meta.source, 'life-operator-ir');
    assert.equal(graph.meta.nextPhysicalId, 'grocery-errand');
    assert.equal(graph.meta.nextAuthId, 'auth-apply-high-signal');
    assert.match(graph.meta.nextPhysicalLabel || '', /Grocery/);
    assert.match(graph.meta.nextAuthLabel || '', /application|Apply/i);

    const beacons = listBeacons(graph);
    assert.ok(beacons.some((b) => b.beaconRole === 'next_physical'));
    assert.ok(beacons.some((b) => b.beaconRole === 'next_auth'));
    // Real action ids appear as nodes — not sample-graph labels
    assert.ok(graph.nodes.some((n) => n.id === 'action-grocery-errand' || n.id.includes('grocery')));
    assert.ok(
      graph.nodes.some(
        (n) =>
          n.id === 'auth-apply-high-signal' ||
          n.id === 'hitl-apply-high-signal' ||
          (n.beaconRole === 'next_auth' && n.label),
      ),
    );
  });

  it('includes Bank place when digital bill_pay flow attached', () => {
    const turn = buildTurnSurface({
      date: '2026-07-15',
      actions,
      snapshot: emptySnapshot(),
    });
    turn.plan = { actions, projects: [], schedule: [] };
    const flow = createDigitalFlow({
      id: 'flow-bill_pay',
      kind: 'bill_pay',
      payeeLabel: 'electric',
      now: '2026-07-15T12:00:00.000Z',
    });
    const graph = buildPlayableGraphFromTurn({ turn, digitalFlows: [flow] });
    assert.ok(graph.nodes.some((n) => n.type === 'place' && n.label === 'Bank'));
    assert.ok(graph.nodes.some((n) => n.id === 'action-flow-bill_pay' || n.label?.includes('Bank')));
    assert.ok(graph.meta.digitalFlowCount >= 1);
  });

  it('session binds life-derived graph (not sample-only theater)', () => {
    const turn = buildTurnSurface({
      date: '2026-07-15',
      actions,
      snapshot: emptySnapshot(),
    });
    turn.plan = { actions, projects: [], schedule: [] };
    const graph = buildPlayableGraphFromTurn({ turn });
    const session = createSession(graph, { pending: graph.pending });
    assert.ok(session.nodes.length >= 2);
    assert.ok(session.nodes.some((n) => /grocery|Grocery/i.test(n.label) || n.id.includes('grocery')));
    assert.ok(session.pending.length >= 1);
  });
});
