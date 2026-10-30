---
name: doctor
description: Use when starting work in a project for the first time, when setup looks incomplete, when CI fails for reasons outside the code, or when disk is filling
owner: flowingstack
source: master-workflow G1
last-reviewed: 2026-10-30
---

# doctor

## Flow

```agentflow-beta LR
  p["project"]@{ shape: input } --> d["scripts/doctor"]@{ shape: task }
  d --> r["report ok, missing, fail"]@{ shape: task }
  r --> s["/setup-pstack by the agent"]@{ shape: action }
```

## Contract

```text
// Signature
P        project dir
D        plugin dirs
M        P/.cursor/rules/pstack-models.mdc

// Axioms
A1  doctor reads P and D, writes nothing
A2  M missing → report /setup-pstack; ¬ write M
A3  disk ≥ 70% → fail disk
A4  CI status ∋ failure ∨ billing → fail ci-health
A5  exit 0 ⟺ no missing ∧ no fail
```

| Item | ok when |
|---|---|
| model-rule | M exists |
| agents-md | P/AGENTS.md exists |
| ci | P/.github/workflows holds a workflow |
| verify-skill | a skill named verify exists |
| hooks | P/.cursor/hooks.json exists |
| aliases | AGENTS.md or docs/host-aliases.md names laptop-1 or mac-mini |
| pstack, craft | found in D |
| disk | below 70% |

## Checks

| Command | Expect |
|---|---|
| `scripts/doctor --project P --plugins-dir D` | report, exit 0 or 1 |
| `scripts/doctor --project P --init` | `next: /setup-pstack`, tree unchanged |
| `python3 -m unittest discover -s test -p 'test_doctor*'` | green |

## Notes

Setup runs through the pstack command /setup-pstack in the agent. Doctor never writes pstack-models.mdc or any setup file.
