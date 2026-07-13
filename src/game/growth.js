/**
 * Growth engine — gamifies real life progress (pure, no DOM).
 * XP, levels, quests, balance, streaks tied to physical / HITL / craft / presence.
 */
import { buildSpnQuote } from './spn.js';

/** @typedef {'physical' | 'presence' | 'hitl' | 'craft' | 'meta'} GrowthRole */

export const XP_TABLE = Object.freeze({
  physical: 40,
  presence: 35,
  hitl_approve: 28,
  hitl_deny: 18,
  craft: 22,
  combo: 6,
});

/** Level titles — ascent language, not cartoon baby-talk */
export const LEVEL_TITLES = Object.freeze([
  'Ember',
  'Spark',
  'Kindling',
  'Forge',
  'Beacon',
  'Pathfinder',
  'Steward',
  'Peram',
  'Ascent',
  'Horizon',
]);

export const XP_PER_LEVEL = 100;

/**
 * @param {object} node
 * @returns {GrowthRole}
 */
export function nodeRole(node) {
  if (!node) return 'meta';
  const t = node.type || '';
  const realm = node.realm || '';
  const area = (node.area || node.growthArea || '').toLowerCase();
  if (t === 'phase' || t === 'game' || node.kind === 'root') return 'meta';
  if (t === 'physical' || realm === 'physical') return 'physical';
  if (t === 'schedule' || area === 'family' || area === 'health' || area === 'presence') {
    return 'presence';
  }
  if (t === 'hitl' || node.hitl) return 'hitl';
  if (t === 'action' || t === 'digital' || realm === 'digital') return 'craft';
  return 'meta';
}

/**
 * @param {object} node
 */
export function isCompletable(node) {
  const role = nodeRole(node);
  return role !== 'meta' && role !== 'hitl';
}

/**
 * @param {number} xp
 */
export function levelFromXp(xp) {
  const safe = Math.max(0, Number(xp) || 0);
  const level = Math.floor(safe / XP_PER_LEVEL) + 1;
  const into = safe % XP_PER_LEVEL;
  const title = LEVEL_TITLES[Math.min(level - 1, LEVEL_TITLES.length - 1)];
  return {
    level,
    title,
    xp: safe,
    xpIntoLevel: into,
    xpToNext: XP_PER_LEVEL - into,
    progress01: into / XP_PER_LEVEL,
  };
}

/**
 * Empty growth state for a new session.
 */
export function emptyGrowth() {
  return {
    xp: 0,
    streak: 0,
    maxStreak: 0,
    completedIds: [],
    balance: { physical: 0, presence: 0, hitl: 0, craft: 0 },
    lastGain: null,
    events: [],
  };
}

/**
 * Build quest board from graph nodes (completable + open HITL).
 * @param {object} session
 */
export function buildQuests(session) {
  const completed = new Set(session.growth?.completedIds || []);
  const pendingOpen = new Set(
    (session.pending || []).filter((p) => p.status === 'pending').map((p) => p.id),
  );
  const quests = [];

  for (const node of session.nodes || []) {
    const role = nodeRole(node);
    if (role === 'meta') continue;

    if (role === 'hitl') {
      const authId = hitlAuthId(node);
      const open =
        pendingOpen.has(authId) ||
        (session.pending || []).some(
          (p) => p.status === 'pending' && (p.id === authId || p.actionId === node.id),
        );
      // Also treat any open pending as linked when node is hitl type
      const anyPending = (session.pending || []).some((p) => p.status === 'pending');
      const done = !open && completed.has(node.id);
      quests.push({
        id: node.id,
        label: node.label || node.id,
        role: 'hitl',
        xp: XP_TABLE.hitl_approve,
        status: done ? 'done' : open || anyPending ? 'open' : 'done',
        hint: 'A approve · D deny',
      });
      continue;
    }

    if (!isCompletable(node)) continue;
    const done = completed.has(node.id);
    quests.push({
      id: node.id,
      label: node.label || node.id,
      role,
      xp: XP_TABLE[role] || XP_TABLE.craft,
      status: done ? 'done' : 'open',
      hint: 'Enter claim',
    });
  }

  return quests;
}

function hitlAuthId(node) {
  if (!node?.id) return '';
  if (node.id.startsWith('auth-')) return node.id;
  if (node.id.startsWith('hitl-')) return node.id.replace(/^hitl-/, 'auth-');
  return `auth-${node.id.replace(/^action-/, '')}`;
}

/**
 * Award XP + streak; mutates growth object (session already cloned).
 * @param {object} growth
 * @param {{ amount: number, reason: string, role?: string, nodeId?: string, tick?: number }} gain
 */
export function applyGain(growth, gain) {
  const amount = Math.max(0, Number(gain.amount) || 0);
  const g = growth || emptyGrowth();
  if (amount <= 0) return g;

  g.xp = (g.xp || 0) + amount;
  g.streak = (g.streak || 0) + 1;
  g.maxStreak = Math.max(g.maxStreak || 0, g.streak);
  g.lastGain = {
    amount,
    reason: gain.reason || 'progress',
    role: gain.role || null,
    nodeId: gain.nodeId || null,
    tick: gain.tick || 0,
  };
  if (gain.role && g.balance && gain.role in g.balance) {
    g.balance[gain.role] = (g.balance[gain.role] || 0) + 1;
  }
  if (gain.nodeId) {
    const ids = new Set(g.completedIds || []);
    ids.add(gain.nodeId);
    g.completedIds = [...ids];
  }
  g.events = [...(g.events || []).slice(-19), g.lastGain];
  return g;
}

/**
 * Combo bonus after a gain streak ≥ 2.
 */
export function comboBonus(streak) {
  if (streak < 2) return 0;
  return Math.min(24, (streak - 1) * XP_TABLE.combo);
}

/**
 * Snapshot for HUD / tests.
 * @param {object} session
 */
export function growthView(session) {
  const g = session?.growth || emptyGrowth();
  const lvl = levelFromXp(g.xp);
  const quests = buildQuests(session || { nodes: [], pending: [], growth: g });
  const open = quests.filter((q) => q.status === 'open');
  const done = quests.filter((q) => q.status === 'done');
  const questProgress01 = quests.length ? done.length / quests.length : 0;

  // Balance score: reward covering physical + presence + craft (not only digital)
  const b = g.balance || {};
  const axes = ['physical', 'presence', 'craft', 'hitl'];
  const covered = axes.filter((k) => (b[k] || 0) > 0).length;
  const balance01 = covered / axes.length;

  const spn = buildSpnQuote(g);

  return {
    ...lvl,
    streak: g.streak || 0,
    maxStreak: g.maxStreak || 0,
    lastGain: g.lastGain,
    balance: { ...b },
    balance01,
    quests,
    questsOpen: open.length,
    questsDone: done.length,
    questsTotal: quests.length,
    questProgress01,
    /** Overall day growth meter (quests + balance + level progress) */
    growthMeter01: clamp01(questProgress01 * 0.55 + balance01 * 0.25 + lvl.progress01 * 0.2),
    nextQuest: open[0] || null,
    /** $SPN progress ticker (stock-style quote from real gameplay events) */
    spn,
  };
}

function clamp01(n) {
  if (n < 0) return 0;
  if (n > 1) return 1;
  return n;
}

/**
 * Human coaching line — growth oriented, not empty praise.
 * @param {ReturnType<typeof growthView>} view
 */
export function growthCoachLine(view) {
  if (!view) return 'Claim beacons · clear gates · show up in the body-world.';
  if (view.questsOpen === 0 && view.questsTotal > 0) {
    return `Day board clear · Lv ${view.level} ${view.title} · protect the win with rest.`;
  }
  if ((view.balance?.physical || 0) === 0 && view.quests.some((q) => q.role === 'physical' && q.status === 'open')) {
    return 'Growth tip: claim a physical beacon — body-world work compounds.';
  }
  if ((view.balance?.presence || 0) === 0 && view.quests.some((q) => q.role === 'presence' && q.status === 'open')) {
    return 'Growth tip: honor presence (family/health schedule) before more craft.';
  }
  if (view.questsOpen > 0 && view.nextQuest) {
    return `Next: ${view.nextQuest.label} (+${view.nextQuest.xp} XP · ${view.nextQuest.role})`;
  }
  return `Lv ${view.level} ${view.title} · ${view.xp} XP · streak ${view.streak}`;
}
