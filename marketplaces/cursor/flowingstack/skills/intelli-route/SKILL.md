---
name: intelli-route
description: Use when a goal needs a route onto at most four skills, or when another skill says load intelli-route
owner: flowingstack
source: p10ns11y/plugins intelli-route at be043b2
last-reviewed: 2026-10-30
---

# intelli-route

## Flow

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  g["goal"] --> a["AGENTS.md decide"] --> r["bin/route --signals"]
  r --> e["route, loads ≤ 4, effort"]
  classDef input fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef task fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef action fill:#fff3bf,stroke:#f08c00,color:#4a3000
  class g input
  class a,r task
  class e action
```

## Contract

```text
// Axioms
A1  rules = AGENTS.md, routes = manifest.json, both vendored
A2  |loads| ≤ 4
A3  house row of AGENTS.md → skills/pstack-map/references/map.md
```

## Checks

| Command | Expect |
|---|---|
| `skills/intelli-route/bin/route --signals '{}'` | `{"effort": "high", "reason": "normal feature"}` |

## Notes

Read AGENTS.md first.
