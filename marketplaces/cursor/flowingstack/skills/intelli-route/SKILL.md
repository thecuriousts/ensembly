---
name: intelli-route
description: Use when a goal needs a route onto at most four skills, or when another skill says load intelli-route
owner: flowingstack
source: p10ns11y/plugins intelli-route at be043b2
last-reviewed: 2026-10-30
---

# intelli-route

## Flow

```agentflow-beta LR
  g["goal"]@{ shape: input } --> a["AGENTS.md decide"]@{ shape: task } --> r["bin/route --signals"]@{ shape: task }
  r --> e["route, loads ≤ 4, effort"]@{ shape: action }
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
