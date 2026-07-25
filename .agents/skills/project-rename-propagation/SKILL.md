---
name: project-rename-propagation
description: Rename package/dir/CLI then update all references, docs and gitignored entries in one pass
---

# project-rename-propagation

## When to use

Rename package/dir/CLI then update all references, docs and gitignored entries in one pass

## Composability

- mode: `workflow`
- evidence: turn 9 finance-dd to wealth-dd rename sequence

## Steps

1. run_terminal_command
2. grep
3. search_replace

## Done when

Outputs are ready for the next skill in a parent workflow, or the user goal is met.
