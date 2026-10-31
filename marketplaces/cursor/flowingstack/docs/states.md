# flowingstack states

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
stateDiagram-v2
  direction LR
  [*] --> Drafted
  Drafted --> Contracted: contract-check exit 0
  Drafted --> Blocked: no check or no counter
  Contracted --> Running: ledger open
  Running --> [*]
  Blocked --> Drafted: fix contract
```

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
stateDiagram-v2
  direction LR
  [*] --> Open: eval cases
  Open --> Frozen: eval-freeze lock
  Frozen --> Frozen: verify exit 0
  Frozen --> Tampered: verify exit 1
  Frozen --> Pending: override
  Pending --> Replayed: eval-replay
  Frozen --> Replayed: eval-replay
  Replayed --> Clean: regressions 0
  Replayed --> Regressed: exit 1
  Clean --> [*]
```

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
stateDiagram-v2
  direction LR
  [*] --> Opened: ledger open loads 4 or fewer
  [*] --> Refused: loads over 4 exit 2
  Opened --> Closed: ledger close
  Closed --> Claimed: ledger claim with source
  Claimed --> Compacted: ledger compact
  Compacted --> [*]
```

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
stateDiagram-v2
  direction LR
  [*] --> Round1: grill
  Round1 --> Approved: exit 0
  Round1 --> Round2: changes exit 1
  Round2 --> Approved: exit 0
  Round2 --> Round3: changes
  Round3 --> Approved: exit 0
  Round3 --> Escalated: rounds over 3 needs escalate
  Round1 --> Screen: same family exit 3
  Escalated --> Person
  Approved --> Person: ship lands nothing
  Person --> [*]
```

| Machine | Enter | Leave | Refuses |
|---|---|---|---|
| contract | `task-contract` | `ledger open` | empty check or counter |
| eval | `eval-freeze lock` | `eval-replay` | tamper or regression |
| ledger | `ledger open` | `ledger compact` | loads over 4 or claim without source |
| review | `grill` | person | same family or round 4 without `--escalate` |

A person lands every change.
