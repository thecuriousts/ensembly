# Portable plain-text bundle

Operator-readable exports for kernel stores plus configured `[[backup.sources]]` trees. Sealed `ensembly backup` (T2 ops pack) is unchanged; use **`ensembly export`**, **`ensembly data-backup`**, **`verify`**, **`import`**, and **`restore`** for portable bundles.

## Layout

```text
bundle/
  MANIFEST.json
  ensembly/
    ops/ops-bundle.json
    memory/ensembly-memory.json
    ledger/ledger.jsonl        # when opportunity-finder ledger exists
  <section>/
    <source-name>/…            # from config; default section `shared`
```

Reserved section name: **`ensembly`** (kernel stores only). User sources must use other section names (`shared`, `work`, `life`, …).

## Config

```toml
[[backup.sources]]
name = "notes"
section = "shared"   # optional; default shared
path = "~/vault/notes"
kind = "dir"
include = ["**/*"]
exclude = ["*.pem"]
```

- `section` must match `[A-Za-z0-9_-]+` (no `/` or `.`).
- Source `name` must be unique within a section; duplicates are a config error (exit 2, file + line).
- Large files are streamed for SHA-256; files over **`--max-file-mb`** (default 200) are skipped with a warning and a MANIFEST `excluded` row.

## Commands

| Command | Role |
|---------|------|
| `ensembly export --out DIR` | Write bundle; refuses non-empty `DIR` unless `--force-new-subdir` |
| `ensembly data-backup [--out DIR]` | Timestamped dir under `data_dir/backups` or `--out` |
| `ensembly verify BUNDLE` | Checksums + jsonl/ics parse |
| `ensembly import BUNDLE [--dry-run]` | Verify-first ack |
| `ensembly restore BUNDLE --sources-to DIR` | Restore config sources under `DIR/<section>/<name>/…` |

Never restores config sources to their original paths without an explicit destination.

## Flow

```mermaid
flowchart LR
  export[export / data-backup] --> manifest[MANIFEST.json]
  manifest --> verify[verify]
  verify --> restore[restore --sources-to]
```