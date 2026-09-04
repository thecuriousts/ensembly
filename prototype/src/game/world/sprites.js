/**
 * Sprite atlas definitions — procedural frames (no external DCC).
 * Each sprite is a drawable recipe the renderer interprets.
 */

/** @typedef {'avatar' | 'beacon' | 'agent' | 'spirit' | 'marker'} SpriteKind */

/**
 * Registry of sprite recipes.
 * @type {Record<string, { kind: SpriteKind, w: number, h: number, colors: object, parts: string[] }>}
 */
export const SPRITE_ATLAS = Object.freeze({
  avatar_walker: {
    kind: 'avatar',
    w: 28,
    h: 40,
    colors: { body: '#6ef0ff', cloak: '#1a3a50', accent: '#ff8a4a', skin: '#e8c4a8' },
    parts: ['cloak', 'body', 'head', 'eyes'],
  },
  beacon_digital: {
    kind: 'beacon',
    w: 36,
    h: 48,
    colors: { core: '#5a8cff', glow: '#6ef0ff', base: '#1a2744' },
    parts: ['base', 'pillar', 'core', 'glow'],
  },
  beacon_physical: {
    kind: 'beacon',
    w: 36,
    h: 48,
    colors: { core: '#3dff9a', glow: '#6dffa8', base: '#0f3a2a' },
    parts: ['base', 'pillar', 'core', 'glow'],
  },
  beacon_hitl: {
    kind: 'beacon',
    w: 40,
    h: 52,
    colors: { core: '#ff7a45', glow: '#ffb080', base: '#4a1c12' },
    parts: ['base', 'pillar', 'core', 'glow', 'ring'],
  },
  beacon_phase: {
    kind: 'beacon',
    w: 32,
    h: 44,
    colors: { core: '#8aa4ff', glow: '#b0c4ff', base: '#1a2860' },
    parts: ['base', 'pillar', 'core'],
  },
  agent_swarm: {
    kind: 'agent',
    w: 24,
    h: 24,
    colors: { body: '#d070ff', eye: '#fff' },
    parts: ['body', 'eye'],
  },
  marker_focus: {
    kind: 'marker',
    w: 48,
    h: 12,
    colors: { ring: '#7cf0ff' },
    parts: ['ellipse'],
  },
});

/**
 * @param {string} spriteId
 */
export function getSprite(spriteId) {
  return SPRITE_ATLAS[spriteId] || SPRITE_ATLAS.beacon_digital;
}

/**
 * Pick sprite id for a graph/session node type.
 * @param {{ type?: string, realm?: string, hitl?: boolean }} node
 */
export function spriteForNode(node = {}) {
  if (node.type === 'hitl' || node.hitl) return 'beacon_hitl';
  if (node.type === 'physical' || node.realm === 'physical') return 'beacon_physical';
  if (node.type === 'phase') return 'beacon_phase';
  if (node.type === 'game') return 'agent_swarm';
  return 'beacon_digital';
}

/**
 * List all sprite ids (for asset inventory / tests).
 */
export function listSpriteIds() {
  return Object.keys(SPRITE_ATLAS);
}
