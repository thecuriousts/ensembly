# Game of Peram — race-car stack decision

**Method:** higher-order-decision-architect · fusion-sage · ai-optimization  
**Date:** 2026-07-13

## Executive verdict

**Hot path = Rust → `wasm32-unknown-unknown` (wasm-bindgen) for layout/sim; presentation = WebGPU with Canvas2D fallback; shell = thin ESM JS for keyboard, voice (Web Speech / text proxy), and DOM HUD.** Do **not** rewrite day/privacy/turn control plane into the engine. Do **not** force WebNN without an on-device ML step (deferred).

## Critical zone

Interactive engagement must feel like a **game** (focus, select, approve/deny, help, voice) while the **control plane** remains the audited Node pure functions. The race-car risk is putting policy in the renderer.

## First principles

- **Data plane vs control plane:** graph IR + turn snapshot = data; approve/deny/privacy = existing control.
- **Hot path only in WASM:** force-directed / grid layout ticks and node packing — O(n) / O(n²) math benefits from Rust+WASM; DOM chrome does not.
- **WebGPU** for GPU-accelerated node quads when available; **Canvas2D** keeps playability without GPU.
- **C++** would work (Emscripten) but we already have `rustc` + `wasm-pack` + `wasm32` target — lower friction, same performance class for this graph size.
- **WebNN:** no inference feature in this goal → explicitly deferred (not theater).

## Consequence chain

| Order | Effect | P | Impact |
|-------|--------|---|--------|
| 1 | Playable interactive surface | high | H |
| 2 | WASM build fails in some CI | med | M → check-in prebuilt pkg + build script |
| 3 | WebGPU missing in headless | high | M → Canvas2D fallback required |
| 3+ | Operator daily drives game room | med | H (2036 thrive) |

## Inversion / pre-mortem

- Fail: pure React SPA re-implements prioritization → **refused**.  
- Fail: Bevy full engine for a graph HUD → thrash.  
- Fail: claim WebNN without model → theater.  
- Fail: `file://` silent WASM fail → document `npm run game:serve`.

## Thrive ascent (2036)

Kernel: pure session + control plane. Bridge: WASM layout + WebGPU view. Boundary: human physical + HITL. Multiplayer voice room stays ascent SN.

## Refuse vs build

| Refuse | Build |
|--------|--------|
| Slow-script-only as sole path when WASM builds | Rust WASM layout hot path |
| Policy in the canvas | Graph IR feed + session actions |
| WebNN cosplay | Optional later local scoring |
| Native-only exclusive | Browser primary race-car |

**Near-term confidence:** 80%. **Thrive bet:** 75%.

## Fused abstraction

**Game session** = `createSession(graphIR)` + `dispatch(action)` ← keyboard | voice | gamepad-shaped.  
**Race engine** = `peram_core` WASM `layout_tick` / `pack_nodes`.  
**View** = WebGPU or Canvas2D consuming packed positions.

Trace: `src/game/session.js`, `crates/peram-core`, `public/game/`.
