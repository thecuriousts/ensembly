/**
 * Environment layer — sky, ground, biome backdrop (pure data).
 * Foundation for scaling biomes without touching control plane.
 */

/** @typedef {'night_courtyard' | 'dawn_garden' | 'ember_hall'} BiomeId */

/**
 * @param {BiomeId} [biome]
 */
export function createEnvironment(biome = 'night_courtyard') {
  const presets = {
    night_courtyard: {
      id: 'night_courtyard',
      name: 'Night Courtyard',
      skyTop: '#070b18',
      skyBottom: '#1a1430',
      ground: '#12181a',
      groundAccent: '#1c2820',
      path: '#2a2418',
      ambient: 0.35,
      stars: 48,
      moon: { x: 0.82, y: 0.14, r: 22 },
      fog: 0.12,
    },
    dawn_garden: {
      id: 'dawn_garden',
      name: 'Dawn Garden',
      skyTop: '#1a2040',
      skyBottom: '#c47850',
      ground: '#1a2218',
      groundAccent: '#243020',
      path: '#3a3020',
      ambient: 0.5,
      stars: 12,
      moon: null,
      fog: 0.08,
    },
    ember_hall: {
      id: 'ember_hall',
      name: 'Ember Hall',
      skyTop: '#10080c',
      skyBottom: '#2a1010',
      ground: '#141010',
      groundAccent: '#201818',
      path: '#2a1c14',
      ambient: 0.4,
      stars: 20,
      moon: { x: 0.2, y: 0.18, r: 16 },
      fog: 0.15,
    },
  };
  const p = presets[biome] || presets.night_courtyard;
  return {
    biome: p.id,
    name: p.name,
    palette: {
      skyTop: p.skyTop,
      skyBottom: p.skyBottom,
      ground: p.ground,
      groundAccent: p.groundAccent,
      path: p.path,
    },
    ambient: p.ambient,
    stars: p.stars,
    moon: p.moon,
    fog: p.fog,
    /** Draw order bottom → top */
    layers: Object.freeze(['sky', 'far', 'mid', 'ground', 'path', 'props', 'entities', 'fx']),
    horizonY: 0.42, // fraction of view height
  };
}

/**
 * World bounds for camera / spawn.
 */
export function defaultBounds() {
  return { width: 1600, height: 900, originX: 0, originY: 0 };
}

/**
 * @param {ReturnType<typeof createEnvironment>} env
 * @param {BiomeId} biome
 */
export function setBiome(env, biome) {
  const next = createEnvironment(biome);
  return { ...env, ...next };
}
