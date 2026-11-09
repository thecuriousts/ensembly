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
sequenceDiagram
  participant B as Bot
  participant L as Record layer
  B->>L: Intent plus owner, time, key
  L-->>B: Accepted or refused
  B->>L: Act
  B->>L: Read back the row
  L-->>B: Dated row or rollback
  classDef x fill:#1e3a5f,stroke:#93c5fd,color:#ffffff
  class B,L x
```

## Spec

| Step | Required | Refuse |
|------|----------|--------|
| Intent | Subject, intended change, owner, event time, source, idempotency key | Acting with no intent row |
| Owner | One writer per fact. Replicas import; they do not become canonical | Two live writers, or an agent claiming source `human` or `owner` |
| Act | Only after intent is accepted. Irreversible acts wait for the person | Logging an attempt as if it already succeeded |
| Verify | Read the row back (same transaction when the store allows). On refuse, roll back | Leaving a half-written fact |
| Retry | Send the same idempotency key. The store no-ops a duplicate | Inventing a second key to force a new row |

Record applied outcomes, not wishes. A later fact that replaces an older one must name the row it supersedes and keep the old row in archive.

When the kernel is present:

- Write: `ensembly record commit` (or MCP `record_commit`)
- Read back: `ensembly record view snapshot` or `record view feed` (MCP `record_view_snapshot`, `record_view_feed`)
- Replace a live pulse: `ensembly pulse supersede` / `admit` with source, time, reason, and a public host alias

When the kernel is absent, apply the same table to files, pulse-memory, or any other store. Do not stand up a new Model Context Protocol server.

Public text uses host aliases only (`laptop-1`, `laptop-2`, `mac-mini`).
