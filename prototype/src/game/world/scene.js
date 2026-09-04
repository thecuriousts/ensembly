/**
 * Scene — composes environment + entities + props; binds session graph.
 * Pure foundation for scalable game world.
 */

import { createEnvironment, defaultBounds } from './environment.js';
import { defaultCourtyardProps, sortProps } from './props.js';
import {
  createAvatar,
  entityFromGraphNode,
  spawnEntity,
  seekEntity,
  syncFocusFlags,
  findByGraphId,
} from './entity.js';

/**
 * Create empty world scene.
 * @param {{ biome?: string }} [opts]
 */
export function createScene(opts = {}) {
  const bounds = defaultBounds();
  const environment = createEnvironment(opts.biome || 'night_courtyard');
  const props = defaultCourtyardProps(bounds);
  const midX = bounds.width * 0.5;
  const midY = bounds.height * 0.72;
  const entities = [createAvatar(midX, midY + 40)];
  return {
    version: 1,
    bounds,
    camera: { x: 0, y: 0, zoom: 1 },
    environment,
    entities,
    props,
    time: 0,
  };
}

/**
 * Place graph nodes as beacons along a path in the courtyard.
 * @param {object} scene
 * @param {Array<object>} nodes session/graph nodes
 */
export function bindGraphToScene(scene, nodes = []) {
  const bounds = scene.bounds;
  const baseY = bounds.height * 0.58;
  const startX = bounds.width * 0.22;
  const span = bounds.width * 0.56;
  const n = Math.max(nodes.length, 1);

  let entities = scene.entities.filter((e) => e.kind === 'avatar' || !e.graphNodeId);
  const avatar = entities.find((e) => e.id === 'avatar-player') || createAvatar(bounds.width * 0.5, bounds.height * 0.75);

  nodes.forEach((node, i) => {
    const t = nodes.length === 1 ? 0.5 : i / (n - 1);
    const x = startX + span * t;
    const y = baseY + Math.sin(t * Math.PI) * 40 + (i % 2) * 18;
    entities = spawnEntity(entities, entityFromGraphNode(node, x, y));
  });

  // ensure single avatar
  entities = entities.filter((e) => e.id !== 'avatar-player');
  entities = spawnEntity(entities, avatar);

  return {
    ...scene,
    entities,
  };
}

/**
 * Apply session focus → entity flags + avatar seeks focused beacon.
 * @param {object} scene
 * @param {object} session
 * @param {{ seekSpeed?: number }} [opts]
 */
export function syncSceneFromSession(scene, session, opts = {}) {
  if (!session) return scene;
  const focused = session.nodes?.[session.focusIndex];
  const focusedId = focused?.id || null;
  const selectedId = session.selectedId || null;

  let entities = syncFocusFlags(scene.entities, focusedId, selectedId);

  if (focusedId) {
    const target = findByGraphId(entities, focusedId);
    const avatarIdx = entities.findIndex((e) => e.id === 'avatar-player');
    if (target && avatarIdx >= 0) {
      const speed = opts.seekSpeed ?? 6;
      // stand slightly below beacon
      entities[avatarIdx] = seekEntity(
        entities[avatarIdx],
        target.x,
        target.y + 36,
        speed,
      );
    }
  }

  return {
    ...scene,
    entities,
    time: (scene.time || 0) + 1,
  };
}

/**
 * Drawable snapshot for renderers (stable contract).
 * @param {object} scene
 */
export function sceneDrawList(scene) {
  const env = scene.environment;
  const props = sortProps(scene.props || []);
  const entities = [...(scene.entities || [])].sort((a, b) => (a.z ?? a.y) - (b.z ?? b.y));
  return {
    bounds: scene.bounds,
    camera: scene.camera,
    environment: env,
    layers: env.layers || [],
    props,
    entities,
    time: scene.time || 0,
  };
}

/**
 * How to add content without rewrite (foundation API).
 */
export function foundationApiSummary() {
  return {
    addBiome: 'createEnvironment(biomeId) + setBiome',
    addSprite: 'SPRITE_ATLAS entry in sprites.js',
    addProp: 'createProp(kind,x,y) + addProp(scene.props, prop)',
    bindData: 'bindGraphToScene(scene, session.nodes)',
    sync: 'syncSceneFromSession(scene, session)',
  };
}
