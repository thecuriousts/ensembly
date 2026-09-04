/**
 * Eisenhower + persona balance weighting prioritization.
 * Pure functions.
 */

/**
 * Raw Eisenhower score: importance/urgency in 1..4.
 * @param {{ importance?: number, urgency?: number }} item
 */
export function eisenhowerScore(item = {}) {
  const importance = clamp(Number(item.importance) || 1, 1, 4);
  const urgency = clamp(Number(item.urgency) || 1, 1, 4);
  // Weighted: importance dominates slightly (strategic over firefighting)
  return importance * 4 + urgency * 3;
}

/**
 * Quadrant label for human plans.
 */
export function eisenhowerQuadrant(item = {}) {
  const importance = clamp(Number(item.importance) || 1, 1, 4);
  const urgency = clamp(Number(item.urgency) || 1, 1, 4);
  if (importance >= 3 && urgency >= 3) return 'DO_FIRST';
  if (importance >= 3 && urgency < 3) return 'SCHEDULE';
  if (importance < 3 && urgency >= 3) return 'DELEGATE';
  return 'ELIMINATE';
}

/**
 * Apply persona balance weights + under-served area boost.
 * @param {object} item
 * @param {Record<string, number>} balanceWeights
 * @param {Record<string, number>} areaCounts recent action counts per area
 * @param {string[]} priorityStack lowercase area names ordered by preference
 */
export function balancedScore(item, balanceWeights = {}, areaCounts = {}, priorityStack = []) {
  const base = eisenhowerScore(item);
  const area = item.area || 'Systems';
  const weight = balanceWeights[area] ?? 1;
  const count = areaCounts[area] ?? 0;
  // Boost areas that have been starved (0 recent) more than saturated ones
  const starveBoost = count === 0 ? 1.25 : count === 1 ? 1.1 : count >= 3 ? 0.85 : 1;
  const stackIdx = priorityStack.findIndex(
    (p) => p.toLowerCase() === String(area).toLowerCase(),
  );
  const stackBoost = stackIdx >= 0 ? 1 + (priorityStack.length - stackIdx) * 0.02 : 1;
  return base * weight * starveBoost * stackBoost;
}

/**
 * Sort and rank a list of candidate actions/projects.
 * @param {Array<object>} items
 * @param {{ balanceWeights?: object, areaCounts?: object, priorityStack?: string[], limit?: number }} opts
 */
export function prioritize(items = [], opts = {}) {
  const {
    balanceWeights = {},
    areaCounts = {},
    priorityStack = [],
    limit = Infinity,
  } = opts;

  const ranked = items.map((item) => {
    const score = balancedScore(item, balanceWeights, areaCounts, priorityStack);
    const quadrant = eisenhowerQuadrant(item);
    return { ...item, score, quadrant };
  });

  ranked.sort((a, b) => b.score - a.score || String(a.id).localeCompare(String(b.id)));
  return ranked.slice(0, limit);
}

/**
 * Merge persona project seeds with optional runtime candidates.
 */
export function mergeCandidates(personaProjects = [], extra = []) {
  const byId = new Map();
  for (const p of [...personaProjects, ...extra]) {
    if (!p || !p.id) continue;
    byId.set(p.id, { ...byId.get(p.id), ...p });
  }
  return [...byId.values()];
}

function clamp(n, lo, hi) {
  return Math.min(hi, Math.max(lo, n));
}
