/**
 * Pure Game of Peram session state — no DOM/GPU.
 * Feed existing graph IR; dispatch keyboard/voice/gamekit actions.
 * Growth (XP/quests) gamifies real physical / HITL / craft progress.
 */

import {
  emptyGrowth,
  nodeRole,
  isCompletable,
  applyGain,
  comboBonus,
  growthView,
  XP_TABLE,
} from './growth.js';

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
    area: n.area || n.growthArea || null,
    hitl: Boolean(n.hitl || n.type === 'hitl'),
    status: n.status || null,
    index: i,
    position: n.position || { x: (i % 4) * 220, y: Math.floor(i / 4) * 80 },
  }));

  // Map for O(1) id lookup (Patterns.dev js-performance-patterns)
  const nodeById = new Map(nodes.map((n) => [n.id, n]));

  const pending = (snapshot?.pending || graph.pending || graph.meta?.pending || [])
    .filter((p) => (p.status || 'pending') === 'pending')
    .map((p) => ({
      id: p.id,
      title: p.title || p.id,
      actionId: p.actionId || null,
      status: p.status || 'pending',
    }));

  const focusIndex = nodes.length ? 0 : -1;
  const growth = emptyGrowth();

  return {
    version: 1,
    mode: /** @type {Mode} */ ('nav'),
    nodes,
    nodeById,
    edges: graph.edges || [],
    pending,
    growth,
    focusIndex,
    selectedId: focusIndex >= 0 ? nodes[focusIndex].id : null,
    helpOpen: false,
    voiceListening: false,
    lastAction: null,
    lastVoice: null,
    message: nodes.length
      ? 'Grow — Enter claim beacon · A/D clear gates · Tab navigate · ? help'
      : 'Empty graph',
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
    case 'COMPLETE':
      return completeFocused(s);
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
  // Enter on a completable beacon = claim progress (growth loop)
  if (isCompletable(node)) {
    return completeFocused(s);
  }
  if (nodeRole(node) === 'hitl') {
    s.message = `Gate: ${node.label} — A approve · D deny (+XP)`;
    return s;
  }
  s.message = `Selected: ${node.label}`;
  return s;
}

function completeFocused(s) {
  ensureGrowth(s);
  const node = focusedNode(s);
  if (!node) {
    s.message = 'Nothing to claim';
    return s;
  }
  if (!isCompletable(node)) {
    if (nodeRole(node) === 'hitl') {
      s.message = `HITL gate — A approve / D deny (not claim)`;
    } else {
      s.message = `Not a claimable beacon: ${node.label}`;
    }
    return s;
  }
  if ((s.growth.completedIds || []).includes(node.id)) {
    s.message = `Already claimed: ${node.label}`;
    return s;
  }
  const role = nodeRole(node);
  const base = XP_TABLE[role] || XP_TABLE.craft;
  const streakBefore = s.growth.streak || 0;
  const combo = comboBonus(streakBefore + 1);
  const amount = base + combo;
  s.growth = applyGain(s.growth, {
    amount,
    reason: `claim:${role}`,
    role,
    nodeId: node.id,
    tick: s.tick,
  });
  node.status = 'done';
  s.selectedId = node.id;
  const comboNote = combo > 0 ? ` · combo +${combo}` : '';
  s.message = `+${amount} XP · claimed ${node.label} (${role})${comboNote}`;
  return s;
}

function resolvePending(s, status, explicitId) {
  ensureGrowth(s);
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
      return awardHitl(s, open, status);
    }
    s.message = `Authorization not found: ${id}`;
    return s;
  }
  if (item.status !== 'pending') {
    s.message = `Already ${item.status}: ${item.id}`;
    return s;
  }
  return awardHitl(s, item, status);
}

function awardHitl(s, item, status) {
  item.status = status;
  const base = status === 'approved' ? XP_TABLE.hitl_approve : XP_TABLE.hitl_deny;
  const streakBefore = s.growth.streak || 0;
  const combo = comboBonus(streakBefore + 1);
  const amount = base + combo;
  const focus = focusedNode(s);
  const nodeId = focus && (focus.type === 'hitl' || focus.hitl) ? focus.id : item.id;
  s.growth = applyGain(s.growth, {
    amount,
    reason: `hitl:${status}`,
    role: 'hitl',
    nodeId,
    tick: s.tick,
  });
  if (focus && (focus.type === 'hitl' || focus.hitl)) {
    focus.status = status;
  }
  const comboNote = combo > 0 ? ` · combo +${combo}` : '';
  s.message = `+${amount} XP · ${status} ${item.title || item.id}${comboNote}`;
  return s;
}

function ensureGrowth(s) {
  if (!s.growth) s.growth = emptyGrowth();
  if (!s.growth.completedIds) s.growth.completedIds = [];
  if (!s.growth.balance) s.growth.balance = { physical: 0, presence: 0, hitl: 0, craft: 0 };
}

/**
 * Snapshot of observable play state (for tests / HUD).
 */
export function sessionView(session) {
  const focus = focusedNode(session);
  const growth = growthView(session);
  return {
    focusIndex: session.focusIndex,
    selectedId: session.selectedId,
    focusLabel: focus?.label || null,
    focusRole: focus ? nodeRole(focus) : null,
    helpOpen: session.helpOpen,
    mode: session.mode,
    voiceListening: session.voiceListening,
    pendingOpen: (session.pending || []).filter((p) => p.status === 'pending').length,
    pendingResolved: (session.pending || []).filter((p) => p.status !== 'pending').length,
    message: session.message,
    tick: session.tick,
    nodeCount: session.nodes.length,
    growth,
  };
}
