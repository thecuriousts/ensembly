# ensembly — operator kernel

**ensembly** is a **thin, complementary operator kernel**: durable HITL/HOOTL gates, T1 SQLite ledger, episodic memory, and pulse-pack sync — designed to sit **under** Grok Bot, Grok Build, Cursor, and other capture harnesses. Not a second chat OS.

Product law: [PRODUCT-CHARTER.md](docs/PRODUCT-CHARTER.md) · [AGENTS.md](AGENTS.md) · cut record: [MUSK-CUT-2026-09-04.md](docs/MUSK-CUT-2026-09-04.md)

**New here?** [docs/MAP.md](docs/MAP.md) — live crates, CLI surfaces, pulse sync. Acronyms: [docs/GLOSSARY.md](docs/GLOSSARY.md).

**life-os vs this repo:** `~/life-os` is the clustered Projects/Areas **vault**. **ensembly** is the **digital clone kernel** — local gates and memory you own. See [LIFE-OS-BOUNDARY.md](docs/LIFE-OS-BOUNDARY.md).

**Parked prototype:** Game of Peram browser client, Node `swarm.js` stack, WASM world sim → [`prototype/`](prototype/README.md) (not SoT).

---

## Drop in

**Prereq:** Rust toolchain (`cargo`) only.

```bash
cargo test -p peram-kernel
cargo test -p peram-memory
cargo build -p peram-agents --bin peram-mcp
```

### Runtime dogfood (Issue #1 SoT)

```bash
cargo run -p peram-kernel -- runtime load --fixture fixtures/issue-1-runtime.json
cargo run -p peram-kernel -- runtime status

# One HOOTL step per tick (claim *or* complete)
cargo run -p peram-kernel -- runtime tick
cargo run -p peram-kernel -- runtime tick
cargo run -p peram-kernel -- runtime tick
cargo run -p peram-kernel -- runtime tick

# HITL: action id (pay-rent), not auth- prefix
cargo run -p peram-kernel -- runtime approve pay-rent
cargo run -p peram-kernel -- runtime claim grocery-errand
cargo run -p peram-kernel -- runtime complete grocery-errand
cargo run -p peram-kernel -- runtime reflect
```

Default durable DB: `data/local/peram-ops.sqlite` (gitignored). Episodic memory: `data/local/peram-memory.json`. Law: [DECISIONS.md](docs/DECISIONS.md).

### Pulse pack (bot ↔ laptop, memory only)

```bash
cargo run -p peram-kernel -- pulse-pack export --out /tmp/session.pulse.json
cargo run -p peram-kernel -- pulse-pack status --pack /tmp/session.pulse.json
cargo run -p peram-kernel -- pulse-pack import --pack /tmp/session.pulse.json
```

Topology: **Grok Bot = canonical kernel host** (single writer on ops DB). Laptop imports pulse packs — no dual-write. Recipe: [PLAYBOOK.md](docs/PLAYBOOK.md).

### Agent wire (read-only)

```bash
cargo build -p peram-agents --bin peram-mcp
# Register with Grok: grok mcp add --scope project peram -- cargo run -p peram-agents --bin peram-mcp
```

---

## Repo map

```text
crates/peram-kernel/   control SoT: life-state S · DepGraph G · CP+P · MsgBus · HITL/HOOTL · T1 SQLite · backup · pulse-pack
crates/peram-memory/   episodic CRDT (aux learning; kernel never delegates control)
crates/peram-agents/   peram-mcp read-only satellite for Grok/Cursor
fixtures/              issue-1-runtime.json · state-sample.json · …
docs/                  charter · MAP · PLAYBOOK · privacy · decisions
prototype/             parked game/watch/Node stack (not maintained as product)
```

---

## Operator playbook

Full dogfood guide: **[docs/PLAYBOOK.md](docs/PLAYBOOK.md)** — runtime HITL/HOOTL, pulse sync, harness fit.

---

## Privacy

Never push: `private/`, `data/`, secrets. Rules: [PRIVACY.md](docs/PRIVACY.md).

---

## License

MIT ([LICENSE.md](LICENSE.md)). Your private life data is not part of the grant.

**Rule:** automate the digital · surface the physical · wait only for permission · complement the harness.
