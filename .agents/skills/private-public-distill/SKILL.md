---
name: private-public-distill
description: Extract detailed alpha into .ignored folder, emit only gist + credits into tracked files
---

# private-public-distill

## When to use

Extract detailed alpha into .ignored folder, emit only gist + credits into tracked files

## Composability

- mode: `workflow`
- evidence: turn 9 narrative and tool sequence

## Steps

1. read_file whitepaper
2. run_terminal_command pdftotext
3. write private pack
4. write tracked gist

## Done when

Outputs are ready for the next skill in a parent workflow, or the user goal is met.
