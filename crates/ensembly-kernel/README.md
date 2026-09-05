# ensembly-kernel

Control Source of Truth for ensembly: life-state **S**, dependency graph **G**, critical path with PERT/Monte Carlo **P**, typed message bus, Human-In-The-Loop gates and Human-Out-Of-The-Loop agent runtime, Tier-1 SQLite durability, sealed backups, pulse-pack sync, and the episodic-memory bridge. Terms expanded: [docs/GLOSSARY.md](../../docs/GLOSSARY.md).

All new *control-plane* logic lands in this crate. Parked game/Node stack: [prototype/](../../prototype/README.md).

## Build and test

```bash
cargo test -p ensembly-kernel     # 44 tests
cargo run -p ensembly-kernel -- version
```

## The runtime (Issue #1) — daily driver

```bash
FIXTURE=fixtures/issue-1-runtime.json

cargo run -p ensembly-kernel -- runtime load --fixture $FIXTURE   # S+G into SQLite, CP computed
cargo run -p ensembly-kernel -- runtime status                    # regime, CP, pending gates
cargo run -p ensembly-kernel -- runtime tick                      # drain bus + one HOOTL step
cargo run -p ensembly-kernel -- runtime approve pay-rent          # HITL gate (action id, not auth-*)
cargo run -p ensembly-kernel -- runtime claim grocery-errand      # physical beacon
cargo run -p ensembly-kernel -- runtime complete grocery-errand
cargo run -p ensembly-kernel -- runtime reflect                   # coherence over episodic memory
cargo run -p ensembly-kernel -- runtime dive --json               # UncertaintyDive inspect (Prior→Probe→Simulate→Score→ActOrAsk)
```

Notes:

- `--json` prints JSON **then** a trailing `RUNTIME_OK …` status line — strip the last line before parsing.
- Approve/deny take the **action id** (`pay-rent`); an `auth-` prefix is accepted and stripped.
- Tick honesty: one agent step per tick — claim **or** complete, never both.
- Top-level gates (`ensembly approve …`) refuse loudly without a durable life-state; run `runtime load` first.
- `runtime dive` does **not** mutate state — it surfaces epistemic emptiness + trauma guards. Quest: [docs/thinking/uncertainty-space-quest.md](../../docs/thinking/uncertainty-space-quest.md) (space = new-era ocean).

## Episodic memory bridge (`memory_sink`)

Every `runtime` command records **applied** bus messages, tick reports, and loads into a durable Conflict-free Replicated Data Type document at `data/local/ensembly-memory.json` (gitignored; legacy `peram-memory.json` is opened if that file already exists). Memory is auxiliary — it records what happened and is never consulted for control decisions.

| Flag | Behavior |
|------|----------|
| *(default)* | Record into `data/local/ensembly-memory.json` (or existing `peram-memory.json`); open failure warns, command continues |
| `--memory <path>` | Explicit path; open failure is **fatal** (exit 2) |
| `--no-memory` | Recording disabled for this invocation |

```bash
cargo run -p ensembly-kernel -- --memory /tmp/test-mem.json runtime tick
cargo run -p ensembly-kernel -- --no-memory runtime status
```

See [../ensembly-memory/README.md](../ensembly-memory/README.md) for the learning-layer semantics.

## Other surfaces

| Command | What you get |
|---------|--------------|
| `ensembly turn [--fixture f]` | FocusPlan: next physical beacon + next auth gate, coached from the critical path |
| `ensembly digital-flow cycle` | bill_pay dry-run through a Human-In-The-Loop finance gate |
| `ensembly backup --out pack.bin --unlock …` | Sealed Tier-2 backup pack (or `ENSEMBLY_UNLOCK` / `PERAM_UNLOCK` env) |
| `ensembly restore-dry-run --pack pack.bin --unlock …` | Verify a pack without touching the primary database |

Default database: `data/local/ensembly-ops.sqlite` (gitignored). Existing `peram-ops.sqlite` is opened in place. Override with `--db`.

## Library map

`agent` · `approvals` · `backup` · `critical_path` · `digital_flow` · `graph` · `life_state` · `memory_sink` · `msg_bus` · `privacy` · `realm` · `runtime` · `store` · `trigger` · `turn` · `vault`

Decision record: [docs/DECISIONS.md](../../docs/DECISIONS.md) · System map: [docs/MAP.md](../../docs/MAP.md)
