# Company audit store

Operator rule (26 Oct 2026): every company is audited for scam risk and other
factors **before a pack is prepared or an application is sent**. Ensembly
stores the audits; other tools gate on `ensembly apps audit status`.

Code: `crates/ensembly-opportunity-finder/src/company_audit.rs`. Schema:
[`schemas/company-audit-row.schema.json`](../schemas/company-audit-row.schema.json).
Sample (fictional): `crates/ensembly-opportunity-finder/fixtures/company-audits-sample.jsonl`.

## Row

```json
{"company":"…","domain":"jobs-host.example.test","apply_url":"https://…","verdict":"flag",
 "checks":[{"id":"apply_domain","result":"pass","note":"…","sources":["https://…"]}, …],
 "audited_at":"2026-10-27T09:00:00+01:00","audited_by":"…"}
```

| Field | Rule |
|---|---|
| `company` | Non-blank. |
| `domain` | Canonical host: lowercase, no scheme, `www.`, path, port or trailing dot. Identity key. |
| `apply_url` | Optional https URL. |
| `checks` | Each of the 8 ids **exactly once**: `apply_domain`, `domain_age`, `funding_ownership`, `headcount`, `reviews_news`, `duplicate_ghost`, `payment_id_ask`, `agency_repost`. Result `pass`, `flag`, `block` or `unknown`. A partial audit is not an audit; record what you could not find as `unknown`. |
| `checks[].note` | Non-blank. Refused if it looks like personal data: an email address, or a phone-like number (9+ digits written with `+`, a leading 0, parentheses or separators). ISO dates and plain amounts pass. |
| `checks[].sources` | https URLs only. Every `pass`/`flag`/`block` needs at least one; `unknown` may have none. |
| `verdict` | **Derived, then checked**: `block` if any check is block; `flag` if any check is flag, any of `apply_domain`, `domain_age`, `funding_ownership` is unknown, or `headcount` and `reviews_news` are both unknown (the employer's size and standing are then unverified); else `pass`. A new row whose stated verdict disagrees is refused. A row stored before 30 Oct 2026 that says `pass` under the earlier rule still reads, but `status` and exports use the derived verdict (`flag`). |
| `audited_at` | RFC 3339 with seconds and any numeric offset or `Z`/`z` (UTC). |
| `audited_by` | Required on a new append: non-blank (agent or person label, no personal contact data). A row written before the field existed may omit it. JSON null reads as absent, the same as an omitted field. Readers keep that as absent and do not invent a label. `validate` and `status` warn `legacy: audited_by missing` with the 1-based line number and still exit 0. `--strict` makes those rows fail (exit 1). A blank value is an error on read and on write, not a legacy warning. |
| `reused_from` | Optional. The `audited_at` of an earlier audit whose result this row reuses (a re-stamp, not a fresh check). Same time format as `audited_at`. On append it must name an audit of the **same domain** already in the store (or earlier in the same batch) with an earlier time; otherwise the append is refused. `validate` warns (exit 0) about stored rows written before this check whose reference names no such audit. |

Unknown fields are refused. Append-only: a re-audit is a new row; the last
row per domain wins. A legacy row is never rewritten. To record who audited
a domain that still has an old line, append a full new row for that domain
(with `audited_by`); the old line stays, and the last row wins. A short
correction row (`kind=correction`, `supersedes` a line or row id) is not
part of this store.

## Where data lives

| What | Path | Git |
|---|---|---|
| Real audits | `data/local/applications/company-audits.jsonl` | local-only (gitignored `data/`) |
| Exports | `data/local/applications/exports/company-audits.{csv,md}` | local-only |

## CLI

```bash
ensembly apps audit validate [--strict] [FILE]   # store, or a FILE in the same format
ensembly apps audit append FILE|-                # all lines validated first (any error: nothing written);
                                                 # a new row without audited_by is refused;
                                                 # rows already stored are skipped, so a re-run appends 0
ensembly apps audit export [--out-dir DIR]       # byte-identical on re-run; one row per domain, sorted; atomic replace
ensembly apps audit status [--strict] <company-or-domain> [--now RFC3339]
```

`--store PATH` (after `audit`) overrides the store path. `status` takes a
domain or URL (anything with a dot) or a company name (case-insensitive; with
several domains, the most recent audit wins).

`validate` and `status` print one stderr warning per stored row that omits
`audited_by`, including rows that are not the company just queried:

```text
warning: line 73: legacy: audited_by missing
```

Exit stays 0 (or the status gate code) unless the file has a real error.
`--strict` prints the same warning and then exits 1, and `status --strict`
does not print the JSON object. `append` always requires `audited_by` on
each new row; `--strict` is not an append flag. Export leaves the auditor
cell blank for a legacy row and does not rewrite the store.

### Exit-code contract (`status`)

| Status | Meaning | Exit |
|---|---|---|
| `pass` | Latest audit passes and is ≤ 30 days old | 0 |
| `flag` | 3 = flag (not approvable): a human reviews the flags before a pack or send | 3 |
| `block` | Latest audit blocks: do not prepare or send | 4 |
| `unaudited` | No audit for this company/domain | 5 |
| `stale` | Latest audit is older than 30 days: re-audit | 5 |
| `stale` | Latest `audited_at` is more than 5 minutes in the future: fix the stamp | 5 |
| (error) | Store unreadable or invalid, bad arguments | 1 / 2 |
| (strict) | `--strict` and any row omits `audited_by` (no status JSON) | 1 |

`status` prints one JSON object: `query`, `status`, `company`, `domain`,
`verdict`, `audited_at`, `stale_after_days`.
