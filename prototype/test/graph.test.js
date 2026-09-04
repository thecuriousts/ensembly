import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { buildGameGraph, graphToMermaid, graphToWatchHtml, layoutGrid } from '../src/graph.js';

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

  it('graphToMermaid quotes labels so parentheses and & parse on Mermaid 11', () => {
    const graph = buildGameGraph({
      trail: [{ phase: 'HITL_WAIT' }],
      projects: [{ id: 'runway', title: 'Runway & admin (local only)' }],
      actions: [
        {
          id: 'apply',
          title: 'Prepare high-signal FT application packet (public CV path)',
          realm: 'digital',
          classification: { hitl: true },
        },
      ],
      schedule: [
        {
          start: '06:00',
          end: '06:30',
          label: 'Wake',
          assigned: { id: 'apply', title: 'Apply' },
        },
      ],
      snapshot: {
        status: 'idle_waiting',
        pending: [
          {
            id: 'auth-apply',
            actionId: 'apply',
            title: 'Prepare high-signal FT application packet (public CV path)',
            status: 'pending',
          },
        ],
      },
    });
    const md = graphToMermaid(graph);
    // Always double-quoted labels (Mermaid 11); & normalized to "and" for safe parse
    assert.match(md, /project_runway\["Runway and admin \(local only\)"\]/);
    assert.match(md, /action_apply\["Prepare high-signal FT application packet \(public/);
    assert.match(md, /hitl_apply\{\{"HITL: Prepare high-signal/);
    assert.match(md, /slot_0600_0630\[\("06:00-06:30 Wake"\)\]/);
    // Every node line must quote its label: shapeOpen "label" shapeClose
    for (const line of md.split('\n')) {
      if (!line.trim() || line.startsWith('flowchart') || line.includes('-->')) continue;
      assert.match(
        line,
        /^\s+\w+(?:\(|\[|\{)+".*"(?:\)|\]|\})+\s*$/,
        `expected quoted mermaid label: ${line}`,
      );
    }
  });

  it('layoutGrid assigns unique-ish positions', () => {
    const pos = layoutGrid([{ id: 'a' }, { id: 'b' }, { id: 'c' }]);
    assert.ok(pos.a);
    assert.ok(pos.b);
    assert.notEqual(pos.a.x + pos.a.y, undefined);
  });

  it('graphToWatchHtml includes next-action panel and quoted mermaid labels', () => {
    const graph = buildGameGraph({
      trail: [{ phase: 'HITL_WAIT' }],
      actions: [
        {
          id: 'apply',
          title: 'Prepare packet (public CV path)',
          realm: 'digital',
          classification: { hitl: true },
        },
        { id: 'grocery', title: 'Grocery & produce', realm: 'physical' },
      ],
    });
    const mermaid = graphToMermaid(graph);
    const status = {
      next: {
        physical: {
          id: 'grocery',
          title: 'Grocery & produce',
          area: 'Relationships',
          claimStatus: 'open',
          commands: {
            claim: 'node bin/swarm.js claim grocery',
            complete: 'node bin/swarm.js complete grocery',
          },
        },
        authorization: {
          id: 'auth-apply',
          title: 'Prepare packet (public CV path)',
          kind: 'hitl',
          commands: {
            approve: 'node bin/swarm.js approve auth-apply',
            deny: 'node bin/swarm.js deny auth-apply',
          },
        },
      },
    };
    const html = graphToWatchHtml(graph, mermaid, { status });
    assert.match(html, /Next action/);
    assert.match(html, /Body — next physical/);
    assert.match(html, /Auth — next authorization/);
    assert.match(html, /swarm\.js claim grocery/);
    assert.match(html, /swarm\.js approve auth-apply/);
    assert.match(mermaid, /\["Prepare packet \(public CV path\)"\]|\{\{"HITL:/);
    assert.doesNotMatch(mermaid, /\[Prepare packet \(public/);
    // Graph IR is debug substrate — collapsed, not a second full-page dump
    assert.match(html, /<details[\s\S]*Graph IR/);
    assert.doesNotMatch(html, /<div class="panel">\s*<h2>Graph IR<\/h2>/);
  });

  it('graphToWatchHtml uses explicit mermaid.run (ESM-safe) and escapes labels for HTML', () => {
    const graph = buildGameGraph({
      date: '2026-07-13',
      actions: [{ id: 'runway', title: 'Runway & admin (local only)', realm: 'digital' }],
      schedule: [
        {
          start: '06:30',
          end: '07:30',
          label: 'Morning',
          assigned: { id: 'rhythm-only', title: 'Rhythm block' },
        },
      ],
    });
    // Assigned-only action becomes a real node (no dangling edge)
    assert.ok(graph.nodes.some((n) => n.id === 'action-rhythm-only'));
    const mermaid = graphToMermaid(graph);
    assert.match(mermaid, /action_rhythm_only/);
    assert.match(mermaid, /Runway and admin/);
    // Dangling-only edges omitted: every edge endpoint is defined
    const defined = new Set(
      mermaid
        .split('\n')
        .filter((l) => /^\s+\w+[(\[{]/.test(l))
        .map((l) => l.trim().match(/^(\w+)/)?.[1])
        .filter(Boolean),
    );
    for (const line of mermaid.split('\n')) {
      const m = line.match(/^\s+(\w+)\s+-->\|[^|]+\|\s+(\w+)\s*$/);
      if (m) {
        assert.ok(defined.has(m[1]), `edge source missing node: ${m[1]}`);
        assert.ok(defined.has(m[2]), `edge target missing node: ${m[2]}`);
      }
    }
    const html = graphToWatchHtml(graph, mermaid, { status: null });
    assert.match(html, /mermaid\.run\s*\(/);
    assert.match(html, /startOnLoad:\s*false/);
    assert.match(html, /id="day-map"/);
    // HTML-escaped ampersand from original title path
    assert.match(html, /Runway and admin|Runway &amp; admin/);
  });
});
