---
name: flowingstack
description: Use first in any flowingstack task to pick which flowingstack skill to load next
owner: flowingstack
source: flowingstack
last-reviewed: 2026-11-07
---

# flowingstack

## Flow

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  g["goal"] --> k["kernel"]
  k --> tc["task-contract"]
  k --> ef["eval-freeze"]
  k --> rl["run-ledger"]
  k --> gr["grill-rubric"]
  g --> s["support"]
  s --> ir["intelli-route"]
  s --> dr["doctor"]
  s --> rr["route-rule"]
  tc & ef & rl & gr --> o["at most 4"]
  classDef input fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef task fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef action fill:#fff3bf,stroke:#f08c00,color:#4a3000
  class g input
  class k,tc,ef,rl,gr,s,ir,dr,rr task
  class o action
```

## Contract

```text
A1  load(flowingstack) → load ≤ 4 more from the table
A2  default path: task-contract, eval-freeze, run-ledger, grill-rubric
A3  support: intelli-route, doctor, route-rule
A4  route-rule shipped roles: engineering, review, docs
A5  a user file adds a row or replaces one key
A6  engineering PR: task-contract, grill-rubric, references/verify.md
A7  craft and ub-mode load only when installed
A8  tomato-loop, harvest, capture-*, pstack-map, aihero-map stay off the default path
```

| Need | Load |
|---|---|
| route a goal | `intelli-route` |
| role for a task | `route-rule` |
| write input, output, check | `task-contract` |
| freeze or replay evals | `eval-freeze` |
| log a run or claim | `run-ledger` |
| review before merge | `grill-rubric` |
| setup or CI health | `doctor` |
| engineering PR | `task-contract`, `grill-rubric`, `references/verify.md` |
| optional focus block | `tomato-loop` |
| optional evidence row | `harvest` |
| optional capture | `capture-sweep`, `capture-mail`, `capture-calendar`, `capture-chat`, `capture-pr` |
| optional maps | `pstack-map`, `aihero-map` |
| optional extras when installed | `craft`, `ub-mode` |

## Checks

| Command | Expect |
|---|---|
| `python3 -m unittest discover -s test -p 'test_skills*'` | green |

## Notes

House desks stay in [examples/house-routing.tsv](../../examples/house-routing.tsv) and load only when a user path points there.
Shipped checks stay in [doctor.tsv](../../doctor.tsv). A project file can add or replace one check.
