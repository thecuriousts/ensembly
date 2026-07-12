/**
 * Life-area balance: ensure non-negotiable areas appear in daily plans.
 * Pure functions.
 */

const DEFAULT_AREAS = [
  'Career',
  'Health',
  'Finance',
  'Learning',
  'Relationships',
  'Systems',
  'Creative',
];

/**
 * Count actions/projects by life area.
 * @param {Array<{ area?: string }>} items
 */
export function countByArea(items = [], areas = DEFAULT_AREAS) {
  const counts = Object.fromEntries(areas.map((a) => [a, 0]));
  for (const item of items) {
    const a = item.area || 'Systems';
    if (counts[a] === undefined) counts[a] = 0;
    counts[a] += 1;
  }
  return counts;
}

/**
 * Score how balanced a selection is (0..1). Penalize missing non-negotiables.
 * @param {Array<{ area?: string }>} selected
 * @param {{ nonNegotiableAreas?: string[], minAreas?: number }} opts
 */
export function balanceScore(selected = [], opts = {}) {
  const nonNeg = opts.nonNegotiableAreas || ['Relationships', 'Health'];
  const minAreas = opts.minAreas ?? 3;
  const counts = countByArea(selected);
  const present = Object.values(counts).filter((c) => c > 0).length;
  const areaCoverage = Math.min(1, present / minAreas);
  let nn = 0;
  for (const a of nonNeg) {
    if ((counts[a] || 0) > 0) nn += 1;
  }
  const nnScore = nonNeg.length ? nn / nonNeg.length : 1;
  // Domination penalty: one area > 60% of items
  const total = selected.length || 1;
  const maxShare = Math.max(...Object.values(counts), 0) / total;
  const domination = maxShare > 0.6 ? 0.7 : 1;
  return Number((areaCoverage * 0.4 + nnScore * 0.45 + domination * 0.15).toFixed(4));
}

/**
 * Inject minimal placeholder actions for missing non-negotiable areas.
 * @param {Array<object>} ranked prioritized list
 * @param {object} persona
 * @param {{ maxInject?: number }} opts
 */
export function ensureBalance(ranked = [], persona = {}, opts = {}) {
  const maxInject = opts.maxInject ?? 3;
  const nonNegAreas = ['Relationships', 'Health'];
  const selected = ranked.slice(0, Math.max(5, ranked.length));
  const counts = countByArea(selected);
  const injections = [];

  for (const area of nonNegAreas) {
    if ((counts[area] || 0) > 0) continue;
    const seed = (persona.project_seeds || []).find((p) => p.area === area);
    injections.push({
      id: seed?.id || `balance-${area.toLowerCase()}`,
      title: seed?.title || `Protected ${area} block`,
      area,
      importance: 4,
      urgency: 3,
      public: seed?.public ?? false,
      source: 'balance_inject',
      quadrant: 'DO_FIRST',
      score: 999,
    });
    if (injections.length >= maxInject) break;
  }

  // Cap career domination: if top 5 are all Career, pull one Systems/Health
  const top = ranked.slice(0, 5);
  if (top.length >= 3 && top.every((t) => t.area === 'Career')) {
    const alt = ranked.find((r) => r.area !== 'Career');
    if (alt && !injections.find((i) => i.id === alt.id)) {
      injections.push({ ...alt, source: 'balance_diversify' });
    }
  }

  const byId = new Map();
  for (const item of [...injections, ...ranked]) {
    if (!item?.id) continue;
    if (!byId.has(item.id)) byId.set(item.id, item);
  }
  return {
    items: [...byId.values()],
    injections,
    balance: balanceScore([...injections, ...ranked.slice(0, 5)], {
      nonNegotiableAreas: nonNegAreas,
    }),
  };
}

/**
 * Propose schedule slots from persona rhythm + ranked work items.
 * Non-negotiable rhythm blocks always appear; deep work gets top Career/Systems.
 */
export function proposeSchedule(persona = {}, rankedActions = []) {
  const rhythm = persona.daily_rhythm || [];
  const work = rankedActions.filter(
    (a) => a.area === 'Career' || a.area === 'Systems' || a.area === 'Learning',
  );
  let workIdx = 0;
  const slots = [];

  for (const block of rhythm) {
    const slot = {
      start: block.start,
      end: block.end,
      label: block.label,
      areas: block.areas || [],
      non_negotiable: Boolean(block.non_negotiable),
      assigned: null,
    };
    if (block.capacity === 'deep' || block.capacity === 'medium') {
      const item = work[workIdx++];
      if (item) {
        slot.assigned = {
          id: item.id,
          title: item.title,
          area: item.area,
          quadrant: item.quadrant,
        };
      }
    } else if (block.non_negotiable) {
      slot.assigned = {
        id: `rhythm-${block.id}`,
        title: block.label,
        area: (block.areas && block.areas[0]) || 'Relationships',
        quadrant: 'DO_FIRST',
      };
    }
    slots.push(slot);
  }

  return slots;
}

export { DEFAULT_AREAS };
