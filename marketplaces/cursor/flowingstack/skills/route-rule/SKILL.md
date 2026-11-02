---
name: route-rule
description: Use first on every Chief of Staff turn before any work and run scripts/route-rule --task
owner: flowingstack
source: routing facts set 31 Oct 2026
last-reviewed: 2026-10-31
---

# route-rule

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  t["task"] --> r["route-rule --task"]
  r -->|match| l["route to lead"]
  r -->|no match exit 2| c["ask Chief of Staff"]
  l --> s["do not do it yourself"]
  classDef src fill:#d0ebff,stroke:#1971c2,color:#0b3d6e
  classDef act fill:#d3f9d8,stroke:#2f9e44,color:#14532d
  classDef out fill:#fff3bf,stroke:#f08c00,color:#4a3000
  class t src
  class r act
  class l,s,c out
```

| Command | Out | Exit |
|---|---|---|
| `scripts/route-rule --task "x"` | type lead rule | 0 or 2 |
| `scripts/route-rule --list` | table | 0 or 2 |
| `--json` | one JSON object | same |
| bad table | one line | 2 |

Table [routing.tsv](../../routing.tsv) Script [route-rule](../../scripts/route-rule)
No pre-turn hook runner exists so the bot loads this skill first
Irreversible yes goes to Chief of Staff in the 1:1
