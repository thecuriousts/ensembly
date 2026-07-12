/**
 * Thin WASM host — all world sim lives in peram-core (Rust).
 * JS only loads the module and forwards bytes to the renderer.
 */

let wasm = null;
let mode = 'uninitialized';

export function engineMode() {
  return mode;
}

export async function initEngine() {
  try {
    const mod = await import('/game/pkg/peram_core.js');
    await mod.default();
    wasm = mod;
    mode = 'wasm';
    // reset default courtyard
    if (mod.world_reset) mod.world_reset(1600, 900);
    return { mode, version: mod.engine_version() };
  } catch (err) {
    mode = 'js-fallback';
    wasm = null;
    return {
      mode,
      version: 'js-fallback (build wasm: npm run build:wasm)',
      error: String(err?.message || err),
    };
  }
}

export function worldReset(w = 1600, h = 900) {
  if (wasm?.world_reset) wasm.world_reset(w, h);
}

/**
 * @param {Array<{id:string,type?:string,realm?:string,hitl?:boolean,label?:string}>} nodes
 */
export function worldBindGraph(nodes = []) {
  if (!wasm?.world_bind_graph) return false;
  const types = nodes.map((n) => n.type || 'action').join('\n');
  const realms = nodes.map((n) => n.realm || '').join('\n');
  const ids = nodes.map((n) => n.id || 'x').join('\n');
  let mask = 0;
  nodes.forEach((n, i) => {
    if (i < 32 && (n.hitl || n.type === 'hitl')) mask |= 1 << i;
  });
  wasm.world_bind_graph(types, realms, ids, mask);
  return true;
}

export function worldFocusNext() {
  wasm?.world_focus_next?.();
}

export function worldFocusPrev() {
  wasm?.world_focus_prev?.();
}

export function worldSetFocus(slot) {
  wasm?.world_set_focus?.(slot | 0);
}

export function worldFocusSlot() {
  return wasm?.world_focus_slot?.() ?? 0;
}

export function worldTick(speed = 5) {
  wasm?.world_tick?.(speed);
}

/** @returns {Float32Array|number[]} */
export function worldDrawBuffer() {
  if (wasm?.world_draw_buffer) return Array.from(wasm.world_draw_buffer());
  return [];
}

export function worldEntityCount() {
  return wasm?.world_entity_count?.() ?? 0;
}

export function worldPropCount() {
  return wasm?.world_prop_count?.() ?? 0;
}

// legacy layout helpers (still used if needed)
export function packGrid(count, cols = 4, colW = 220, rowH = 90) {
  if (wasm?.pack_grid) return Array.from(wasm.pack_grid(count, cols, colW, rowH));
  const out = [];
  for (let i = 0; i < count; i++) out.push((i % cols) * colW, Math.floor(i / cols) * rowH);
  return out;
}

export function layoutTick(positions, strength = 0.8) {
  if (wasm?.layout_tick) {
    return Array.from(wasm.layout_tick(new Float32Array(positions), strength));
  }
  return positions.slice();
}
