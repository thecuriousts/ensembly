---
name: record-layer
description: >
  Write or change a shared fact under ensembly's record layer: one owner per
  fact, dated rows, write intent then act then verify, and idempotency keys.
  Use when authorizing, claiming, completing, committing a record, superseding
  a pulse, resolving who owns a fact, or retrying a write. Do not treat an
  attempt as a fact.
---

# record-layer

```mermaid
flowchart LR
  I[Intent plus owner, time, key] --> D{Accepted?}
  D -->|no| X[Refuse and stop]
  D -->|yes| A[Act]
  A --> V[Read the row back]
  V --> R[Dated row or rollback]
  classDef x fill:#1e3a5f,stroke:#93c5fd,color:#ffffff
  class I,D,X,A,V,R x
```

## Spec

| Step | Required | Refuse |
|------|----------|--------|
| Intent | Subject, intended change, owner, event time, source, idempotency key | Acting with no intent row |
| Owner | One writer per fact. Replicas import; they do not become canonical | Two live writers, or an agent claiming source `human` or `owner` |
| Act | Only after intent is accepted. Irreversible acts wait for the person | Logging an attempt as if it already succeeded |
| Verify | Read the row back (same transaction when the store allows). On refuse, roll back | Leaving a half-written fact |
| Retry | Send the same unit again. If every event is already logged unchanged, the store writes nothing and answers `"duplicate":true` | Inventing a second key to force a new row, or reusing a key with a different payload (that is a conflict and is refused) |

Record applied outcomes, not wishes. A later fact that replaces an older one must name the row it supersedes and keep the old row in archive.

When the kernel is present:

- Write: `ensembly record commit --unit '<json>'` (or MCP `record_commit`). The log defaults to `ENSEMBLY_RECORD_LOG`, else `paths.data_dir/record/events.jsonl`; pass `--log` to choose one.
- Read back: `ensembly record view snapshot` or `record view feed` (MCP `record_view_snapshot`, `record_view_feed`)
- Replace a live pulse: `ensembly pulse supersede` / `admit` with source, time, reason, and the configured writer identity

A unit is an event array (or `{"events": [...]}`). Every field below is required except `prev_hash`; unknown fields are refused:

```json
[{"id": "01JCV1Q8Z00000000000000000", "aggregate": "agg://example.test/acme-north/staff/1",
  "key": "staff-1-intent", "version": 1, "event_time": "2026-10-26T18:59:00+01:00",
  "record_time": "2026-10-26T19:00:00+01:00", "source": "document", "schema_v": 1,
  "payload": {"writer": "laptop-1", "note": "intent"}}]
```

More examples: `crates/ensembly-schema/fixtures/scenarios/`.

When the kernel is absent, apply the same table to files, pulse-memory, or any other store. Do not stand up a new Model Context Protocol server.

Public text uses host aliases only (`laptop-1`, `laptop-2`, `mac-mini`).
