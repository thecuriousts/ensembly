---
name: circular-import-break
description: Make dependent module pure (no import of turn) to resolve bidirectional dependency
---

# circular-import-break

## When to use

Make dependent module pure (no import of turn) to resolve bidirectional dependency

## Composability

- mode: `workflow`
- evidence: turn 5: play.js made pure, 146 tests pass

## Steps

1. grep
2. search_replace
3. run_terminal_command

## Done when

Outputs are ready for the next skill in a parent workflow, or the user goal is met.
