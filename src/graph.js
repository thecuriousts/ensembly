/**
 * Serializable game graph IR for the Game of Peram watch surface.
 * Shape inspired by Stately graph packages (nodes + edges); no hard dep required.
 */

/**
 * @param {{
 *   date?: string,
 *   card?: object,
 *   trail?: Array<{ phase: string }>,
 *   actions?: Array<object>,
 *   projects?: Array<object>,
 *   schedule?: Array<object>,
 *   snapshot?: object,
 * }} state
 */
export function buildGameGraph(state = {}) {
  const nodes = [];
  const edges = [];
  const seen = new Set();

  function addNode(node) {
    if (seen.has(node.id)) return;
    seen.add(node.id);
    nodes.push(node);
  }

  function addEdge(source, target, kind = 'flow') {
    edges.push({ id: `e-${source}-${target}-${kind}`, source, target, kind });
  }

  // Phase trail
  const trail = state.trail || [];
  let prevPhase = null;
  for (const t of trail) {
    const id = `phase-${t.phase}`;
    addNode({
      id,
      type: 'phase',
      label: t.phase,
      kind: 'phase',
    });
    if (prevPhase) addEdge(prevPhase, id, 'phase_transition');
    prevPhase = id;
  }

  // Root play node
  addNode({
    id: 'game-peram',
    type: 'game',
    label: `Game of Peram ${state.date || ''}`.trim(),
    kind: 'root',
  });
  if (prevPhase) addEdge('game-peram', prevPhase, 'contains');
  else if (state.card?.phase) {
    const id = `phase-${state.card.phase}`;
    addNode({ id, type: 'phase', label: state.card.phase, kind: 'phase' });
    addEdge('game-peram', id, 'contains');
  }

  // Projects
  for (const p of state.projects || []) {
    const id = `project-${p.id}`;
    addNode({
      id,
      type: 'project',
      label: p.title || p.id,
      area: p.area,
      visibility: p.classification?.visibility,
      kind: 'project',
    });
    addEdge('game-peram', id, 'project');
  }

  // Actions
  for (const a of state.actions || []) {
    const id = `action-${a.id}`;
    const realm = a.realm || a.realmInfo?.realm || 'digital';
    addNode({
      id,
      type: realm === 'physical' ? 'physical' : 'action',
      label: a.title || a.id,
      area: a.area,
      realm,
      hitl: Boolean(a.classification?.hitl),
      kind: 'action',
    });
    addEdge('game-peram', id, realm === 'physical' ? 'physical_pickup' : 'digital_action');
    if (a.classification?.hitl) {
      const hid = `hitl-${a.id}`;
      addNode({
        id: hid,
        type: 'hitl',
        label: `HITL: ${a.title || a.id}`,
        kind: 'hitl_wait',
      });
      addEdge(id, hid, 'requires_auth');
    }
  }

  // Snapshot pending
  for (const p of state.snapshot?.pending || []) {
    const id = `auth-${p.id}`;
    addNode({
      id,
      type: 'hitl',
      label: `${p.status}: ${p.title}`,
      status: p.status,
      kind: 'authorization',
    });
    if (p.actionId) {
      addEdge(`action-${p.actionId}`, id, 'authorization');
    } else {
      addEdge('game-peram', id, 'authorization');
    }
  }

  // Schedule blocks
  for (const s of state.schedule || []) {
    const id = `slot-${s.start}-${s.end}`.replace(/:/g, '');
    addNode({
      id,
      type: 'schedule',
      label: `${s.start}-${s.end} ${s.label}`,
      non_negotiable: s.non_negotiable,
      kind: 'schedule',
    });
    addEdge('game-peram', id, 'schedule');
    if (s.assigned?.id) {
      addEdge(id, `action-${s.assigned.id}`, 'assigned');
    }
  }

  // Simple layout-ready positions (grid; Stately layout peer optional later)
  const layout = layoutGrid(nodes);

  return {
    version: 1,
    date: state.date || null,
    meta: {
      nodeCount: nodes.length,
      edgeCount: edges.length,
      snapshotStatus: state.snapshot?.status || null,
      generator: 'ensembly/src/graph.js',
    },
    nodes: nodes.map((n) => ({ ...n, position: layout[n.id] })),
    edges,
  };
}

/**
 * Deterministic grid layout for watch surfaces (no external layout peer).
 */
export function layoutGrid(nodes = [], opts = {}) {
  const colWidth = opts.colWidth || 220;
  const rowHeight = opts.rowHeight || 80;
  const cols = opts.cols || 4;
  const positions = {};
  nodes.forEach((n, i) => {
    const col = i % cols;
    const row = Math.floor(i / cols);
    positions[n.id] = { x: col * colWidth, y: row * rowHeight };
  });
  return positions;
}

/**
 * Export mermaid flowchart for watch / docs.
 */
export function graphToMermaid(graph) {
  const lines = ['flowchart TB'];
  for (const n of graph.nodes || []) {
    const safeId = sanitizeId(n.id);
    const label = escapeLabel(n.label || n.id);
    const shape = shapeFor(n.type);
    lines.push(`  ${safeId}${shape.open}${label}${shape.close}`);
  }
  for (const e of graph.edges || []) {
    lines.push(`  ${sanitizeId(e.source)} -->|${e.kind || 'flow'}| ${sanitizeId(e.target)}`);
  }
  return lines.join('\n');
}

function sanitizeId(id) {
  return String(id).replace(/[^a-zA-Z0-9_]/g, '_');
}

function escapeLabel(s) {
  return String(s).replace(/"/g, "'").slice(0, 60);
}

function shapeFor(type) {
  switch (type) {
    case 'phase':
      return { open: '([', close: '])' };
    case 'physical':
      return { open: '[[', close: ']]' };
    case 'hitl':
      return { open: '{{', close: '}}' };
    case 'schedule':
      return { open: '[(', close: ')]' };
    case 'game':
      return { open: '((', close: '))' };
    default:
      return { open: '[', close: ']' };
  }
}

/**
 * Minimal HTML watch page embedding mermaid (CDN) for local viewing.
 */
export function graphToWatchHtml(graph, mermaidSource) {
  const mermaid = mermaidSource || graphToMermaid(graph);
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Game of Peram — watch</title>
  <style>
    body { font-family: system-ui, sans-serif; margin: 1.5rem; background: #0b1020; color: #e8ecf4; }
    h1 { font-size: 1.25rem; }
    .meta { opacity: 0.8; font-size: 0.9rem; }
    .panel { background: #141a2e; border-radius: 12px; padding: 1rem; margin-top: 1rem; }
    pre.json { overflow: auto; max-height: 240px; font-size: 0.75rem; }
  </style>
</head>
<body>
  <h1>Game of Peram — live watch</h1>
  <p class="meta">nodes: ${graph.meta?.nodeCount ?? 0} · edges: ${graph.meta?.edgeCount ?? 0} · snapshot: ${graph.meta?.snapshotStatus ?? 'n/a'}</p>
  <div class="panel">
    <pre class="mermaid">
${mermaid}
    </pre>
  </div>
  <div class="panel">
    <h2>Graph IR</h2>
    <pre class="json">${escapeHtml(JSON.stringify(graph, null, 2))}</pre>
  </div>
  <script type="module">
    import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
    mermaid.initialize({ startOnLoad: true, theme: 'dark' });
  </script>
</body>
</html>
`;
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}
