/**
 * Privacy classification — public (commit-eligible / shareable) vs private (local-only).
 * Pure functions; no IO.
 */

/** @typedef {'public' | 'private'} Visibility */

const PRIVATE_AREAS = new Set(['Finance']);

const PRIVATE_KEYWORDS = [
  /medical|dna|diagnosis|prescription|hospital|clinic/i,
  /debt|a-kassa|pension|bank|transfer|salary|tax|invoice|account\s*#/i,
  /passport|visa|oci\s*number|ssn|personnummer/i,
  /address|phone\s*number|private\s*calendar/i,
  /family\s*health|infant\s*medical|wife\s*medical/i,
];

const PUBLIC_HINTS = [
  /oss|open.?source|github|pr\b|commit|publish\s*skill|portfolio|devprofile/i,
  /blog|build\s*in\s*public|x\.com|tweet/i,
];

const HITL_KINDS = new Set([
  'external_email_send',
  'job_application_submit',
  'calendar_mutate',
  'finance_transfer',
  'git_push_shared',
  'publish_private_data',
]);

/**
 * Classify a single action or project item.
 * @param {{ id?: string, title?: string, area?: string, public?: boolean, kind?: string, tags?: string[], body?: string }} item
 * @returns {{ visibility: Visibility, reason: string, hitl: boolean, pushable: boolean }}
 */
export function classifyItem(item = {}) {
  const title = String(item.title || item.id || '');
  const body = String(item.body || '');
  const text = `${title} ${body} ${(item.tags || []).join(' ')}`;
  const area = item.area || '';
  const kind = item.kind || '';

  const highStakes =
    HITL_KINDS.has(kind) || /submit|send_email|wire_transfer|force.?push/i.test(text);

  if (item.public === false) {
    return {
      visibility: 'private',
      reason: 'explicit public:false',
      hitl: highStakes,
      pushable: false,
    };
  }

  if (PRIVATE_AREAS.has(area)) {
    return {
      visibility: 'private',
      reason: `area ${area} is local-only by default`,
      hitl: true, // finance-class always human-gated
      pushable: false,
    };
  }

  for (const re of PRIVATE_KEYWORDS) {
    if (re.test(text)) {
      return {
        visibility: 'private',
        reason: `matched private pattern ${re}`,
        hitl: true,
        pushable: false,
      };
    }
  }

  if (highStakes) {
    const isFinancey = /finance|transfer|bank/i.test(text) || kind === 'finance_transfer';
    return {
      visibility: isFinancey || item.public === false ? 'private' : item.public === true ? 'public' : 'private',
      reason: `high-stakes kind/action requires HITL (${kind || 'inferred'})`,
      hitl: true,
      pushable: item.public === true && !isFinancey,
    };
  }

  if (item.public === true || PUBLIC_HINTS.some((re) => re.test(text))) {
    return {
      visibility: 'public',
      reason: item.public === true ? 'explicit public:true' : 'public craft/share signal',
      hitl: false,
      pushable: true,
    };
  }

  // Default: local-only until proven public (safe default)
  return {
    visibility: 'private',
    reason: 'default-deny: not marked public',
    hitl: false,
    pushable: false,
  };
}

/**
 * Partition items into public vs private lists with classification metadata.
 * @param {Array<object>} items
 */
export function partitionByVisibility(items = []) {
  const publicItems = [];
  const privateItems = [];
  for (const item of items) {
    const classification = classifyItem(item);
    const enriched = { ...item, classification };
    if (classification.visibility === 'public') publicItems.push(enriched);
    else privateItems.push(enriched);
  }
  return { publicItems, privateItems };
}

/**
 * Paths that must remain untracked / local-only (contract for docs + checks).
 */
export function privatePathPatterns() {
  return [
    'private/',
    'data/local/',
    'data/private/',
    '*.local.json',
    '*.private.md',
    '*.private.json',
    '.env',
    '.env.*',
  ];
}

export { HITL_KINDS, PRIVATE_AREAS };
