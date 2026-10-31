# flowingstack harness lanes

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  cu["Cursor"] & gb["Grok Bot"] & ot["other harness"] --> led[("ledger.tsv --harness")]
  led --> gr["grill family check"] --> pe(["person"])
  classDef src fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef run fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef stop fill:#ffe3e3,stroke:#e03131,color:#4a0a0a
  classDef out fill:#fff3bf,stroke:#f08c00,color:#4a3000
  classDef who fill:#e5dbff,stroke:#7048e8,color:#2b1a5e
  class cu,gb,ot src
  class gr run
  class led out
  class pe who
```

| Harness | Reads | Format note |
|---|---|---|
| Cursor | plugin skills | marketplace entry |
| Grok Bot | skills from the repo | plain SKILL.md, no plugin manifest needed |
| other harness | SKILL.md | same |

Record the lane with `ledger open --harness NAME`.
