# flowingstack connections

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  pins[("pins.tsv")] --> sync["sync-from-upstream"]
  rw[("rewrites.tsv")] --> sync
  sync --> sk[("skills/")] & nt[("NOTICE.md")]
  tpl[("templates/contract.yml")] --> ct[("contract yml")]
  ct --> cc["contract-check"] --> led[("ledger.tsv")]
  ct --> br["brief-lint"]
  ev[("evals/*.case")] --> ef["eval-freeze"] --> lk[("frozen hash")]
  lk --> er["eval-replay"]
  fi[("findings tsv")] --> gr["grill"] --> led
  led --> lq["ledger query"]
  sk --> dl["doc-lint"] & ps["privacy-scan"]
  bl[("baselines")] --> dl & ps
  cc & ef & ps & dl & dr["doctor"] --> sh["ship"]
  sh --> person(["person lands"])
  classDef src fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef run fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef stop fill:#ffe3e3,stroke:#e03131,color:#4a0a0a
  classDef out fill:#fff3bf,stroke:#f08c00,color:#4a3000
  classDef who fill:#e5dbff,stroke:#7048e8,color:#2b1a5e
  class pins,rw,tpl,ct,ev,fi,bl src
  class sync,cc,ef,er,gr,br,dr,lq run
  class dl,ps stop
  class sk,nt,led,lk,sh out
  class person who
```

| File | Written by | Read by |
|---|---|---|
| `pins.tsv` | person | `sync-from-upstream` |
| `rewrites.tsv` | person | `sync-from-upstream` |
| contract yml | `task-contract` | `contract-check` `ledger` `ship` |
| frozen hash | `eval-freeze lock` | `verify` `eval-replay` |
| `ledger.tsv` | `ledger` `grill` | `ledger query` |
| baselines | person | `doc-lint` `privacy-scan` |

Cylinders are files. Boxes are scripts.
