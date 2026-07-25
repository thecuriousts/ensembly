---
name: approval-gate-mapping
description: Map dry_run_ok/executed/failed states to closed approved gate; pending_auth stays open
---

# approval-gate-mapping

## When to use

Map dry_run_ok/executed/failed states to closed approved gate; pending_auth stays open

## Composability

- mode: `workflow`
- evidence: turn 7: skeptic gaps fixed by editing flowToApprovalRecord and snapshot assertions

## Steps

1. grep
2. search_replace
3. run_terminal_command

## Done when

Outputs are ready for the next skill in a parent workflow, or the user goal is met.
