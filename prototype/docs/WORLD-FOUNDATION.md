# World foundation — Game of Peram

**Why:** Prior surface read as software telemetry. World is primary; HUD is glass chrome.

**Engine of record:** Rust `crates/peram-core` (WASM in browser, `rlib` for desktop later).  
See [ENGINE.md](./ENGINE.md) for host split and X-inspired direction.

## Layers

| Layer | Where | Responsibility |
|-------|--------|----------------|
| World sim | `crates/peram-core/src/world.rs` | Entities, props, seek, focus, draw_buffer |
| Layout | `crates/peram-core/src/layout.rs` | Grid pack, repulsion tick |
| Canvas paint | `public/game/world-render.js` | Thin: buffer → pixels only |
| Host shell | `public/game/main.js` | Input, HITL store, labels |
| JS pure (tests / Node) | `src/game/world/*` | Reference model; not required at runtime if WASM loads |

## Add content

- **Prop/entity kinds:** extend Rust `world.rs` constants + `courtyard_props` / `bind_beacons`
- **Paint:** `world-render.js` switch arms for new kind codes
- **Desktop:** `use peram_core::World` in a native binary — same sim

## Launch

```bash
npm run build:wasm
npm run game   # http://127.0.0.1:4173/game/
```

Tab · avatar seeks beacon · Enter **claim** (+XP) · A/D HITL gates (session store).  
**Growth loop:** pure `src/game/growth.js` — physical / presence / craft / HITL quests, levels, streak combo, balance chips. World path glows with growth meter; claimed beacons mark green.
