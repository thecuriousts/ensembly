/**
 * World foundation barrel — environment, sprites, props, scene.
 */
export {
  createEnvironment,
  defaultBounds,
  setBiome,
} from './environment.js';
export {
  SPRITE_ATLAS,
  getSprite,
  spriteForNode,
  listSpriteIds,
} from './sprites.js';
export {
  createProp,
  defaultCourtyardProps,
  addProp,
  removeProp,
  sortProps,
} from './props.js';
export {
  createEntity,
  createAvatar,
  entityFromGraphNode,
  spawnEntity,
  seekEntity,
  syncFocusFlags,
  findByGraphId,
} from './entity.js';
export {
  createScene,
  bindGraphToScene,
  syncSceneFromSession,
  sceneDrawList,
  foundationApiSummary,
} from './scene.js';
