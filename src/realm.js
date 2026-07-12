/**
 * Physical vs digital realm classification.
 * Physical = human body / offline world must act.
 * Digital = agents/swarm can advance (subject to privacy/HITL).
 */

/** @typedef {'physical' | 'digital'} Realm */

const PHYSICAL_KINDS = new Set([
  'physical_errand',
  'physical_presence',
  'outdoor',
  'caregiving_in_person',
  'health_body',
  'household',
]);

const PHYSICAL_KEYWORDS = [
  /\boutdoor\b/i,
  /\bwalk\b/i,
  /\bgrocer/i,
  /\berrand\b/i,
  /\bpick\s*up\b/i,
  /\bfamily\s+support\b/i,
  /\bplay\s+with\b/i,
  /\bnaps?\b/i,
  /\blight\s+(weight\s+)?lift/i,
  /\bgarden/i,
  /\bin[- ]person\b/i,
  /\bcommute\b/i,
  /\bcook\b|\bmeal\s+prep\b/i,
  /\bput\s+(the\s+)?(baby|son|child)\s+to\s+sleep\b/i,
  /\bpresence\s+block\b/i,
  /\bwind-?down\b/i,
  /\bsleep\b/i,
  /\bbreakfast\b|\blunch\b|\bdinner\b/i,
];

const DIGITAL_KEYWORDS = [
  /\bgithub\b|\bpr\b|\bcommit\b|\boss\b/i,
  /\bemail\b|\bapply\b|\bapplication\b/i,
  /\bcv\b|\bportfolio\b|\bdevprofile\b/i,
  /\bcode\b|\bship\b|\bdeploy\b|\brefactor\b/i,
  /\bswarm\b|\bagent\b|\bcli\b/i,
];

/**
 * @param {{ id?: string, title?: string, area?: string, kind?: string, tags?: string[], realm?: string, body?: string, non_negotiable?: boolean, capacity?: string }} item
 * @returns {{ realm: Realm, reason: string }}
 */
export function classifyRealm(item = {}) {
  if (item.realm === 'physical' || item.realm === 'digital') {
    return { realm: item.realm, reason: `explicit realm:${item.realm}` };
  }

  const tags = (item.tags || []).map((t) => String(t).toLowerCase());
  if (tags.includes('physical') || tags.includes('offline') || tags.includes('errand')) {
    return { realm: 'physical', reason: 'tag physical/offline/errand' };
  }
  if (tags.includes('digital') || tags.includes('online')) {
    return { realm: 'digital', reason: 'tag digital/online' };
  }

  const kind = item.kind || '';
  if (PHYSICAL_KINDS.has(kind)) {
    return { realm: 'physical', reason: `kind ${kind}` };
  }

  const text = `${item.title || ''} ${item.body || ''} ${item.id || ''}`;
  for (const re of PHYSICAL_KEYWORDS) {
    if (re.test(text)) {
      return { realm: 'physical', reason: `matched ${re}` };
    }
  }

  // Relationship non-negotiable blocks and pure Health body blocks default physical
  if (item.non_negotiable && (item.area === 'Relationships' || item.area === 'Health')) {
    return { realm: 'physical', reason: 'non-negotiable presence/health block' };
  }

  for (const re of DIGITAL_KEYWORDS) {
    if (re.test(text)) {
      return { realm: 'digital', reason: `matched digital ${re}` };
    }
  }

  // Career/Systems/Learning default digital; Relationships/Health default physical when ambiguous
  if (item.area === 'Relationships' || item.area === 'Health') {
    return { realm: 'physical', reason: `area ${item.area} defaults physical when ambiguous` };
  }

  return { realm: 'digital', reason: 'default digital (agent-automatable)' };
}

/**
 * Attach realm classification to items.
 * @param {Array<object>} items
 */
export function enrichWithRealm(items = []) {
  return items.map((item) => {
    const realmInfo = classifyRealm(item);
    return { ...item, realm: realmInfo.realm, realmInfo };
  });
}

/**
 * Physical pickups the human must do (pending / not done).
 * @param {Array<object>} actions
 * @param {{ includeDone?: boolean }} opts
 */
export function physicalPickups(actions = [], opts = {}) {
  const enriched = enrichWithRealm(actions);
  return enriched.filter((a) => {
    if (a.realm !== 'physical') return false;
    if (!opts.includeDone && (a.status === 'done' || a.status === 'completed')) return false;
    return true;
  });
}

/**
 * Digital actions agents can progress (still may need HITL).
 */
export function digitalActions(actions = []) {
  return enrichWithRealm(actions).filter((a) => a.realm === 'digital');
}

export { PHYSICAL_KINDS };
