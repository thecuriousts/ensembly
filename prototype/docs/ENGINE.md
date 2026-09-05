# Shared engine architecture (WASM now · desktop later)

## Inspiration (X / industry, 2026)

| Signal | Takeaway for ensembly |
|--------|------------------------|
| Rust+WASM in-browser world sims (e.g. generative life games with WorldGen in WASM) | **Sim lives in Rust**, host only paints + inputs |
| WebGPU + WASM engines (indie 3D engines in Rust) | Same crate targets browser now, native GPU later |
| Open interactive world models (LingBot-World, Matrix-Game, HunyuanWorld) | Agent-driven worlds need a **stable state engine** + thin presentation |
| Qwen-AgentWorld / language world models | Separate **control plane** (plans, HITL) from **world model** (place, entities) |
| Stack rebuilt for agents (orchestration + verification) | Keep day/turn/privacy as control; world as play surface |

We do **not** ship a 14B neural world model here — we ship a **shareable deterministic world engine** that can later feed or sit beside neural generators.

## Split (restart 2026-07-15)

```text
┌─────────────────────────────────────────────┐
│  Host shell (thin)                          │
│  primary trajectory: native peram-console   │
│  browser: public/game/*.js  (thin host)     │
└─────────────────┬───────────────────────────┘
                  │ draw_buffer / FocusPlan bind
┌─────────────────▼───────────────────────────┐
│  peram-core / world (Rust)                  │
│  world sim · layout · entity seek · props   │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│  ensembly-kernel (Rust)  ← control SoT         │
│  privacy · realm · approvals · rank_now     │
│  digital_flow · T1 SQLite · backup · vault  │
│  Issue #1: S · G · CP+P · MsgBus · HITL     │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│  Operator CLI — bin/swarm.js + src/*        │
│  day · turn · graph · game session          │
│  control-plane features expand in kernel    │
└─────────────────────────────────────────────┘
```

**Law:** [DECISIONS.md](DECISIONS.md) · Rust life-console restart.

## Crate layout

```text
crates/peram-core/
  src/lib.rs      wasm exports + re-exports
  src/world.rs    World, entities, props, tick, draw_buffer ABI
  src/layout.rs   pack_grid, layout_tick
```

**Native desktop later:** depend on `peram-core` as a normal Rust library (`World` API), render with wgpu/egui — no rewrite of sim.

## Draw buffer ABI (host-agnostic)

```
[time, width, height, focus_slot, ent_count, prop_count,
  entities×: kind, flags, x, y, graph_slot, id_hash,
  props×: kind, x, y, scale]
```

JS canvas painter (`world-render.js`) only interprets this — no game rules.

## Commands

```bash
# Control SoT (Issue #1 dogfood)
cargo test -p ensembly-kernel
cargo run -p ensembly-kernel -- runtime load --fixture fixtures/issue-1-runtime.json
cargo run -p ensembly-kernel -- runtime status
# or: npm run peram -- runtime status

# World / game
npm run build:wasm    # → public/game/pkg
cargo test -p peram-core
npm run game          # thin shell + WASM world
```

Operator path: [PLAYBOOK.md](PLAYBOOK.md) · orientation: [MAP.md](MAP.md).

## JS bloat policy

| Keep in JS | Keep in Rust |
|------------|--------------|
| DOM, keyboard, voice API | Entity positions, seek, props, bind |
| HITL session store (Node CLI parity) | Layout, draw snapshot |
| Optional Eve cloud bridge (channels/schedules) — see [EVE-FIT.md](EVE-FIT.md) | — not in engine |
| Graph JSON fetch | Simulation tick |

When adding features: prefer **Rust world API + buffer fields** over new JS sim modules.
