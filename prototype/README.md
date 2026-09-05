# Prototype — parked Game of Peram surfaces

**Status:** Not source of truth. History preserved; not deleted.

ensembly at repo root is now a **thin operator kernel** (`ensembly-kernel`, `ensembly-memory`, `ensembly-agents`). Everything under `prototype/` is the former **Game of Peram / Node operator theater** — browser game, watch HTML, WASM world sim, and the legacy `swarm.js` CLI stack.

## What lives here

| Path | Was | Role (prototype only) |
|------|-----|------------------------|
| `bin/swarm.js` + `src/` | Root operator CLI | Day/turn/graph/watch via Node wait-snapshot IR |
| `public/game/` | Browser game host | Courtyard world, $SPN, focus session |
| `public/watch/` | Static glance UI | Graph + turn-status consumer |
| `public/events/`, `public/persona/`, `public/thinking/` | Public projections | Examples and thinking helpers |
| `crates/peram-core/` | Workspace member | Rust → WASM world sim (mirrors focus) |
| `scripts/` | Root scripts | `serve-game.mjs`, `build-wasm.sh`, smoke |
| `test/` | Node unit tests | Pure-module tests for parked stack |
| `docs/` | Game stretch docs | GAME-STACK, ENGINE, WORLD-FOUNDATION, SWARM-DESIGN, … |
| `arch-design/` | Formal stretch | AppGenMathPhyLang, virtual-life handler notes |

## Control plane (repo root — use this)

```bash
cargo test -p ensembly-kernel
cargo test -p ensembly-memory
cargo build -p ensembly-agents --bin ensembly-mcp

cargo run -p ensembly-kernel -- runtime load --fixture fixtures/issue-1-runtime.json
cargo run -p ensembly-kernel -- runtime status
cargo run -p ensembly-kernel -- runtime tick
cargo run -p ensembly-kernel -- pulse-pack export --out /tmp/pulse.pulse.json
```

HITL/HOOTL, T1 SQLite, privacy, backup, pulse-pack, and `ensembly-mcp` are **only** maintained at repo root.

## Running the parked game (optional)

Requires Node ≥ 22.5. From this directory:

```bash
cd prototype
npm test
npm run game    # → http://127.0.0.1:4173/game/
```

Do not treat game focus, wait-snapshot JSON, or watch exports as canonical state. The kernel owns gates, ops DB, and pulse memory.

## Why parked

See [docs/MUSK-CUT-2026-09-04.md](../docs/MUSK-CUT-2026-09-04.md) — Musk 5-step cut + Odysseus core-nail: complement Grok Bot / Grok Build / other harnesses with a durable operator kernel, not a second chat OS or dual live writers.
