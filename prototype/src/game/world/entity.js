/**
 * Entities — sprite-backed actors bound optionally to graph/session nodes.
 */

import { spriteForNode, getSprite } from './sprites.js';

/**
 * @param {object} opts
 */
export function createEntity(opts = {}) {
  const spriteId = opts.spriteId || 'beacon_digital';
  const sprite = getSprite(spriteId);
  return {
    id: opts.id || `ent-${Math.random().toString(36).slice(2, 8)}`,
    kind: opts.kind || sprite.kind,
    spriteId,
    x: opts.x ?? 0,
    y: opts.y ?? 0,
    vx: 0,
    vy: 0,
    graphNodeId: opts.graphNodeId || null,
    label: opts.label || null,
    focused: Boolean(opts.focused),
    selected: Boolean(opts.selected),
    realm: opts.realm || null,
    z: opts.z ?? opts.y ?? 0,
  };
}

/**
 * Spawn player avatar in world.
 * @param {number} x
 * @param {number} y
 */
export function createAvatar(x, y) {
  return createEntity({
    id: 'avatar-player',
    kind: 'avatar',
    spriteId: 'avatar_walker',
    x,
    y,
    label: 'You',
    z: y,
  });
}

/**
 * Bind a session/graph node to a world beacon entity.
 * @param {object} node session node
 * @param {number} x
 * @param {number} y
 */
export function entityFromGraphNode(node, x, y) {
  const spriteId = spriteForNode(node);
  return createEntity({
    id: `ent-${node.id}`,
    kind: 'beacon',
    spriteId,
    x,
    y,
    graphNodeId: node.id,
    label: node.label || node.id,
    focused: false,
    selected: false,
    realm: node.realm || null,
    z: y,
  });
}

/**
 * @param {Array<object>} entities
 * @param {object} entity
 */
export function spawnEntity(entities, entity) {
  if (entities.some((e) => e.id === entity.id)) {
    return entities.map((e) => (e.id === entity.id ? { ...e, ...entity } : e));
  }
  return [...entities, entity];
}

/**
 * Move entity toward target (simple seek).
 * @param {object} entity
 * @param {number} tx
 * @param {number} ty
 * @param {number} speed
 */
export function seekEntity(entity, tx, ty, speed = 4) {
  const dx = tx - entity.x;
  const dy = ty - entity.y;
  const dist = Math.hypot(dx, dy) || 1;
  if (dist < speed) {
    return { ...entity, x: tx, y: ty, vx: 0, vy: 0, z: ty };
  }
  const vx = (dx / dist) * speed;
  const vy = (dy / dist) * speed;
  const y = entity.y + vy;
  return { ...entity, x: entity.x + vx, y, vx, vy, z: y };
}

/**
 * Sync focus flags from session focus graph id.
 * @param {Array<object>} entities
 * @param {string | null} focusedGraphId
 * @param {string | null} selectedGraphId
 */
export function syncFocusFlags(entities, focusedGraphId, selectedGraphId) {
  return entities.map((e) => ({
    ...e,
    focused: Boolean(focusedGraphId && e.graphNodeId === focusedGraphId),
    selected: Boolean(selectedGraphId && e.graphNodeId === selectedGraphId),
  }));
}

/**
 * Find entity by graph node id.
 */
export function findByGraphId(entities, graphNodeId) {
  return entities.find((e) => e.graphNodeId === graphNodeId) || null;
}
