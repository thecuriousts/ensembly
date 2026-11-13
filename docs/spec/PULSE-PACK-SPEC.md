# Canonical pulse pack (`pulse-pack-v1`, canon 1)

**Status:** E2, 2026-11-13. Implementations: `crates/ensembly-kernel/src/pulse_pack/canonical.rs`
(Rust) and `scripts/pulse/pulse_pack_check.py` (Python, stdlib only). Both reproduce the
golden vectors in `crates/ensembly-kernel/tests/golden/pulse-pack/`.

## Why

Two pack formats drifted apart. The Python pulse tools write `pulse-pack-v1` (13 trace
fields, `host`, `archive`). The kernel writes `ensembly-pulse-pack-v1` (and still reads
`peram-pulse-pack-v1`): `source_host`, `ts`, `admitted_at`, `agent_id`, a `content`
value, coherence floats and a memory CRDT. Neither side could check the other's pack, and
the same memory had two hashes.

Canon 1 keeps the Python shape as the one format and maps the Rust formats into it. Old
packs stay readable; nothing is rewritten in place. `pulse-pack export` still writes
`ensembly-pulse-pack-v1` for CRDT sync; `pulse canon` turns any of the three into the
canonical bytes.

## Envelope

```json
{"archive":[...],"exported_at":"2026-11-13T09:00:00Z","ext":{...},"format":"pulse-pack-v1","host":"box","traces":[...]}
```

| Field | Type | Rule |
| --- | --- | --- |
| `format` | string | `pulse-pack-v1` |
| `exported_at` | string | timestamp, normalized (below) |
| `host` | string | any string; not one of `laptop-1`, `laptop-2`, `mac-mini`, `box` gives `W_HOST_ALIAS` |
| `traces` | array | sorted by `nat_key` (UTF-8 bytes); `nat_key` unique |
| `archive` | array | optional on input, always present on output; sorted by canonical JSON bytes |
| `ext` | object | optional; dropped when empty |

Trace fields (all required, no others): `nat_key` (non-empty string), `snippet` (string),
`source` (string), `time`, `first_seen`, `last_seen` (timestamps), `status`
(`user-stated`, `tool-verified`, `inferred`, `unknown`), `kind` (`data`, `context`,
`fact`, `memory`), `lock` (`open`, `locked`), `verify_hash`, `prefetch_key`,
`superseded_by` (string or null), `seen_count` (integer ≥ 1). Optional `ext` object with
`coherence_milli` (0..=1000), `content` (any JSON), `kind` (string) and `metadata` (object).

Archive event fields: `correlation_id`, `payload`, `source` (strings), `ts` (timestamp),
optional `ext` with `kind`.

Evidence: a `tool-verified` trace needs `verify_hash = "sha256:" + hex(sha256(snippet UTF-8))`,
else `E_EVIDENCE`. On other statuses a non-null hash that does not match gives
`W_VERIFY_MISMATCH`. A `superseded_by` key missing from the pack gives
`W_DANGLING_SUPERSEDED`.

## Serialization

- Object keys sorted by Unicode code point; separators `,` and `:`; no whitespace.
- Strings: escape `"`, `\`, `\b`, `\t`, `\n`, `\f`, `\r`; other characters below U+0020 as
  lowercase `\u00xx`; everything else raw UTF-8 (no `\u` for non-ASCII).
- Numbers: integers only, within ±(2^53 − 1). Any float (including `-0`, `1.0`, `1e2`) is `E_FLOAT`.
- `canonical_bytes(doc) = canonical_json(doc) + "\n"`.
- `pack_sha256 = sha256(canonical_bytes(doc))`;
  `content_sha256 = sha256(canonical_bytes({"archive":…,"traces":…}))`, which ignores
  `exported_at`, `host` and `ext`.

Python reaches the same bytes with
`json.dumps(v, ensure_ascii=False, sort_keys=True, separators=(",", ":"))`.

## Strict parse

`E_JSON` (path `""`) for: a UTF-8 BOM, invalid UTF-8, `NaN` / `Infinity`, a duplicate
object key, a lone surrogate escape, trailing data, a number whose magnitude overflows
binary64 (`1E400`, or an integer literal past about 1.8e308), or any other syntax error.

Numbers that parse: an integer literal beyond ±(2^53 − 1), including one beyond u64, is
`E_RANGE` at its path; a fraction or exponent literal (`1e20`, `1e-400`) or `-0` is
`E_FLOAT`.

## Timestamps

Accepted (ASCII digits only): RFC 3339 `YYYY-MM-DDTHH:MM:SS[.f{1,9}](Z|±HH:MM)`,
`YYYY-MM-DD HH:MM` (UTC), `YYYY-MM-DD` (midnight UTC). Output:
`YYYY-MM-DDTHH:MM:SS[.frac]Z` in UTC, trailing fraction zeros trimmed. The UTC result
must fall in years 0001–9999. Anything else is `E_TIME`.

## Mapping old Rust packs (migration alias)

`ensembly-pulse-pack-v1` and `peram-pulse-pack-v1` are read, not rejected:

| Rust | Canonical |
| --- | --- |
| `source_host` | `host` |
| `memory_traces`, `archive_events` | `traces`, `archive` |
| `memory_hash` (string or null), `format`, `pack_hash` | `ext.memory_hash`, `ext.source_format`, `ext.source_pack_hash` |
| `memory_crdt` | not carried (CRDT state, not memory facts) |
| trace `ts` | `time` |
| trace `admitted_at` | `first_seen` and `last_seen` |
| trace `agent_id` | `source` |
| trace `content` string | `snippet` |
| trace `content` other JSON | `snippet = canonical_json(content)`, `ext.content = content` |
| trace `kind` | `ext.kind`; `kind = memory` |
| trace `metadata` (non-empty object) | `ext.metadata` |
| trace `coherence` | `ext.coherence_milli = floor(c × 1000 + 0.5)`, must be 0..=1000 |
| (absent) | `status = unknown`, `lock = open`, nullable fields null, `seen_count = 1` |
| archive event | `correlation_id = nat_key`, `payload = canonical_json(payload)`, `source = "ensembly"`, `ext.kind` |

The verdict's `source_format` names the input format.

## Codes and verdict

Errors: `E_JSON`, `E_FORMAT`, `E_TYPE`, `E_MISSING`, `E_UNKNOWN_FIELD`, `E_STATUS`,
`E_KIND`, `E_LOCK`, `E_TIME`, `E_FLOAT`, `E_RANGE`, `E_DUP_KEY`, `E_EVIDENCE`.
Warnings (reported only when there are no errors): `W_HOST_ALIAS`,
`W_DANGLING_SUPERSEDED`, `W_VERIFY_MISMATCH`. Each issue is `{code, path}`, where `path`
is an RFC 6901 pointer into the input document; issues are sorted by (path, code).

Verdict (canonical JSON): `archive`, `canon`, `content_sha256`, `errors`, `exit`,
`format`, `pack_sha256`, `source_format`, `traces`, `verdict` (`ok`, `warn`, `fail`),
`warnings`. Exit 0 ok, 1 warn, 2 fail.

## CLI

```sh
ensembly pulse canon PACK                       # canonical bytes on stdout
ensembly pulse --pulse-db DB canon --from-db --exported-at T --host H
ensembly pulse pack-check PACK [--json]         # exit 0 / 1 / 2
python3 -I scripts/pulse/pulse_pack_check.py canon PACK
python3 -I scripts/pulse/pulse_pack_check.py canon --db DB --exported-at T --host H
python3 -I scripts/pulse/pulse_pack_check.py check PACK
python3 -I scripts/pulse/pulse_pack_check.py db-check DB   # evidence + supersession findings
```

Invalid input to `canon`: the verdict goes to stderr, exit 2, nothing on stdout.

## Read-only store

Readers open `file:<percent-encoded path>?mode=ro` (keep `A-Z a-z 0-9 - . _ ~ /`,
uppercase hex) with the URI flag, then `PRAGMA query_only = ON`. Rust:
`SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_URI | SQLITE_OPEN_NO_MUTEX`. Python:
`sqlite3.connect(ro_uri(path), uri=True)`. A missing store is an error, never created.
Every write (INSERT, UPDATE, DELETE, CREATE, PRAGMA user_version) fails, including after
`query_only` is switched off. On a WAL store SQLite may create `-shm` and an empty `-wal`;
the main file bytes do not change.

## Honest pulse check

`ensembly pulse check` (and `pulse_pack_check.py db-check`) warn, exit 1, on:

- `db.evidence`: a `tool-verified` row whose `verify_hash` is NULL, empty, or not
  `sha256:<hex>` of its snippet. Listed by key, sorted by key bytes.
- `db.supersession`: a `supersession_events` row stopped at `intent` or `applied`, listed
  as `old -> new (phase)`. Rerunning the same supersede finishes it and clears the warn.

`DbStats` carries them as `unpinned_verified` and `half_supersessions`. Golden vectors
`11-db-findings` and `12-db-findings-none` hold the expected lists for both sides.

## Decisions

- **D-E2-1** The Python shape is canonical: it is the richer fact record (status, lock, evidence).
- **D-E2-2** Rust formats are an alias, mapped on read; no in-place rewrite of old packs.
- **D-E2-3** Integers only; floats are refused, coherence travels as `coherence_milli`.
- **D-E2-4** Strict parse; duplicate keys and lone surrogates are errors, not last-wins.
- **D-E2-5** Goldens are generated by the Python checker from synthetic inputs (generator
  kept outside the repo) and frozen by `MANIFEST.sha256`; the Rust implementation was
  written separately and must match byte for byte.
- **D-E2-6** Read-only means `mode=ro` plus `query_only`; WAL side files are allowed,
  main-file changes are not.
- **D-E2-7** `pulse admit` writes `verify_hash = content_hash(snippet)`, as Python does,
  so kernel-admitted `tool-verified` rows pass canon.
- **D-E2-8** Warnings only when there are no errors, so a failing verdict has one cause list.
- **D-E2-9** Pulse-state rules live in `docs/spec/tla/PulseTrace.tla`; see its README.
- **D-E2-10** `pulse check` is honest (TLA+ `CheckIsHonest`): an unpinned tool-verified
  row or a half-finished supersession is a warn, never ok. Warn, not fail: the store is
  readable and the fix is a rerun or a re-pin, not a stop.
- **D-E2-11** One code per odd number on both sides: an integer past u64 is `E_RANGE`
  (Rust tracks integer tokens, since serde_json hands them over as f64); a binary64
  overflow such as `1E400` is `E_JSON` (serde_json cannot read it, and RFC 8259 §6 leaves
  it outside interoperable JSON).

## Former divergences

Before D-E2-11, an integer past u64 was `E_FLOAT` in Rust and `E_RANGE` in Python, and
`1E400` was `E_JSON` in Rust and `E_FLOAT` in Python. Golden vectors `33-int-over-u64` and
`34-float-overflow` now pin one code each.
