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
| `pulse index apply --writer ALIAS` | `CREATE INDEX IF NOT EXISTS` for `last_seen`, a partial index for live `pulse list`, and `superseded_by` (names prefixed `ensembly_`). |
| `pulse index status` | Lists which managed indexes exist. |
| `pulse index revert --writer ALIAS` | `DROP INDEX IF EXISTS` for managed names only. |
| `--dry-run` on apply/revert | Prints SQL; no writes. |

Reversible: `revert` never drops indexes that are not on the managed list. Apply/revert append one audit row per statement to `ensembly_audit` (created if missing); no row deletion anywhere.

Host aliases only in examples (`laptop-1`, `laptop-2`, `mac-mini`).

```bash
export ENSEMBLY_WRITER=laptop-1
ensembly pulse index apply --writer laptop-1
ensembly pulse index status
# later
ensembly pulse index revert --writer laptop-1
```

Synthetic 100k-row fixtures (release, ignored test): `pulse list` ~17 ms → ~0.01 ms with the partial index; dangling-supersession check ~110 ms → ~10.5 ms after index + JOIN rewrite (prefix read is slice 1b).