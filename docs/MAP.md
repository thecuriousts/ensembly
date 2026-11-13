# System map — operator kernel

**Audience:** new readers, agents, operator.  
**Not this doc:** parked game (`prototype/README.md`), cut rationale (`MUSK-CUT-2026-09-04.md`), privacy law (`PRIVACY.md`).

**Last verified:** 2026-09-08 (participatory-mesh value + proactive wording)

---

## 0. One sentence

**ensembly** is a **local operator kernel**: Rust control plane for HITL/HOOTL runtime, T1 SQLite, episodic memory, and pulse-pack sync — complementing Grok/Cursor harnesses without pushing private life data. Optional peer: [participatory-mesh](https://github.com/thecuriousts/participatory-mesh) for allowlisted acts on another device after authorize/claim.

```text
Kernel (ensembly-kernel)     → authorize/claim/complete, MsgBus, ops DB, backup, pulse-pack
Memory (ensembly-memory)     → CRDT episodic layer (aux; never decides)
Agents (ensembly-agents)     → memory tools plus record commit and view
Harness (Grok/Cursor/…)      → capture — proposes; never owns vault
participatory-mesh (external)→ CommandFabric — dispatches allowlisted peer acts
Prototype (parked)           → game/watch/Node — not SoT
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
| **Record log** | `ensembly record commit\|recover`, `ensembly record view snapshot\|feed`, `ensembly record apps import\|compare` | Commit and read the schema record log. `record apps import` appends one event per current application-ledger row (idempotency key is link key + stage; a repeat with another writer or clock appends nothing) and does not write the ledger. It refuses when replay would leave a stage different from the ledger. `record apps compare` replays the log, diffs stage and submit time against the ledger, exits 1 on a mismatch and 2 on a read error, and writes nothing. Reconcile and Sync stay out of MCP. MCP commit refuses source `human` and `owner`, stamps `record_time`, and only writes a sandbox path, `ENSEMBLY_RECORD_LOG`, or `data/local` under `ENSEMBLY_HOME` or the server cwd |

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
| **ensembly-mcp** | `cargo build -p ensembly-agents --bin ensembly-mcp` | Memory tools plus `record_commit`, `record_view_snapshot`, `record_view_feed` |
| **Tests** | `cargo test -p ensembly-agents` | RPC, inference stubs |

### 1.4 `crates/evalbench-core` and `crates/evalbench-cli`

| Capability | Entry | What you get |
|------------|-------|--------------|
| **Task-set hash** | `cargo run -p evalbench-cli -- hash crates/evalbench-core/fixtures/tasks --verify crates/evalbench-core/fixtures/tasks.sha256` | Hash of the 12 frozen synthetic tasks; exit 1 if it differs from the committed file, 2 on a stray or missing file |
| **Gap check** | `evalbench check <posting.json> <pack.json>` | Gap list as JSON; exit 1 when there are gaps. Matches the Python reference on well-formed input (every frozen fixture). Rust's schema is strict and requires every field; Python reads only the fields it uses, so schema-invalid input Python tolerates exits 2 here (rule in `crates/evalbench-core/src/check.rs`, examples in `tests/python_stricter.rs`) |
| **Score** | `evalbench score --tasks <dir> [--run-id ID] <traces.jsonl>` | First-pass score = floor(100 * tasks passing their frozen checker on attempt 1 / tasks), with the lower-median tokens per passing task beside it |
| **Record / run** | `evalbench record ...`, `evalbench run --replay <replies> \| --exec <wrapper>` | Checker-judged trace rows (no `--pass`); live calls only through a wrapper, no keys in evalbench |
| **Baseline** | `scripts/evalbench-baseline` | Replays `crates/evalbench-cli/baselines/replay-001` offline and checks the numbers did not move. Synthetic replies, not a model result |
| **Tests** | `cargo test -p evalbench-core -p evalbench-cli` | Port, differential vs Python (needs `python3`), examples where Rust's strict schema rejects input Python tolerates, P1/P2 bounded proofs with broken variants |

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
| `crates/evalbench-core/`, `crates/evalbench-cli/` | evalbench E1: frozen synthetic task set, gap checker, trace schema, first-pass score |
| **external: [participatory-mesh](https://github.com/thecuriousts/participatory-mesh)** | Multi-device CommandFabric: after ensembly authorizes/claims, dispatch allowlisted work onto another participant. Raises execution reach; not life SoT — [thinking/mesh-bridge-2026-09-08.md](thinking/mesh-bridge-2026-09-08.md) |


---

## 4. What “IR” means here

**IR = Intermediate Representation** — versioned JSON (or CLI `--json`) between kernel and harnesses.

| IR | Produced by | Notes |
|----|-------------|-------|
| Runtime status | `runtime status --json` | Regime, pending gates, CP |
| Turn / FocusPlan | `ensembly turn --json` | Next physical + auth from CP |
| **Channel pulse** | `ensembly turn --channel` · `channel-pulse reconcile` | Redacted one body + one gate; see [PLAYBOOK.md §5](PLAYBOOK.md#5-channel-pulse-issue-8) |
| **Application ledger** | `ensembly apps validate\|append\|export` | SoT for job applications; real rows local-only; see [APPLICATION-LEDGER.md](APPLICATION-LEDGER.md) |
| **Pulse core** | `ensembly pulse check\|read\|list\|history\|supersede\|admit\|conflict resolve\|missions` | Check and read stay read-only. `list` is live memory (`kind = memory`, `superseded_by` empty). `history` reads the archive chain. `supersede`, `admit --supersedes`, and `conflict resolve` record supersession (source, time, reason, writer) and keep the old row. Exit 0 ok, 1 warn, 2 fail, 7 refused write (audit keeps 3 for flag) |
| **Company audit** | `ensembly apps audit validate\|append\|export\|status` | Scam-risk gate before packs/applications; exit 0/3/4/5; see [COMPANY-AUDIT.md](COMPANY-AUDIT.md) |
| Pulse pack | `pulse-pack export` | Portable memory + archive events |
| **Pulse canon** | `ensembly pulse canon [PACK \| --from-db]` · `pulse pack-check PACK` | One canonical pack (`pulse-pack-v1`, canon 1); reads Python and Rust packs; store opened `mode=ro` + `query_only`. Spec: [spec/PULSE-PACK-SPEC.md](spec/PULSE-PACK-SPEC.md) |
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
