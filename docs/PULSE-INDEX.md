# Pulse index (opt-in)

```text
  pulse-memory SQLite          ensembly kernel (read paths)
  -------------------          ----------------------------
  memory_traces    <-----      pulse check / read / list / history
        ^
        |  optional DDL only via explicit CLI
        |
  ensembly pulse index apply|status|revert
        |
        v
  ensembly_audit (append-only rows: who, when, sql, outcome)
```

The live **pulse-memory** database is owned by premflow / pulse-memory, not ensembly. Kernel read paths stay the default; **no command other than `pulse index apply`** creates these indexes.

## Recommendation (measured, 2026-11)

**Do not run `pulse index apply` on a store that looks like the retrieval-bench corpus** (~100% of rows are live `kind = memory` with empty `superseded_by`). On that shape, retrieval-bench at 1k / 10k / 100k consistently shows **no latency win** for `pulse list` (often slightly slower with indexes), and dangling checks stay ~sub‑10 ms with or without indexes.

Why:

| Finding | Detail |
|---------|--------|
| Query match | `pulse list` uses `ORDER BY time DESC, nat_key ASC` with `kind = 'memory'` and empty `superseded_by` — same as the partial index definition (not `last_seen`). |
| Planner | With indexes present, SQLite often plans `MULTI-INDEX OR` on `superseded_by` plus **`USE TEMP B-TREE FOR ORDER BY`**, and **does not** use `ensembly_memory_traces_live_list` when ~all rows are live. |
| Corpus | Slice‑7 microbenchmarks that showed large list speedups used a **sparse** live set (most rows archived). The generator behind retrieval-bench does not. |
| Dangling | JOIN rewrite is correct; at bench scale the check is already cheap; `superseded_by` index yields only small deltas. |

The sparse-live probe embedded in retrieval-bench (`sparse live_fraction` ≈ 0.05) is the honest place to see whether partial-index list wins on **your** machine. If that probe does not improve p50, do not apply on production.

Until a production pulse store is measured with a **sparse** archive (or planner starts using the partial index on your file), treat this command as **experimental** — kept for operators who own the DB and have evidence.

## Commands

| Command | Effect |
|---------|--------|
| `pulse index apply --writer ALIAS` | `CREATE INDEX IF NOT EXISTS` for `last_seen`, partial live `pulse list`, and `superseded_by` (`ensembly_*`). One transaction; audit rows committed with indexes. |
| `--dry-run` | Prints SQL only; no tables, indexes, or audit rows. |
| `--analyze` | After indexes, runs `ANALYZE memory_traces` (logged in audit). Writes `sqlite_stat*`; **`revert` does not remove stats.** |
| `pulse index status` | Lists which managed indexes exist. |
| `pulse index revert --writer ALIAS` | Drops managed indexes only, in one transaction with audit rows. |

Reversible indexes only: `revert` never drops indexes outside the managed list.

## Footprint after `revert`

| Left in the pulse DB | Removed by `revert` |
|----------------------|---------------------|
| `ensembly_audit` table (if ever created) and all audit rows | — |
| `sqlite_stat*` from optional `--analyze` | — (not reversible) |
| — | All three `ensembly_*` indexes |

Host aliases only in examples (`laptop-1`, `laptop-2`, `mac-mini`).

## Measurement

```bash
RETRIEVAL_BENCH_MACHINE=laptop-1 cargo run -p retrieval-bench --release -- --sizes 1000,10000,100000 --out target/retrieval-bench
```

Read `summary.md` → **Pulse index** (latencies, `live_memory_fraction`, EXPLAIN lines, sparse probe). Set `RETRIEVAL_BENCH_MACHINE` to a host alias; if unset, reports use `unspecified` (never the OS hostname).