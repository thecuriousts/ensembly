---
name: verified-retrieve
description: >
  Retrieve one verified pulse instead of dumping context. Use when reading
  memory, pulsing, prefetching, checking a fact, assembling context for
  another agent, or when two notes disagree. Prefer a dated sourced row.
  Do not average contradictions or fill empty check fields from an archive dump.
---

# verified-retrieve

```mermaid
flowchart TD
  A[Ask one subject] --> C[Check or read the live pulse]
  C -->|row found| P[Hand one dated sourced pulse]
  C -->|empty or unknown| U[Say unknown and keep the pointer]
  C -->|conflict| X[Return both rows tagged, do not average]
  classDef x fill:#1e3a5f,stroke:#93c5fd,color:#ffffff
  class A,C,P,U,X x
```

## Spec

| Rule | Do | Do not |
|------|----|--------|
| One pulse | Return the live row for this next action | Paste a transcript, history dump, or whole store |
| Freshness | Prefer later time, then higher status | Blend two stories into one fluent paragraph |
| Status | `tool-verified` > `user-stated` > `inferred` > `unknown` | Promote an inferred fill to locked |
| Empty check | If check or prefetch is empty, say unknown and keep the pointer | Backfill from archive or invent a value |
| Conflict | Tag both rows and leave them in evidence | Average, or pick a side to sound sure |

Hint tags on every pulse you hand on: `source`, `time`, `status`, `kind`. Untagged text is a hypothesis.

When the kernel is present, stay on the read path: `ensembly pulse check`, `pulse read`, or `pulse list` (live memory). `pulse history` is archive. Use it only when you must trace a supersession, not as default context.

When the kernel is absent, ask the store you have for one dated row with the same tags (pulse-memory is the usual companion). Still do not dump.

After retrieve, if the next step is a write, load `record-layer`.
