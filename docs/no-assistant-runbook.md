# No-assistant runbook — portable bundle

Operator steps without an SI harness. Synthetic host names only in examples (`laptop-1`).

## Export

```bash
ensembly export --out /tmp/bundle-fixture
ensembly backup --out /tmp/backups-root    # timestamped subdir
```

Expect `PORTABLE_EXPORT_OK`. Inspect:

```text
/tmp/bundle-fixture/MANIFEST.json
/tmp/bundle-fixture/ensembly/ops/ops-bundle.json
/tmp/bundle-fixture/ensembly/views/todos.csv      # read-only view
/tmp/bundle-fixture/ensembly/views/chores.csv
/tmp/bundle-fixture/ensembly/views/deadlines.ics
/tmp/bundle-fixture/<section>/<source>/…
```

Views are derived from `life_state` in the ops JSON. **Restore never applies views** — only the ops JSON is authoritative.

## Verify

```bash
ensembly verify /tmp/bundle-fixture
```

Exit 0, or exit 1 with checksum/parse errors.

## Import / restore

```bash
ensembly import /tmp/bundle-fixture --dry-run
ensembly restore /tmp/bundle-fixture --into /tmp/restore-data --merge
ensembly restore /tmp/bundle-fixture --sources-to /tmp/restored-sources
```

| Flag | Meaning |
|------|---------|
| `--into DIR` | Ops sqlite + memory JSON under standard filenames in `DIR` |
| `--sources-to DIR` | Config sources under `DIR/<section>/<name>/…` |
| `--merge` | Union ops kv and ledger lines; refuse overwrite without it |
| `--dry-run` | Verify + plan only |

Refuses non-empty export dir unless `--force-new-subdir`. Skips oversize files (`--max-file-mb`, default 200) and secret-pattern hits (stderr + MANIFEST).

## Sealed ops backup (not portable)

```bash
ensembly sealed-backup --out /tmp/sealed.pack --unlock '…'
```

See [portable-bundle.md](portable-bundle.md) for layout and config.