# flowingstack skill graph

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
graph LR
  hub["flowingstack"]
  hub --> ir["intelli-route"] & tc["task-contract"] & ef["eval-freeze"] & rl["run-ledger"] & gr["grill-rubric"] & dr["doctor"]
  hub --> pm["pstack-map"] & am["aihero-map"] & tl["tomato-loop"] & hv["harvest"] & cs["capture-sweep"]
  ir <--> pm
  am --> ir & pm
  tc --> ir
  ef <--> rl
  tl --> hv & ir & pm
  cs <--> cc["capture-calendar"] & ch["capture-chat"] & cm["capture-mail"] & cp["capture-pr"]
  dr & tc & ef & rl & gr & ir -.-> hub
  classDef src fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef run fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef stop fill:#ffe3e3,stroke:#e03131,color:#4a0a0a
  classDef out fill:#fff3bf,stroke:#f08c00,color:#4a3000
  classDef who fill:#e5dbff,stroke:#7048e8,color:#2b1a5e
  class hub out
  class ir,pm,am src
  class tc,ef,rl,gr,dr run
  class tl,hv,cs,cc,ch,cm,cp who
```

| Group | Skills | Role |
|---|---|---|
| hub | flowingstack | picks the next skill |
| route | intelli-route pstack-map aihero-map | at most 4 loads |
| task | task-contract eval-freeze run-ledger grill-rubric doctor | scripted checks |
| between sessions | tomato-loop harvest capture-sweep capture-* | read only |

Edges come from skill names each SKILL.md mentions.
