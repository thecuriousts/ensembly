# Application ledger (Stage 1)

Ensembly is the **source of truth** for job applications and their exports
(operator decision, 26 Oct 2026). Chief of Staff keeps a copy; Linear and
Trello are **views only** and never win a conflict with this ledger.

## Row format

One JSON object per line (JSONL). Schema: [`schemas/application-ledger-row.schema.json`](../schemas/application-ledger-row.schema.json).
Code: `crates/ensembly-kernel/src/application_ledger.rs`.

| Field | Rule |
|---|---|
| `company` | Non-blank. Kept because role titles alone are ambiguous ("Full-Stack Engineer, Creative Agents" does not name the employer) and every view groups by company.  |
| `role` | Non-blank posting title as shown on the posting. |
| `link` | `https://` canonical posting URL. Identity key of an application; a trailing `/application` is ignored when matching, and `append` refuses a link that ends in it. |
| `apply_url` | Optional `https://` application-form URL (for example the posting URL plus `/application`). Omitted from the JSON when absent. |
| `status` | `prepared`, `submitted`, `interview`, `rejected`, `withdrawn`, `silent-closed`. |
| `submit_time` | RFC 3339 / ISO 8601 with the Europe/Stockholm offset (`+01:00` or `+02:00`). `null` only for `prepared`. |
| `cv_version` | Non-blank label of the CV that was sent. `legacy-unknown` is the one explicit marker for rows migrated from records that never noted the CV (operator decision, 26 Oct 2026); ad-hoc markers such as `unknown`, `n/a`, `tbd` are refused. |

Unknown fields are refused, so personal details (phone, address) cannot ride along.

### Why these six statuses

They are the states that change what the operator does next: still to send
(`prepared`), waiting (`submitted`), act now (`interview`), closed by them
(`rejected`), closed by us (`withdrawn`), closed by silence after a long wait
(`silent-closed`). Offers and finer interview stages wait until a real row
needs them.

## Append-only

Never edit a line. A status change is a new line with the same `link`; the
last line per link is the current state. `ensembly apps append` refuses an
identical row, so re-running an append is safe.

### Correcting a row

Append a new row; never edit. Because matching uses the canonical link, a row
stored with an `…/application` form link is superseded by a later row whose
`link` is the posting URL and whose `apply_url` carries the form URL. Old
lines stay valid, so history is kept and exports show only the corrected row.

## Where data lives

| What | Path | Git |
|---|---|---|
| Real rows | `data/local/applications/ledger.jsonl` | local-only (`data/` is gitignored) |
| Exports | `data/local/applications/exports/applications.{csv,md}` | local-only |
| Fictional sample | `fixtures/application-ledger-sample.jsonl` | committed |

## CLI

```bash
cargo run -p ensembly-kernel -- apps validate
cargo run -p ensembly-kernel -- apps append --company "Example" --role "Engineer" \
  --link https://jobs.example.test/1 --apply-url https://jobs.example.test/1/application --status submitted \
  --submit-time 2026-10-26T18:59:00+01:00 --cv-version cv-sample-1
cargo run -p ensembly-kernel -- apps export   # byte-identical on re-run
```

Exports list current state only, ordered by submit instant, then link;
`prepared` rows (no time) come last.
