/**
 * Pure Game of Peram session state — no DOM/GPU.
 * Feed existing graph IR; dispatch keyboard/voice/gamekit actions.
 */

/** @typedef {'nav' | 'command' | 'help' | 'voice'} Mode */

/**
 * @param {{ nodes?: Array<object>, edges?: Array<object>, meta?: object }} graph
 * @param {{ pending?: Array<object> }} [snapshot]
 */
export function createSession(graph = {}, snapshot = null) {
  const nodes = (graph.nodes || []).map((n, i) => ({
    id: n.id,
    label: n.label || n.id,
    type: n.type || 'action',
    realm: n.realm || null,
    hitl: Boolean(n.hitl || n.type === 'hitl'),
    status: n.status || null,
    index: i,
    position: n.position || { x: (i % 4) * 220, y: Math.floor(i / 4) * 80 },
  }));

  // Map for O(1) id lookup (Patterns.dev js-performance-patterns)
  const nodeById = new Map(nodes.map((n) => [n.id, n]));

  const pending = (snapshot?.pending || graph.meta?.pending || [])
    .filter((p) => (p.status || 'pending') === 'pending')
    .map((p) => ({
      id: p.id,
      title: p.title || p.id,
      actionId: p.actionId || null,
      status: p.status || 'pending',
    }));

  const focusIndex = nodes.length ? 0 : -1;

  return {
    version: 1,
    mode: /** @type {Mode} */ ('nav'),
    nodes,
    nodeById,
    edges: graph.edges || [],
    pending,
    focusIndex,
    selectedId: focusIndex >= 0 ? nodes[focusIndex].id : null,
    helpOpen: false,
    voiceListening: false,
    lastAction: null,
    lastVoice: null,
    message: nodes.length ? 'Ready — Tab/arrows navigate · ? help · A/D approve/deny · U undo' : 'Empty graph',
    tick: 0,
    meta: graph.meta || {},
  };
}

/**
 * Focused node or null.
 */
export function focusedNode(session) {
  if (!session || session.focusIndex < 0) return null;
  return session.nodes[session.focusIndex] || null;
}

/**
 * Apply a pure action. Returns new session (immutable).
 * @param {object} session
 * @param {{ type: string, payload?: any }} action
 */
export function dispatch(session, action) {
  if (!session || !action?.type) return session;
  // structuredClone (Patterns.dev) — deep copy without JSON loss
  const s = structuredClone(session);
  // Rebuild Map after clone (Map survives structuredClone in modern engines;
  // re-sync from nodes for safety)
  s.nodeById = new Map((s.nodes || []).map((n) => [n.id, n]));
  s.tick = (s.tick || 0) + 1;
  s.lastAction = action.type;

  switch (action.type) {
    case 'FOCUS_NEXT':
      return focusDelta(s, 1);
    case 'FOCUS_PREV':
      return focusDelta(s, -1);
    case 'FOCUS_UP':
      return focusGrid(s, 0, -1);
    case 'FOCUS_DOWN':
      return focusGrid(s, 0, 1);
    case 'FOCUS_LEFT':
      return focusGrid(s, -1, 0);
    case 'FOCUS_RIGHT':
      return focusGrid(s, 1, 0);
    case 'SELECT':
      return selectFocused(s);
    case 'TOGGLE_HELP':
      s.helpOpen = !s.helpOpen;
      s.mode = s.helpOpen ? 'help' : 'nav';
      s.message = s.helpOpen ? 'Help open — press ? or Esc to close' : 'Help closed';
      return s;
    case 'CLOSE_HELP':
      s.helpOpen = false;
      s.mode = 'nav';
      s.message = 'Nav mode';
      return s;
    case 'APPROVE':
      return resolvePending(s, 'approved', action.payload?.id);
    case 'DENY':
      return resolvePending(s, 'denied', action.payload?.id);
    case 'VOICE_START':
      s.voiceListening = true;
      s.mode = 'voice';
      s.message = 'Listening… (or type voice command)';
      return s;
    case 'VOICE_STOP':
      s.voiceListening = false;
      s.mode = 'nav';
      s.message = 'Voice stopped';
      return s;
    case 'VOICE_TEXT':
      s.lastVoice = String(action.payload?.text || '');
      s.message = `Heard: ${s.lastVoice}`;
      return s;
    case 'SET_POSITIONS': {
      const positions = action.payload?.positions || [];
      // Map lookup O(1) in hot layout path
      for (let i = 0, len = positions.length; i < len; i++) {
        const p = positions[i];
        const node = s.nodeById.get(p.id);
        if (node && p.x != null && p.y != null) {
          node.position = { x: p.x, y: p.y };
        }
      }
      s.message = `Layout updated (${positions.length} nodes)`;
      return s;
    }
    case 'SET_MESSAGE':
      s.message = String(action.payload?.message || s.message);
      return s;
    case 'FOCUS_INDEX': {
      const idx = Number(action.payload?.index);
      if (!Number.isFinite(idx) || idx < 0 || idx >= s.nodes.length) {
        s.message = 'Invalid focus index';
        return s;
      }
      s.focusIndex = idx;
      s.selectedId = s.nodes[idx].id;
      s.message = `Focus: ${s.nodes[idx].label}`;
      return s;
    }
    default:
      s.message = `Unknown action: ${action.type}`;
      return s;
  }
}

function focusDelta(s, delta) {
  if (!s.nodes.length) return s;
  const n = s.nodes.length;
  s.focusIndex = (s.focusIndex + delta + n * 10) % n;
  s.selectedId = s.nodes[s.focusIndex].id;
  s.message = `Focus: ${s.nodes[s.focusIndex].label}`;
  return s;
}

function focusGrid(s, dx, dy) {
  if (!s.nodes.length || s.focusIndex < 0) return s;
  const cols = 4;
  const i = s.focusIndex;
  const col = i % cols;
  const row = Math.floor(i / cols);
  let nc = col + dx;
  let nr = row + dy;
  if (nc < 0) nc = 0;
  if (nr < 0) nr = 0;
  let ni = nr * cols + nc;
  if (ni >= s.nodes.length) ni = s.nodes.length - 1;
  s.focusIndex = ni;
  s.selectedId = s.nodes[ni].id;
  s.message = `Focus: ${s.nodes[ni].label}`;
  return s;
}

function selectFocused(s) {
  const node = focusedNode(s);
  if (!node) {
    s.message = 'Nothing to select';
    return s;
  }
  s.selectedId = node.id;
  s.message = `Selected: ${node.label}`;
  return s;
}

function resolvePending(s, status, explicitId) {
  let id = explicitId;
  if (!id) {
    const node = focusedNode(s);
    if (node?.type === 'hitl' || node?.hitl) {
      id = node.id.startsWith('auth-') || node.id.startsWith('hitl-')
        ? node.id.replace(/^hitl-/, 'auth-')
        : `auth-${node.id.replace(/^action-/, '')}`;
    }
    const open = s.pending.find((p) => p.status === 'pending');
    if (!id && open) id = open.id;
  }
  if (!id) {
    s.message = `No pending authorization to ${status === 'approved' ? 'approve' : 'deny'}`;
    return s;
  }
  const item = s.pending.find((p) => p.id === id || p.actionId === id || p.id === `auth-${id}`);
  if (!item) {
    // try fuzzy match on focused action
    const open = s.pending.find((p) => p.status === 'pending');
    if (open) {
      open.status = status;
      s.message = `${status}: ${open.id}`;
      return s;
    }
    s.message = `Authorization not found: ${id}`;
    return s;
  }
  if (item.status !== 'pending') {
    s.message = `Already ${item.status}: ${item.id}`;
    return s;
  }
  item.status = status;
  s.message = `${status}: ${item.id} — ${item.title}`;
  return s;
}

/**
 * Snapshot of observable play state (for tests / HUD).
 */
export function sessionView(session) {
  const focus = focusedNode(session);
  return {
    focusIndex: session.focusIndex,
    selectedId: session.selectedId,
    focusLabel: focus?.label || null,
    helpOpen: session.helpOpen,
    mode: session.mode,
    voiceListening: session.voiceListening,
    pendingOpen: (session.pending || []).filter((p) => p.status === 'pending').length,
    pendingResolved: (session.pending || []).filter((p) => p.status !== 'pending').length,
    message: session.message,
    tick: session.tick,
    nodeCount: session.nodes.length,
  };
}
