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

Exit 0 when every manifest file is present with a matching checksum, **no extra files** on disk (except `MANIFEST.json`), and jsonl/ics parse rules pass. Exit 1 on any failure.

## Pack / join (size limits)

When each transferred file must stay under a cap (default **16 MiB**):

```bash
ensembly bundle pack /tmp/bundle-fixture --out /tmp/chunks
# move chunks/bundle-fixture.parts.json and all bundle-fixture.tar.partNNN
ensembly bundle join /tmp/chunks/bundle-fixture.parts.json --out /tmp/rejoined
ensembly verify /tmp/rejoined
```

Pack does not edit the source bundle. Join checks every part hash and the total tar hash, refuses extra/missing parts and a non-empty `--out` (or an `--out` path that is a file), unpacks with the same path-safety rules as restore, then runs **verify** on the result. Part order in `parts.json` does not matter; join uses each part’s `index`.

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
| `--dry-run` | **Verify** + plan only (no writes) |

`import` and `restore` (including `--into` for ops/memory) **always run verify first** and exit non-zero before touching targets when the bundle fails verification.

Refuses non-empty export dir unless `--force-new-subdir`. Skips oversize files (`--max-file-mb`, default 200) and secret-pattern hits (stderr + MANIFEST).

## Sealed ops backup (not portable)

```bash
ensembly sealed-backup --out /tmp/sealed.pack --unlock '…'
```

See [portable-bundle.md](portable-bundle.md) for layout and config.

## Adopt a writer seal

The application ledger writer refuses to append when `{file}.writer-hash` is missing (`seal is missing; the writer will not adopt current bytes`). It does not create that seal from bytes it did not write. Deleting a seal and appending again stays refused.

When a file was written before the seal existed, the operator adopts it explicitly:

```bash
sha256sum data/local/applications/ledger.jsonl
ensembly apps adopt-seal \
  --ledger data/local/applications/ledger.jsonl \
  --sha256 <64 hex characters from sha256sum> \
  --reason "reviewed on laptop-1"
```

The same command seals a company-audit file with `--store` instead of `--ledger`. Pass exactly one of those paths. `--reason` is required and is one line.

Adoption writes the seal only when all of these hold:

- no seal file exists yet (a second run is refused; a hash mismatch on append stays `history was rewritten`)
- the file's current sha256 equals `--sha256` (the operator has looked at these exact bytes)
- a full validate of the file has no errors

Stored rows that only break a newer per-row rule warn `legacy: <rule>` and do not block adoption. Invalid JSON, unknown fields, and a blank `audited_by` do. The command prints `adopted path=… sha256=… rows=…` and appends one JSON line (path, sha256, rows, timestamp, reason) to `{file}.adoptions`. It does not rewrite the data file.

`adopt-seal` locks the file, re-reads those bytes, and writes `{file}.adoptions` before it publishes the seal. A symlink at the seal or the adoption log is refused. The ledger writer and the audit writer both consult the seal: a missing seal blocks `ensembly apps append` and `ensembly apps audit append`, and a successful append updates the seal. After `adopt-seal`, either append can write a new row. Deleting a seal does not let append recreate it.