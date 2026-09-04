//! peram-core — shared Game of Peram engine
//!
//! **Host-agnostic:** `rlib` for native/desktop later; `cdylib` + wasm-bindgen for browser.
//! JS shell stays thin: input, canvas paint, load graph JSON.
//!
//! X-trend inspiration (2026): interactive world models + agent sims want a real
//! engine for state (Rust/WASM), not logic sprawled in the DOM layer.
//! Examples in the wild: Rust WASM world gens, WebGPU+WASM engines — same split.

mod layout;
mod world;

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

use layout::{layout_tick as layout_tick_inner, pack_grid as pack_grid_inner};
use world::{kind_from_type, World, KIND_BEACON_DIGITAL};

thread_local! {
    static WORLD: RefCell<World> = RefCell::new(World::new_courtyard(1600.0, 900.0));
}

/// Engine banner for feature detect / desktop parity.
#[wasm_bindgen]
pub fn engine_version() -> String {
    "peram-core 0.2.0 rust-world-sim wasm+native".to_string()
}

#[wasm_bindgen]
pub fn pack_grid(count: u32, cols: u32, col_w: f32, row_h: f32) -> Vec<f32> {
    pack_grid_inner(count, cols, col_w, row_h)
}

#[wasm_bindgen]
pub fn layout_tick(positions: &[f32], strength: f32) -> Vec<f32> {
    layout_tick_inner(positions, strength)
}

#[wasm_bindgen]
pub fn hash_id(id: &str) -> u32 {
    world::fnv1a(id.as_bytes())
}

/// Reset world to empty courtyard with avatar + default props.
#[wasm_bindgen]
pub fn world_reset(width: f32, height: f32) {
    WORLD.with(|w| {
        *w.borrow_mut() = World::new_courtyard(width.max(320.0), height.max(240.0));
    });
}

/// Bind graph nodes.
/// `types_joined` — types separated by `\n`
/// `realms_joined` — realms separated by `\n` (same length)
/// `ids_joined` — ids separated by `\n`
/// `hitl_mask` — bit i set if node i is HITL (as u32, first 32 nodes)
#[wasm_bindgen]
pub fn world_bind_graph(types_joined: &str, realms_joined: &str, ids_joined: &str, hitl_mask: u32) {
    let types: Vec<&str> = types_joined.split('\n').filter(|s| !s.is_empty()).collect();
    let realms: Vec<&str> = realms_joined.split('\n').collect();
    let ids: Vec<&str> = ids_joined.split('\n').filter(|s| !s.is_empty()).collect();
    let n = types.len().min(ids.len());
    let mut kinds = Vec::with_capacity(n);
    let mut hashes = Vec::with_capacity(n);
    for i in 0..n {
        let hitl = (hitl_mask & (1u32 << i)) != 0;
        let realm = realms.get(i).copied().unwrap_or("");
        kinds.push(kind_from_type(types[i], realm, hitl));
        hashes.push(world::fnv1a(ids[i].as_bytes()));
    }
    if kinds.is_empty() {
        kinds.push(KIND_BEACON_DIGITAL);
        hashes.push(0);
    }
    WORLD.with(|w| w.borrow_mut().bind_beacons(&kinds, &hashes));
}

#[wasm_bindgen]
pub fn world_focus_next() {
    WORLD.with(|w| w.borrow_mut().focus_next());
}

#[wasm_bindgen]
pub fn world_focus_prev() {
    WORLD.with(|w| w.borrow_mut().focus_prev());
}

#[wasm_bindgen]
pub fn world_set_focus(slot: i32) {
    WORLD.with(|w| w.borrow_mut().set_focus_slot(slot));
}

#[wasm_bindgen]
pub fn world_focus_slot() -> i32 {
    WORLD.with(|w| w.borrow().focus_slot)
}

#[wasm_bindgen]
pub fn world_tick(speed: f32) {
    WORLD.with(|w| w.borrow_mut().tick(speed.max(0.1)));
}

/// Flattened draw buffer — see world::World::draw_buffer docs.
#[wasm_bindgen]
pub fn world_draw_buffer() -> Vec<f32> {
    WORLD.with(|w| w.borrow().draw_buffer())
}

#[wasm_bindgen]
pub fn world_entity_count() -> u32 {
    WORLD.with(|w| w.borrow().entity_count())
}

#[wasm_bindgen]
pub fn world_prop_count() -> u32 {
    WORLD.with(|w| w.borrow().prop_count())
}

// Re-export pure API for native desktop crates later.
pub use layout::{layout_tick as layout_tick_native, pack_grid as pack_grid_native};
pub use world::{KIND_AVATAR, KIND_BEACON_HITL};

#[cfg(test)]
mod integration {
    use super::world::*;

    #[test]
    fn end_to_end_buffer() {
        let mut w = World::new_courtyard(1600.0, 900.0);
        w.bind_beacons(&[KIND_BEACON_DIGITAL, KIND_BEACON_HITL], &[11, 22]);
        w.tick(5.0);
        let b = w.draw_buffer();
        assert!(b[4] >= 2.0);
        assert!(b[5] >= 8.0);
    }
}
