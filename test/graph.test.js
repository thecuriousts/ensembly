import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { buildGameGraph, graphToMermaid, layoutGrid } from '../src/graph.js';

describe('graph (shipped game IR)', () => {
  it('buildGameGraph creates phase, action, physical, hitl nodes and edges', () => {
    const graph = buildGameGraph({
      date: '2026-07-13',
      card: { phase: 'HITL_WAIT', goal: 'play' },
      trail: [{ phase: 'ORIENT' }, { phase: 'PLAN' }, { phase: 'HITL_WAIT' }],
      projects: [{ id: 'ensembly-swarm', title: 'swarm', area: 'Systems' }],
      actions: [
        {
          id: 'grocery-errand',
          title: 'Grocery errand',
          realm: 'physical',
          area: 'Relationships',
        },
        {
          id: 'apply-high-signal',
          title: 'Apply',
          realm: 'digital',
          classification: { hitl: true },
        },
      ],
      schedule: [
        {
          start: '17:00',
          end: '18:00',
          label: 'Evening outdoor',
          non_negotiable: true,
          assigned: { id: 'grocery-errand', title: 'Grocery', area: 'Relationships' },
        },
      ],
      snapshot: {
        status: 'idle_waiting',
        pending: [
          {
            id: 'auth-apply-high-signal',
            actionId: 'apply-high-signal',
            title: 'Apply',
            status: 'pending',
          },
        ],
      },
    });

    assert.ok(graph.nodes.length >= 5);
    assert.ok(graph.edges.length >= 3);
    assert.ok(graph.nodes.some((n) => n.type === 'phase'));
    assert.ok(graph.nodes.some((n) => n.type === 'physical'));
    assert.ok(graph.nodes.some((n) => n.type === 'action' || n.type === 'hitl'));
    assert.ok(graph.nodes.every((n) => n.position && typeof n.position.x === 'number'));
    assert.equal(graph.meta.generator, 'ensembly/src/graph.js');
  });

  it('graphToMermaid emits flowchart with nodes', () => {
    const graph = buildGameGraph({
      trail: [{ phase: 'ORIENT' }],
      actions: [{ id: 'a', title: 'Do', realm: 'digital' }],
    });
    const md = graphToMermaid(graph);
    assert.match(md, /flowchart TB/);
    assert.match(md, /phase_ORIENT|ORIENT/);
  });

  it('layoutGrid assigns unique-ish positions', () => {
    const pos = layoutGrid([{ id: 'a' }, { id: 'b' }, { id: 'c' }]);
    assert.ok(pos.a);
    assert.ok(pos.b);
    assert.notEqual(pos.a.x + pos.a.y, undefined);
  });
});
