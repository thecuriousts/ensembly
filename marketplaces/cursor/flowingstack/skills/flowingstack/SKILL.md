---
name: flowingstack
description: Use first in any flowingstack task to pick which flowingstack skill to load next
owner: flowingstack
source: flowingstack
last-reviewed: 2026-10-30
---

# flowingstack

## Flow

```agentflow-beta LR
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
  g["goal"]@{ shape: input } --> r["intelli-route"]@{ shape: task }
  r --> p["plan: task-contract, eval-freeze"]@{ shape: task }
  r --> u["run: run-ledger, tomato-loop, harvest"]@{ shape: task }
  r --> c["check: grill-rubric, doctor"]@{ shape: task }
  r --> s["capture: capture-sweep"]@{ shape: task }
  p & u & c & s --> o["≤ 4 skills loaded"]@{ shape: action }
```

## Contract

```text
A1  load(flowingstack) → load ≤ 4 more, chosen below
A2  unsure → intelli-route
```

| Need | Load |
|---|---|
| route a goal | `intelli-route` |
| write input, output, check | `task-contract` |
| freeze or replay evals | `eval-freeze` |
| log a run or claim | `run-ledger` |
| review before merge | `grill-rubric` |
| setup or CI health | `doctor` |
| one focus block | `tomato-loop` |
| block evidence to a row | `harvest` |
| read mail, calendar, chat, PRs | `capture-sweep`, `capture-mail`, `capture-calendar`, `capture-chat`, `capture-pr` |
| house skill map | `pstack-map`, `aihero-map` |

## Checks

| Command | Expect |
|---|---|
| `python3 -m unittest discover -s test -p 'test_skills*'` | green |
