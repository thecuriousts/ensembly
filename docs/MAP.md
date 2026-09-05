# System map — operator kernel

**Audience:** new readers, agents, operator.  
**Not this doc:** parked game (`prototype/README.md`), cut rationale (`MUSK-CUT-2026-09-04.md`), privacy law (`PRIVACY.md`).

**Last verified:** 2026-09-05 (ensembly-* crate rename)

---

## 0. One sentence

**ensembly** is a **local operator kernel**: Rust control plane for HITL/HOOTL runtime, T1 SQLite, episodic memory, and pulse-pack sync — complementing Grok/Cursor harnesses without pushing private life data.

```text
Kernel (ensembly-kernel)     → gates, MsgBus, ops DB, backup, pulse-pack
Memory (ensembly-memory)     → CRDT episodic layer (aux; never decides)
Agents (ensembly-agents)     → read-only ensembly-mcp for harnesses
Harness (Grok/Cursor/…)   → capture — calls kernel, never owns vault
Prototype (parked)        → game/watch/Node — not SoT
```

---

## 1. Live capabilities

Only features with a **dogfood path** today.

### 1.1 `crates/ensembly-kernel`

| Capability | Entry | What you get |
|------------|-------|--------------|
| **Runtime S+G+CP+P** | `cargo run -p ensembly-kernel -- runtime load\|status\|tick\|approve\|deny\|claim\|complete` | Life-state **S**, DepGraph **G**, CP+P; MsgBus; HOOTL agents; HITL gates. DB: `data/local/ensembly-ops.sqlite` (legacy `peram-ops.sqlite` discovered if present) |
| **UncertaintyDive** | `runtime dive [--json]` | Inspect-only epistemic probe over G+CP |
| **Reflect** | `runtime reflect [--json]` | Coherence + skills + goals over episodic memory |
| **Turn / rank** | `ensembly turn [--fixture]` | FocusPlan from critical path |
| **Digital-flow** | `ensembly digital-flow cycle` | bill_pay dry-run through HITL gate |
| **Backup / restore** | `backup`, `restore-dry-run`, `restore-apply` | Sealed T2 pack |
| **Ops bundle** | `ops-bundle` | Unsealed portable ops snapshot |
| **Pulse pack** | `pulse-pack export\|import\|status` | New writes `ensembly-pulse-pack-v1`; still imports `peram-pulse-pack-v1` — **memory only**; no ops dual-write |

Fixture: `fixtures/issue-1-runtime.json`. `--json` appends trailing `RUNTIME_OK …` line.

### 1.2 `crates/ensembly-memory`

| Capability | Entry | What you get |
|------------|-------|--------------|
| **CRDT episodic store** | via kernel `memory_sink` | Durable trajectory at `data/local/ensembly-memory.json` (legacy `peram-memory.json` discovered if present) |
| **Tests** | `cargo test -p ensembly-memory` | Merge, coherence, persistence |

Memory is **auxiliary** — kernel never delegates control decisions to it.

### 1.3 `crates/ensembly-agents`

| Capability | Entry | What you get |
|------------|-------|--------------|
| **ensembly-mcp** | `cargo build -p ensembly-agents --bin ensembly-mcp` | Read-only MCP tools for Grok/Cursor |
| **Tests** | `cargo test -p ensembly-agents` | RPC, inference stubs |

---

## 2. Pulse sync topology

| Host | Role | Writable |
|------|------|----------|
| **Grok Bot** (canonical) | Kernel SoT, runtime tick, sealed backup | `ensembly-ops.sqlite` / `ensembly-memory.json` (legacy `peram-*` discovered) |
| **Laptop** (client) | Offline reflect, harness dev | Import pulse → CRDT merge only |

```bash
# Canonical host — after session
cargo run -p ensembly-kernel -- pulse-pack export --out ~/sync/pulse/bot.pulse.json --include-archive

# Client — idempotent merge
cargo run -p ensembly-kernel -- pulse-pack import --pack ~/sync/pulse/bot.pulse.json
```

**Not live:** Drive automation (next). See [PLAYBOOK.md](PLAYBOOK.md).

---

## 3. Layer ownership

| Path | Owns |
|------|------|
| `crates/ensembly-kernel/` | Control SoT: S, G, CP, MsgBus, runtime, store, privacy, backup, pulse-pack |
| `crates/ensembly-memory/` | Episodic CRDT, coherence engine |
| `crates/ensembly-agents/` | MCP satellite, inference provider trait |
| `fixtures/` | Committed runtime/turn fixtures |
| `data/local/` | Operator DB + memory (gitignored) |
| `prototype/` | **Parked** — game, Node CLI, WASM — not product |

---

## 4. What “IR” means here

**IR = Intermediate Representation** — versioned JSON (or CLI `--json`) between kernel and harnesses.

| IR | Produced by | Notes |
|----|-------------|-------|
| Runtime status | `runtime status --json` | Regime, pending gates, CP |
| Turn / FocusPlan | `ensembly turn --json` | Next physical + auth from CP |
| **Channel pulse** | `ensembly turn --channel` · `channel-pulse reconcile` | Redacted one body + one gate; see [PLAYBOOK.md §5](PLAYBOOK.md#5-channel-pulse-issue-8) |
| Pulse pack | `pulse-pack export` | Portable memory + archive events |
| Ops bundle | `ops-bundle` | Full ops snapshot (canonical host only) |

Legacy Node turn/graph IR lives in `prototype/` — not maintained at root.

---

## 5. Dogfood commands

```bash
cargo test -p ensembly-kernel
cargo test -p ensembly-memory
cargo build -p ensembly-agents --bin ensembly-mcp

cargo run -p ensembly-kernel -- runtime load --fixture fixtures/issue-1-runtime.json
cargo run -p ensembly-kernel -- runtime status
cargo run -p ensembly-kernel -- runtime tick
cargo run -p ensembly-kernel -- runtime approve pay-rent
cargo run -p ensembly-kernel -- runtime reflect

# Isolated channel-pulse reconcile (fixtures only; does not write G)
cargo run -p ensembly-kernel -- --db /tmp/peram-ops-smoke.sqlite channel-pulse reconcile \
  --fixture fixtures/issue-1-runtime.json --out /tmp/channel-pulse.json --json
```

---

## 6. Where to go next

| Need | Doc |
|------|-----|
| Dogfood recipes | [PLAYBOOK.md](PLAYBOOK.md) |
| Musk cut / what was parked | [MUSK-CUT-2026-09-04.md](MUSK-CUT-2026-09-04.md) |
| Parked game stack | [../prototype/README.md](../prototype/README.md) |
| Privacy | [PRIVACY.md](PRIVACY.md) |
| life-os boundary | [LIFE-OS-BOUNDARY.md](LIFE-OS-BOUNDARY.md) |
| Decisions log | [DECISIONS.md](DECISIONS.md) |
| Crate rename | [RENAME.md](../RENAME.md) |

**Footer:** If it is not in §1, it is trajectory or prototype until it has a `cargo` path and tests.
