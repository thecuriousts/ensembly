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
| Sparse probe | retrieval-bench archives ~95% of rows on a copy (`live_fraction` ≈ 0.05) and calls `pulse index apply` **without** `--analyze` (same as the default CLI). **List p50 still drops** (e.g. 13.2 → 6.9 ms at 100k; 1.15 → 0.66 ms at 10k; 0.11 → 0.07 ms at 1k), but EXPLAIN still shows **`MULTI-INDEX OR` on `ensembly_memory_traces_superseded_by`** plus **`USE TEMP B-TREE FOR ORDER BY`** — **not** `ensembly_memory_traces_live_list`. That sparse win is **not** evidence that the partial list index is doing the work. |
| After `ANALYZE` | `pulse index apply --analyze` (or `ANALYZE memory_traces` after apply) is what makes SQLite plan **`SCAN … USING INDEX ensembly_memory_traces_live_list`** for the list query. Without fresh stats, do not read sparse timings as proof the partial list index will be chosen. |

Do **not** treat the embedded sparse probe as “partial list index wins on your machine.” Use it only to separate **sparse archive** from **dense live** corpora. If your production store is dense like retrieval-bench (`live_memory_fraction` ≈ 1.0), do not apply. If it is sparse, run apply with **`--analyze`** on a copy, confirm EXPLAIN uses `live_list`, then measure before you apply on the real file.

Until that sequence is satisfied on **your** pulse DB, treat this command as **experimental** — kept for operators who own the DB and have evidence.

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