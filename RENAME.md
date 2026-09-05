# Crate rename — `peram-*` → `ensembly-*`

Operator decision (2026-09-05): drop the personal `peram-` package prefix. Use **`ensembly-*`**. Do **not** use `actor-`.

Domain vocab is unchanged: **pulse / gate / reconcile / wait-snapshot**.

## Package and path map

| Old | New |
|-----|-----|
| `crates/peram-kernel` · package `peram-kernel` | `crates/ensembly-kernel` · `ensembly-kernel` |
| `crates/peram-memory` · package `peram-memory` | `crates/ensembly-memory` · `ensembly-memory` |
| `crates/peram-agents` · package `peram-agents` | `crates/ensembly-agents` · `ensembly-agents` |
| CLI bin `peram` | Primary **`ensembly`** |
| MCP bin `peram-mcp` | Primary **`ensembly-mcp`** |

Rust imports follow the hyphen→underscore rule: `ensembly_kernel`, `ensembly_memory`, `ensembly_agents`.

Parked `prototype/crates/peram-core` stays as historical Game-of-Peram WASM — not workspace SoT.

## Compat aliases (one release)

Cheap Cargo `[[bin]]` aliases, same source, `default-run` keeps `cargo run -p …` unambiguous. Cargo may warn that one file is present in multiple bin targets — expected, one-release only.

| Primary | Alias |
|---------|-------|
| `ensembly` | `peram` |
| `ensembly-mcp` | `peram-mcp` |

Prefer the primary names. After this release, aliases may be deleted.

```bash
cargo run -p ensembly-kernel -- runtime status
cargo run -p ensembly-kernel --bin peram -- runtime status   # alias
cargo build -p ensembly-agents --bin ensembly-mcp
```

Grok register (new): `grok mcp add --scope project ensembly -- ./target/debug/ensembly-mcp`.  
Existing `[mcp_servers.peram]` + `peram-mcp` still work via the alias.

## On-disk paths — document, do not migrate

Local operator files and wire formats keep their historical names so existing DBs, backups, and pulse packs stay valid. **Do not force-rename or migrate user data.**

| Kind | Stable identifier |
|------|-------------------|
| T1 ops DB | `data/local/peram-ops.sqlite` (also `private/state/peram-ops.sqlite`) |
| Episodic memory | `data/local/peram-memory.json` |
| Ops bundle format | `peram-ops-bundle-v1` |
| Pulse-pack format | `peram-pulse-pack-v1` |
| T2 vault domain separators | `peram-kernel-t2-v1:` · `peram-kernel-fp-v1:` |
| CRDT replica id | `peram-swarm` |
| Env vars | `PERAM_MEMORY` · `PERAM_AGENT_ID` · `PERAM_INFERENCE` · `PERAM_DELEGATE` · `PERAM_UNLOCK` |

Override paths with `--db` / `--memory` if you want different filenames. The kernel will not rewrite a live `peram-ops.sqlite` to a new name.

## Historical prose

July 2026 decisions and parked `prototype/` notes may still say `peram-kernel` as the name that shipped then. Live law (AGENTS, MAP, PLAYBOOK, MUSK-CUT SoT table) uses `ensembly-*`.
