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

## On-disk paths — discover + fresh default (no silent migrate)

**Law:** fresh installs create generic `ensembly-*` files. Existing dogfood trees that already have `peram-*` files keep working. The kernel **never** rewrites or renames the only live file in place.

### Discover (no `--db` / `--memory`)

1. Prefer an existing `ensembly-*` file if present.
2. Else if a legacy `peram-*` file exists, **open that**.
3. Else create/use the new `ensembly-*` default.

`--db` and `--memory` always win.

| Role | Fresh default (create) | Legacy discover (open only) |
|------|------------------------|-----------------------------|
| T1 ops DB | `data/local/ensembly-ops.sqlite` (also `private/state/ensembly-ops.sqlite` if that parent exists) | `data/local/peram-ops.sqlite`, `private/state/peram-ops.sqlite` |
| Episodic memory | `data/local/ensembly-memory.json` | `data/local/peram-memory.json` |
| New CRDT replica id | `ensembly-swarm` | existing docs keep `peram-swarm` |
| Env | `ENSEMBLY_MEMORY` · `ENSEMBLY_AGENT_ID` · `ENSEMBLY_INFERENCE` · `ENSEMBLY_DELEGATE` · `ENSEMBLY_UNLOCK` | `PERAM_*` one-release aliases |

### Wire formats — dual-read; new writes generic

| Kind | New export | Still imports |
|------|------------|---------------|
| Pulse pack | `ensembly-pulse-pack-v1` | `peram-pulse-pack-v1` |
| Ops bundle | `ensembly-ops-bundle-v1` | `peram-ops-bundle-v1` |

**Vault domain separators:** write-side stays `peram-kernel-t2-v1:` / `peram-kernel-fp-v1:` so existing sealed backups stay valid. Unseal dual-reads the `ensembly-kernel-t2-v1:` prefix if a blob was sealed that way. Do not change write-side without a versioned re-seal path.

### One-shot operator copy (then resync)

Do this on the **canonical host** (Grok Bot computer / ops writer) when you want the new filenames. Copy, smoke, then pulse-pack resync. **Do not dual-write ops.**

```bash
# Preview
cargo run -p ensembly-kernel -- migrate-local-paths --dry-run

# Copy-if-missing (keeps peram-* until you delete them)
cargo run -p ensembly-kernel -- migrate-local-paths
```

Manual equivalent:

```bash
cp data/local/peram-ops.sqlite data/local/ensembly-ops.sqlite
# if in use: cp private/state/peram-ops.sqlite private/state/ensembly-ops.sqlite
cp data/local/peram-memory.json data/local/ensembly-memory.json
```

Then:

1. Smoke on the canonical host: `cargo run -p ensembly-kernel -- runtime status` (defaults now prefer `ensembly-*` because those files exist).
2. **Resync memory** bot → laptop via existing pulse-pack (ops stays on the canonical host only):

```bash
# bot
cargo run -p ensembly-kernel -- pulse-pack export --out ~/sync/pulse/bot.pulse.json --include-archive

# laptop (after migrate-local-paths or after the first import creates ensembly-memory.json)
cargo run -p ensembly-kernel -- pulse-pack import --pack ~/sync/pulse/bot.pulse.json
```

3. After verify, operator may delete the legacy copies. Discover-fallback stays for one release so un-migrated trees still open.

## Historical prose

July 2026 decisions and parked `prototype/` notes may still say `peram-kernel` as the name that shipped then. Live law (AGENTS, MAP, PLAYBOOK, MUSK-CUT SoT table) uses `ensembly-*`.
