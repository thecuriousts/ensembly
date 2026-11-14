# Portable plain-text bundle

Operator-readable exports for kernel stores plus configured `[[backup.sources]]` trees. Sealed T1 pack: **`ensembly sealed-backup`**. Portable path: **`ensembly export`**, **`backup`**, **`verify`**, **`import`**, **`restore`**. Operator checklist: [no-assistant-runbook.md](no-assistant-runbook.md).

## Layout

```text
bundle/
  MANIFEST.json
  ensembly/
    ops/ops-bundle.json
    memory/ensembly-memory.json
    ledger/ledger.jsonl        # when opportunity-finder ledger exists
    views/todos.csv            # read-only; from life_state
    views/chores.csv
    views/deadlines.ics        # chore due dates only
  <section>/
    <source-name>/…            # from config; default section `shared`
```

Reserved section name: **`ensembly`** (kernel stores only). User sources must use other section names (`shared`, `work`, `life`, …).

### What `ensembly/` includes (and what it does not)

| File | SoT | Notes |
|------|-----|--------|
| `ops/ops-bundle.json` | T1 ops SQLite (`kv` + schema) | Same shape as `ensembly ops-bundle export`. Chores, deadlines, and HITL wait state live inside `kv` (e.g. `life_state`, `wait_snapshot`) — not separate tables. |
| `memory/ensembly-memory.json` | Episodic CRDT | Plain JSON on disk. |
| `ledger/ledger.jsonl` | Application ledger | Present only when the ledger file exists (`opportunity-finder` build). |

Plain **md / csv / ics** in the bundle come from **`[[backup.sources]]`** (or future derived exporters). This path does **not** replace **`pulse-pack`** (traces, optional audit tail, CRDT merge sync).

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
- **`force_include`** on a source lists paths (globs) that bypass default name excludes such as `*token*` or `*.pem`; secret-pattern scanning still applies.

## Commands

| Command | Role |
|---------|------|
| `ensembly export --out DIR` | Write bundle; refuses non-empty `DIR` unless `--force-new-subdir` |
| `ensembly backup [--out DIR]` | Timestamped dir under `data_dir/backups` or `--out` |
| `ensembly verify BUNDLE` | Manifest paths, checksums, parse rules, **no missing/extra files** |
| `ensembly import BUNDLE [--dry-run]` | Always **verify-first** (exit 1 on failure) |
| `ensembly restore BUNDLE --sources-to DIR` | **Verify-first**, then restore sources under `DIR/<section>/<name>/…` |
| `ensembly bundle pack BUNDLE_DIR --out DIR [--split-mb N]` | Deterministic tar split (default **16** MiB); writes `<name>.parts.json` + `<name>.tar.partNNN` |
| `ensembly bundle join PARTS_JSON --out DIR` | Verify parts + total hash, safe unpack, **verify** joined bundle |

Never restores config sources to their original paths without an explicit destination.

## Moving a bundle in parts

When a channel limits individual files (e.g. **16 MiB**), pack the bundle directory without changing it:

```bash
ensembly bundle pack /tmp/bundle-fixture --out /tmp/chunks --split-mb 16
```

That directory gets `bundle-fixture.parts.json` and `bundle-fixture.tar.part001`, `part002`, … Each part’s size and SHA-256 are listed; the manifest also carries the **total** tar SHA-256. Transfer every part file **and** the JSON manifest. The `parts` array order does not matter — join sorts by each entry’s `index` field.

**Tar format:** `bundle pack` writes **GNU** tar with `././@LongLink` extension records when a manifest-relative path exceeds the ustar 100-byte name limit. Common **GNU tar** and **bsdtar** read these archives for split/join; ustar-only tools may not.

### `*.parts.json` fields

| Field | Meaning |
|-------|---------|
| `parts_format` | Always `ensembly-portable-bundle-parts-v1` |
| `bundle_name` | Base name used for `*.tar.partNNN` files |
| `split_mb` | Max part size in MiB used when packing |
| `part_count` | Number of parts (must match `parts.len()`) |
| `total_sha256` | SHA-256 of the concatenated tar |
| `parts[]` | One object per part: `index` (1-based), `file`, `sha256`, `bytes` |

Reassemble on the receiver (empty target directory):

```bash
ensembly bundle join /tmp/chunks/bundle-fixture.parts.json --out /tmp/rejoined
```

Join refuses checksum mismatches, missing or extra part files, non-empty output, tar path traversal, symlinks, and absolute paths. It finishes with the same **verify** rules as `ensembly verify` on the unpacked tree.

## Flow

```mermaid
flowchart LR
  export[export / data-backup] --> manifest[MANIFEST.json]
  manifest --> verify[verify: checksums + no extra files]
  verify --> restore[restore: verify once, then apply]
```
