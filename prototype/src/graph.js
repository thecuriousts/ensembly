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
      label: `${s.start}-${s.end} ${s.label || ''}`.trim(),
      non_negotiable: s.non_negotiable,
      kind: 'schedule',
    });
    addEdge('game-peram', id, 'schedule');
    if (s.assigned?.id) {
      const aid = `action-${s.assigned.id}`;
      // Ensure endpoint exists so Mermaid never gets dangling edges only
      if (!seen.has(aid)) {
        addNode({
          id: aid,
          type: 'action',
          label: s.assigned.title || s.assigned.id,
          area: s.assigned.area,
          kind: 'action',
          realm: s.assigned.realm || 'digital',
        });
      }
      addEdge(id, aid, 'assigned');
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
 * Labels are always double-quoted so real titles with (), &, +, : parse on Mermaid 11+.
 * Only edges whose endpoints exist as nodes are emitted (no silent parse failures).
 */
export function graphToMermaid(graph) {
  const lines = ['flowchart TB'];
  const ids = new Set();
  for (const n of graph.nodes || []) {
    const safeId = sanitizeId(n.id);
    ids.add(safeId);
    const label = escapeLabel(n.label || n.id);
    const shape = shapeFor(n.type);
    lines.push(`  ${safeId}${shape.open}"${label}"${shape.close}`);
  }
  for (const e of graph.edges || []) {
    const src = sanitizeId(e.source);
    const tgt = sanitizeId(e.target);
    if (!ids.has(src) || !ids.has(tgt)) continue;
    const kind = sanitizeEdgeLabel(e.kind || 'flow');
    lines.push(`  ${src} -->|${kind}| ${tgt}`);
  }
  return lines.join('\n');
}

function sanitizeId(id) {
  const s = String(id).replace(/[^a-zA-Z0-9_]/g, '_');
  // Mermaid ids must not start with a digit
  return /^\d/.test(s) ? `n_${s}` : s;
}

/** Mermaid node text inside "..." — strip chars that still break quoted labels. */
function escapeLabel(s) {
  return String(s)
    .replace(/[\r\n]+/g, ' ')
    .replace(/"/g, "'")
    .replace(/&/g, 'and')
    .replace(/[<>]/g, '')
    .replace(/[{}]/g, '')
    .slice(0, 56)
    .trim();
}

/** Edge mid-labels: keep identifiers only (no | or newlines). */
function sanitizeEdgeLabel(kind) {
  return String(kind).replace(/[^a-zA-Z0-9_ -]/g, '_').slice(0, 40) || 'flow';
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
 * Minimal HTML watch page: next-action panel + mermaid (CDN) + graph IR.
 * @param {object} graph
 * @param {string} [mermaidSource]
 * @param {{ status?: object|null }} [opts] turn status IR (next physical / next auth)
 */
export function graphToWatchHtml(graph, mermaidSource, opts = {}) {
  const mermaid = mermaidSource || graphToMermaid(graph);
  const status = opts.status || graph.meta?.turnStatus || null;
  const actionPanel = renderActionPanel(status);
  // Escape for HTML text nodes; browser decodes entities before Mermaid reads textContent.
  const mermaidHtml = escapeHtml(mermaid);
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Game of Peram — watch</title>
  <style>
    body { font-family: system-ui, sans-serif; margin: 1.5rem; background: #0b1020; color: #e8ecf4; }
    h1 { font-size: 1.25rem; }
    h2 { font-size: 1.05rem; margin: 0 0 0.75rem; }
    .meta { opacity: 0.8; font-size: 0.9rem; }
    .panel { background: #141a2e; border-radius: 12px; padding: 1rem; margin-top: 1rem; }
    .next { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); }
    .card { background: #0f1528; border: 1px solid #2a3555; border-radius: 10px; padding: 0.85rem 1rem; }
    .card h3 { margin: 0 0 0.4rem; font-size: 0.95rem; color: #9ecbff; }
    .card.auth h3 { color: #ffb4a2; }
    .card p { margin: 0.25rem 0; font-size: 0.9rem; }
    .card code, .card pre { font-size: 0.78rem; }
    .card pre { background: #080c18; padding: 0.6rem 0.75rem; border-radius: 8px; overflow: auto; margin: 0.5rem 0 0; }
    pre.json { overflow: auto; max-height: 320px; font-size: 0.75rem; }
    .empty { opacity: 0.65; font-style: italic; }
    details.ir { margin-top: 1rem; }
    details.ir > summary {
      cursor: pointer; list-style: none; opacity: 0.75; font-size: 0.85rem;
      padding: 0.5rem 0;
    }
    details.ir > summary:hover { opacity: 1; }
    details.ir > summary::-webkit-details-marker { display: none; }
    details.ir[open] > summary { margin-bottom: 0.5rem; opacity: 0.9; }
    .ir-note { font-size: 0.8rem; opacity: 0.7; margin: 0 0 0.5rem; }
    /* Mermaid host: div (not pre) avoids whitespace/font quirks */
    .mermaid-wrap { overflow: auto; min-height: 12rem; }
    .mermaid { background: #0a0e1a; border-radius: 10px; padding: 0.75rem; text-align: center; }
    .mermaid svg { max-width: 100%; height: auto; }
    #mermaid-error {
      display: none; margin-top: 0.75rem; padding: 0.75rem 1rem;
      background: #2a1520; border: 1px solid #6a3040; border-radius: 10px; color: #ffb4a2;
      font-size: 0.88rem; white-space: pre-wrap;
    }
    #mermaid-error.show { display: block; }
  </style>
</head>
<body>
  <h1>Game of Peram — live watch</h1>
  <p class="meta">nodes: ${graph.meta?.nodeCount ?? 0} · edges: ${graph.meta?.edgeCount ?? 0} · snapshot: ${graph.meta?.snapshotStatus ?? 'n/a'}${graph.meta?.turnStatusAt ? ` · updated: ${escapeHtml(String(graph.meta.turnStatusAt))}` : ''}${graph.meta?.nextPhysicalId != null ? ` · next body: ${escapeHtml(String(graph.meta.nextPhysicalId || '—'))}` : ''}</p>
  ${actionPanel}
  <div class="panel">
    <h2>Day map</h2>
    <div class="mermaid-wrap">
      <div class="mermaid" id="day-map">
${mermaidHtml}
      </div>
    </div>
    <div id="mermaid-error" role="alert"></div>
    <p class="ir-note">If the diagram stays blank offline: open via a local server (not raw file://) so the Mermaid CDN can load — e.g. <code>npm run game</code> then <code>/public/watch/</code>, or re-run <code>npm run swarm:graph</code>.</p>
  </div>
  <details class="panel ir">
    <summary>Graph IR (debug · agents) — collapsed; not for day-to-day pickup</summary>
    <p class="ir-note">Machine nodes/edges that <em>feed</em> the diagram and game. Humans use <strong>Next action</strong> + Mermaid above. Full file: <code>public/watch/graph.json</code></p>
    <pre class="json">${escapeHtml(JSON.stringify(graph, null, 2))}</pre>
  </details>
  <script type="module">
    const errEl = document.getElementById('mermaid-error');
    function showErr(msg) {
      if (!errEl) return;
      errEl.textContent = msg;
      errEl.classList.add('show');
    }
    try {
      // Explicit run: ESM loads after window "load", so startOnLoad alone never fires.
      const mermaid = (await import('https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs')).default;
      mermaid.initialize({
        startOnLoad: false,
        theme: 'dark',
        securityLevel: 'strict',
        flowchart: { htmlLabels: false, curve: 'basis' },
      });
      await mermaid.run({ querySelector: '#day-map.mermaid' });
    } catch (e) {
      showErr(
        'Mermaid failed to render. ' +
        (String(e && e.message || e)) +
        '\\n\\nTip: serve over http (not file://) so jsDelivr ESM can load. ' +
        'Try: npm run game  →  open /public/watch/index.html  or  python -m http.server 4174'
      );
      console.error('[watch mermaid]', e);
    }
  </script>
</body>
</html>
`;
}

function renderActionPanel(status) {
  if (!status || !status.next) {
    return `<div class="panel"><h2>Next action</h2><p class="empty">No turn status attached — run <code>npm run swarm:graph</code> after turn, or <code>node bin/swarm.js graph --html</code>.</p></div>`;
  }
  const phys = status.next.physical;
  const auth = status.next.authorization;
  const physCard = phys
    ? `<div class="card">
        <h3>Body — next physical</h3>
        <p><strong>${escapeHtml(phys.title)}</strong></p>
        <p>id: <code>${escapeHtml(phys.id)}</code> · ${escapeHtml(phys.area || '—')} · ${escapeHtml(phys.claimStatus || 'open')}</p>
        ${
          phys.scheduleWindow
            ? `<p>window: ${escapeHtml(phys.scheduleWindow.start)}–${escapeHtml(phys.scheduleWindow.end)}${phys.scheduleWindow.label ? ` · ${escapeHtml(phys.scheduleWindow.label)}` : ''}</p>`
            : ''
        }
        <pre>${escapeHtml((phys.commands && phys.commands.claim) || '')}\n${escapeHtml((phys.commands && phys.commands.complete) || '')}</pre>
      </div>`
    : `<div class="card"><h3>Body — next physical</h3><p class="empty">Queue clear</p></div>`;
  const authCard = auth
    ? `<div class="card auth">
        <h3>Auth — next authorization</h3>
        <p><strong>${escapeHtml(auth.title)}</strong></p>
        <p>id: <code>${escapeHtml(auth.id)}</code> · ${escapeHtml(auth.kind || 'hitl')}</p>
        <pre>${escapeHtml((auth.commands && auth.commands.approve) || '')}\n${escapeHtml((auth.commands && auth.commands.deny) || '')}</pre>
      </div>`
    : `<div class="card auth"><h3>Auth — next authorization</h3><p class="empty">No pending HITL</p></div>`;
  return `<div class="panel"><h2>Next action</h2><div class="next">${physCard}${authCard}</div></div>`;
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}
