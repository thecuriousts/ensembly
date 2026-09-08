# ensembly — operator kernel

**ensembly** is a **thin, complementary operator kernel**: durable HITL/HOOTL authorize/claim/complete, a T1 SQLite ledger, episodic memory, and pulse-pack sync — designed to sit **under** Grok Bot, Grok Build, Cursor, and other capture harnesses. Not a second chat OS.

Product law: [PRODUCT-CHARTER.md](docs/PRODUCT-CHARTER.md) · [AGENTS.md](AGENTS.md) · cut record: [MUSK-CUT-2026-09-04.md](docs/MUSK-CUT-2026-09-04.md)

**New here?** [docs/MAP.md](docs/MAP.md) — live crates, CLI surfaces, pulse sync. Acronyms: [docs/GLOSSARY.md](docs/GLOSSARY.md). Crate rename: [RENAME.md](RENAME.md).

**life-os vs this repo:** The **life-os vault** (`$LIFEOS`) is the clustered Projects/Areas portfolio. **ensembly** is the **digital clone kernel** — local authorize/claim state and memory you own. See [LIFE-OS-BOUNDARY.md](docs/LIFE-OS-BOUNDARY.md).

**Multi-device hand:** Pair with [participatory-mesh](https://github.com/thecuriousts/participatory-mesh) when an allowlisted machine act should run on **another participant**. Bots/harnesses propose here; ensembly authorizes and claims; CommandFabric dispatches across the private mesh. Honest scope: mesh does not own the life ledger — it multiplies where ensembly-approved work can execute. Bridge: [docs/thinking/mesh-bridge-2026-09-08.md](docs/thinking/mesh-bridge-2026-09-08.md).

**Parked prototype:** Game of Peram browser client, Node `swarm.js` stack, WASM world sim → [`prototype/`](prototype/README.md) (not SoT).

---

## Drop in

**Prereq:** Rust toolchain (`cargo`) only.

```bash
cargo test -p ensembly-kernel
cargo test -p ensembly-memory
cargo build -p ensembly-agents --bin ensembly-mcp
```

### Runtime dogfood (Issue #1 SoT)

```bash
cargo run -p ensembly-kernel -- runtime load --fixture fixtures/issue-1-runtime.json
cargo run -p ensembly-kernel -- runtime status

# One HOOTL step per tick (claim *or* complete)
cargo run -p ensembly-kernel -- runtime tick
cargo run -p ensembly-kernel -- runtime tick
cargo run -p ensembly-kernel -- runtime tick
cargo run -p ensembly-kernel -- runtime tick

# HITL: action id (pay-rent), not auth- prefix
cargo run -p ensembly-kernel -- runtime approve pay-rent
cargo run -p ensembly-kernel -- runtime claim grocery-errand
cargo run -p ensembly-kernel -- runtime complete grocery-errand
cargo run -p ensembly-kernel -- runtime reflect
```

Fresh default DB: `data/local/ensembly-ops.sqlite` (gitignored). Episodic memory: `data/local/ensembly-memory.json`. Existing `peram-ops.sqlite` / `peram-memory.json` are opened in place — no silent migrate. Law: [DECISIONS.md](docs/DECISIONS.md) · [RENAME.md](RENAME.md).

### Pulse pack (bot ↔ laptop, memory only)

```bash
cargo run -p ensembly-kernel -- pulse-pack export --out /tmp/session.pulse.json
cargo run -p ensembly-kernel -- pulse-pack status --pack /tmp/session.pulse.json
cargo run -p ensembly-kernel -- pulse-pack import --pack /tmp/session.pulse.json
```

Topology: **Grok Bot = canonical kernel host** (single writer on ops DB). Laptop imports pulse packs — no dual-write. Recipe: [PLAYBOOK.md](docs/PLAYBOOK.md).

### Agent wire (read-only)

```bash
cargo build -p ensembly-agents --bin ensembly-mcp
# Register with Grok: grok mcp add --scope project ensembly -- cargo run -p ensembly-agents --bin ensembly-mcp
```

---

## Repo map

```text
crates/ensembly-kernel/   control SoT: life-state S · DepGraph G · CP+P · MsgBus · HITL/HOOTL · T1 SQLite · backup · pulse-pack
crates/ensembly-memory/   episodic CRDT (aux learning; kernel never delegates control)
crates/ensembly-agents/   ensembly-mcp read-only satellite for Grok/Cursor
fixtures/              issue-1-runtime.json · state-sample.json · …
docs/                  charter · MAP · PLAYBOOK · privacy · decisions · mesh bridge
prototype/             parked game/watch/Node stack (not maintained as product)
# companion (external): participatory-mesh — CommandFabric across private-network peers
```

---

## Operator playbook

Full dogfood guide: **[docs/PLAYBOOK.md](docs/PLAYBOOK.md)** — runtime HITL/HOOTL, pulse sync, harness fit. Multi-device dispatch: **[participatory-mesh](https://github.com/thecuriousts/participatory-mesh)** + [mesh bridge](docs/thinking/mesh-bridge-2026-09-08.md).

---

## Privacy

Never push: `private/`, `data/`, secrets. Rules: [PRIVACY.md](docs/PRIVACY.md).

---

## License

MIT ([LICENSE.md](LICENSE.md)). Your private life data is not part of the grant.

**Rule:** automate the digital · surface the physical · wait only for permission · complement the harness.
