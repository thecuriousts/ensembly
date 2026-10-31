# flowingstack formal

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  sc["statechart replay<br/>every change"] --> tlc["TLC<br/>every ordering"] --> kani["Kani<br/>Rust hosts only"] --> bend["Bend 2<br/>trial, unchecked"]
  tsv["formal/machine.tsv"] --> sc
  tsv --> tlc
  classDef input fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef task fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef action fill:#fff3bf,stroke:#f08c00,color:#4a3000
  class tsv input
  class sc,tlc task
  class kani,bend action
```

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
stateDiagram-v2
  direction LR
  [*] --> drafted
  drafted --> contracted: contract-check exit 0
  contracted --> frozen: eval-freeze lock
  frozen --> running: ledger open
  running --> review: grill
  review --> review: changes under 3
  review --> approved: exit 0
  review --> escalated: round 3 changes
  approved --> shipped: ship ready
  approved --> escalated: eval tampered
  shipped --> closed: ledger close
  escalated --> closed: ledger close escalate
  closed --> [*]
```

| Property | Kind | Means | Breaks when |
|---|---|---|---|
| RoundCap | invariant | review round never above 3 | MaxRounds 4 |
| ShipFrozen | invariant | shipped implies frozen evals | Ship without frozen |
| AppendOnly | action | ledger only grows | a row is rewritten |
| EventuallyClosed | liveness | every run closes | a dead end state |

| Command | Exit |
|---|---|
| `scripts/formal-check` | 0 proved, 1 counterexample, 3 java or jar missing (skip) |
| `scripts/formal-check --bend` | 0 law holds, 1 fails, 3 bend missing (skip) |

Edges live in `formal/machine.tsv`. Tests hold the spec, this chart and the replay of the real scripts to it.
TLC: set `TLA2TOOLS` to a tla2tools.jar, or the script fetches v1.8.0 by sha256 into the user cache.
Statechart handoff: read TLC output with the pstack statechart skill.
Kani: only for hosts with Rust code, for parsers and byte handling; no Python here.
Bend 2: `formal/bend/ledger.bend` states close is idempotent; unchecked until bend runs.
