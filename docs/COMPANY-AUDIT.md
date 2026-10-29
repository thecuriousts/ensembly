# Company audit store

Operator rule (26 Oct 2026): every company is audited for scam risk and other
factors **before a pack is prepared or an application is sent**. Ensembly
stores the audits; other tools gate on `ensembly apps audit status`.

Code: `crates/ensembly-kernel/src/company_audit.rs`. Schema:
[`schemas/company-audit-row.schema.json`](../schemas/company-audit-row.schema.json).
Sample (fictional): `fixtures/company-audits-sample.jsonl`.

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
| `audited_at` | RFC 3339 with seconds and the Europe/Stockholm offset (`+01:00` / `+02:00`). |
| `audited_by` | Non-blank (agent or person label, no personal contact data). |
| `reused_from` | Optional. The `audited_at` of an earlier audit whose result this row reuses (a re-stamp, not a fresh check). Same time format as `audited_at`. |

Unknown fields are refused. Append-only: a re-audit is a new row; the last
row per domain wins.

## Where data lives

| What | Path | Git |
|---|---|---|
| Real audits | `data/local/applications/company-audits.jsonl` | local-only (gitignored `data/`) |
| Exports | `data/local/applications/exports/company-audits.{csv,md}` | local-only |

## CLI

```bash
ensembly apps audit validate [FILE]          # store, or a FILE in the same format
ensembly apps audit append FILE|-            # all lines validated first (any error: nothing written);
                                             # rows already stored are skipped, so a re-run appends 0
ensembly apps audit export [--out-dir DIR]   # byte-identical on re-run; one row per domain, sorted
ensembly apps audit status <company-or-domain> [--now RFC3339]
```

`--store PATH` (after `audit`) overrides the store path. `status` takes a
domain or URL (anything with a dot) or a company name (case-insensitive; with
several domains, the most recent audit wins).

### Exit-code contract (`status`)

| Status | Meaning | Exit |
|---|---|---|
| `pass` | Latest audit passes and is ≤ 30 days old | 0 |
| `flag` | Latest audit flags: a human looks before a pack or send | 3 |
| `block` | Latest audit blocks: do not prepare or send | 4 |
| `unaudited` | No audit for this company/domain | 5 |
| `stale` | Latest audit is older than 30 days: re-audit | 5 |
| `stale` | Latest `audited_at` is more than 5 minutes in the future: fix the stamp | 5 |
| (error) | Store unreadable or invalid, bad arguments | 1 / 2 |

`status` prints one JSON object: `query`, `status`, `company`, `domain`,
`verdict`, `audited_at`, `stale_after_days`.
