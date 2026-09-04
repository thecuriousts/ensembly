import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  createEnvironment,
  setBiome,
  createScene,
  bindGraphToScene,
  syncSceneFromSession,
  sceneDrawList,
  spawnEntity,
  createEntity,
  seekEntity,
  addProp,
  createProp,
  listSpriteIds,
  spriteForNode,
  foundationApiSummary,
} from '../src/game/world/index.js';
import { createSession, dispatch } from '../src/game/session.js';

describe('world foundation (shipped)', () => {
  it('createEnvironment has layers sky→entities and palette', () => {
    const env = createEnvironment('night_courtyard');
    assert.equal(env.biome, 'night_courtyard');
    assert.ok(env.layers.includes('sky'));
    assert.ok(env.layers.includes('props'));
    assert.ok(env.layers.includes('entities'));
    assert.ok(env.palette.skyTop);
  });

  it('setBiome switches environment identity', () => {
    let env = createEnvironment('night_courtyard');
    env = setBiome(env, 'ember_hall');
    assert.equal(env.biome, 'ember_hall');
  });

  it('createScene has props and avatar entity', () => {
    const scene = createScene();
    assert.ok(scene.props.length >= 5, 'courtyard props');
    assert.ok(scene.entities.some((e) => e.id === 'avatar-player'));
    assert.ok(scene.environment.layers.length >= 5);
  });

  it('bindGraphToScene spawns beacon entities for nodes', () => {
    const nodes = [
      { id: 'a1', type: 'action', label: 'Ship' },
      { id: 'p1', type: 'physical', realm: 'physical', label: 'Walk' },
      { id: 'h1', type: 'hitl', hitl: true, label: 'Gate' },
    ];
    let scene = createScene();
    scene = bindGraphToScene(scene, nodes);
    const beacons = scene.entities.filter((e) => e.graphNodeId);
    assert.equal(beacons.length, 3);
    assert.ok(scene.entities.some((e) => e.spriteId === 'beacon_physical'));
    assert.ok(scene.entities.some((e) => e.spriteId === 'beacon_hitl'));
  });

  it('syncSceneFromSession moves avatar and sets focus flags', () => {
    const nodes = [
      { id: 'a1', type: 'action', label: 'A' },
      { id: 'b1', type: 'action', label: 'B' },
    ];
    let scene = bindGraphToScene(createScene(), nodes);
    let session = createSession({ nodes }, null);
    session = dispatch(session, { type: 'FOCUS_NEXT' });
    const before = scene.entities.find((e) => e.id === 'avatar-player');
    scene = syncSceneFromSession(scene, session, { seekSpeed: 100 });
    const after = scene.entities.find((e) => e.id === 'avatar-player');
    const focusedBeacon = scene.entities.find((e) => e.focused);
    assert.ok(focusedBeacon, 'a beacon should be focused');
    assert.equal(focusedBeacon.graphNodeId, session.nodes[session.focusIndex].id);
    // avatar sought toward beacon (may snap if speed high)
    assert.ok(
      Math.hypot(after.x - before.x, after.y - before.y) > 0 ||
        Math.hypot(after.x - focusedBeacon.x, after.y - (focusedBeacon.y + 36)) < 1,
    );
  });

  it('sceneDrawList exposes layers props entities for renderer', () => {
    const scene = bindGraphToScene(createScene(), [
      { id: 'x', type: 'phase', label: 'ORIENT' },
    ]);
    const list = sceneDrawList(scene);
    assert.ok(list.environment.palette);
    assert.ok(list.props.length > 0);
    assert.ok(list.entities.length > 0);
    assert.ok(Array.isArray(list.layers));
  });

  it('spawnEntity and seekEntity mutate world state', () => {
    let ents = [];
    ents = spawnEntity(ents, createEntity({ id: 'e1', x: 0, y: 0, spriteId: 'beacon_digital' }));
    assert.equal(ents.length, 1);
    const moved = seekEntity(ents[0], 100, 0, 10);
    assert.ok(moved.x > 0);
    assert.ok(moved.x <= 100);
  });

  it('addProp grows prop list', () => {
    let props = [];
    props = addProp(props, createProp('lantern', 10, 20, { id: 'L1' }));
    props = addProp(props, createProp('tree', 30, 40, { id: 'T1' }));
    assert.equal(props.length, 2);
  });

  it('sprite atlas has avatar and beacons', () => {
    const ids = listSpriteIds();
    assert.ok(ids.includes('avatar_walker'));
    assert.ok(ids.includes('beacon_hitl'));
    assert.equal(spriteForNode({ type: 'physical' }), 'beacon_physical');
  });

  it('foundationApiSummary documents extension points', () => {
    const api = foundationApiSummary();
    assert.ok(api.addSprite);
    assert.ok(api.addProp);
    assert.ok(api.bindData);
  });
});
