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

| Command | Effect |
|---------|--------|
| `pulse index apply --writer ALIAS` | `CREATE INDEX IF NOT EXISTS` for `last_seen`, a partial index for live `pulse list`, and `superseded_by` (names prefixed `ensembly_`). One transaction; audit rows committed with indexes. |
| `--dry-run` | Prints SQL only; no tables, indexes, or audit rows. |
| `--analyze` | After indexes, runs `ANALYZE memory_traces` and logs it in `ensembly_audit`. Writes `sqlite_stat*` planner data the owner did not get from index DDL alone. **`revert` does not remove stats.** |
| `pulse index status` | Lists which managed indexes exist. |
| `pulse index revert --writer ALIAS` | `DROP INDEX IF EXISTS` for managed names only, in one transaction with audit rows. |
| `--dry-run` on revert | Prints `DROP` SQL only; no writes. |

Reversible indexes only: `revert` never drops indexes outside the managed list.

## Footprint after `revert`

| Left in the pulse DB | Removed by `revert` |
|----------------------|---------------------|
| `ensembly_audit` table (if ever created by `apply`) and all audit rows | — |
| `sqlite_stat*` from optional `--analyze` | — (not reversible) |
| — | All three `ensembly_*` indexes |

Host aliases only in examples (`laptop-1`, `laptop-2`, `mac-mini`).

```bash
export ENSEMBLY_WRITER=laptop-1
ensembly pulse index apply --writer laptop-1
ensembly pulse index status
# later
ensembly pulse index revert --writer laptop-1
```

## Measured latency

Use the retrieval bench (honest, on your machine):

```bash
cargo run -p retrieval-bench --release -- --sizes 100000 --out target/retrieval-bench-100k
```

See `summary.md` → **Pulse index**: `pulse_list` with/without indexes; dangling check with legacy `NOT EXISTS` vs production `JOIN`, each with/without indexes. Do not cite numbers from docs — read the artifact.