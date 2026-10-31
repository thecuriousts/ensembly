# peers

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  m["mention in a vendored skill"] --> s{"ships here"}
  s --> run["run it"]
  s --> p["peer row below"]
  classDef input fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef action fill:#fff3bf,stroke:#f08c00,color:#4a3000
  class m input
  class run,p action
```

| file | mention | when missing |
|---|---|---|
| skills/aihero-map/references/map.md | `bin/steer-log` | install the steer-log plugin; without it, hold the card |
| skills/intelli-route/AGENTS.md | `references/map.md` | read skills/pstack-map/references/map.md |
