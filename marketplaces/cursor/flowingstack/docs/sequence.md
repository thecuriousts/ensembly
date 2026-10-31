# flowingstack task sequence

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
sequenceDiagram
  autonumber
  actor P as person
  participant A as agent
  participant S as scripts
  participant L as ledger.tsv
  participant R as reviewer
  P->>A: brief
  A->>S: brief-lint
  A->>S: contract-check
  A->>L: open loads 4 or fewer
  A->>S: eval-freeze lock
  loop red then green
    A->>A: failing test then code
  end
  A->>S: eval-replay
  loop at most 3 rounds
    A->>R: grill
    R-->>A: verdict
  end
  A->>L: close and claim with source
  A->>S: ship
  S-->>P: all checks exit 0
  P->>P: lands the change
```

| Step | Exit 0 means |
|---|---|
| brief-lint | skills snapshot and PR body rule present |
| contract-check | route found |
| eval-replay | regressions 0 |
| grill | approve |
| ship | ready for a person |
