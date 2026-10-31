---
name: flowingstack
description: Use first in any flowingstack task to pick which flowingstack skill to load next
owner: flowingstack
source: flowingstack
last-reviewed: 2026-10-30
---

# flowingstack

## Flow

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
  g["goal"] --> r["intelli-route"]
  r --> p["plan: task-contract, eval-freeze"]
  r --> u["run: run-ledger, tomato-loop, harvest"]
  r --> c["check: grill-rubric, doctor"]
  r --> s["capture: capture-sweep"]
  p & u & c & s --> o["≤ 4 skills loaded"]
  classDef input fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef task fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef action fill:#fff3bf,stroke:#f08c00,color:#4a3000
  class g input
  class r,p,u,c,s task
  class o action
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
