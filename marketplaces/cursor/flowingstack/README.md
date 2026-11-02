# flowingstack

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  src["p10ns11y/plugins at pin"] --> sync["sync-from-upstream"] --> skills["skills"]
  skills --> lint["doc-lint"] --> priv["privacy-scan"] --> ok["exit 0"]
  classDef task fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  class src,sync,skills,lint,priv,ok task
```

## Diagrams

| Page | Shows | Image |
|---|---|---|
| [flow](docs/flow.md) | end to end flow | [svg](docs/img/flow-1.svg) |
| [states](docs/states.md) | contract, eval, ledger, review | [svg](docs/img/states-4.svg) |
| [skills](docs/skills.md) | skill graph | [svg](docs/img/skills-1.svg) |
| [connections](docs/connections.md) | scripts and files | [svg](docs/img/connections-1.svg) |
| [sequence](docs/sequence.md) | brief to ship | [svg](docs/img/sequence-1.svg) |
| [install](docs/install.md) | install and sync | [svg](docs/img/install-1.svg) |
| [harness](docs/harness.md) | Cursor, Grok Bot, other lanes | [svg](docs/img/harness-1.svg) |
| [formal](docs/formal.md) | statechart, TLC, Kani, Bend | [svg](docs/img/formal-1.svg) |

## Flow

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  g["goal"] --> hub["flowingstack"] --> ir["intelli-route"]
  ir --> tc["task-contract"] --> cc["contract-check"]
  cc --> run["run"] --> gr["grill"] --> led["ledger"]
  tc --> ef["eval-freeze"] --> led
  classDef src fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef act fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef out fill:#fff3bf,stroke:#f08c00,color:#4a3000
  class g,hub,ir src
  class tc,cc,run,gr,ef act
  class led out
```

## Stages

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
stateDiagram-v2
  [*] --> Contract: task-contract
  Contract --> Open: ledger open
  Open --> Running: loads ≤ 4
  Running --> Review: grill
  Review --> Running: request changes
  Review --> Closed: ledger close
  Closed --> Claimed: ledger claim
  Claimed --> [*]
```

## Ledger

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
sequenceDiagram
  participant A as agent
  participant L as ledger.tsv
  A->>L: open contract route loads
  L-->>A: run id t
  A->>L: close t opened switches failed-fixes
  A->>L: claim t text status source
  A->>L: compact
  Note over L: first open, last close, last claim
```

```text
// Signature
R      plugin root, FLOWINGSTACK_ROOT or --root
P      pins.tsv rows: repo sha40 path dest
B      baseline rows: path rule count

// Axioms
A1  ∀ path p: p relative to R ∧ ".." ∉ p
A2  scan(f) → f ∈ R ∨ f = repo/.cursor-plugin/marketplace.json
A3  check: tree(dest) = rewrite(tree(src@sha)) ∧ NOTICE = f(P, rewrites.tsv)
A4  symlink ∨ binary → exit 1
A5  hits(path, rule) > count → all live, exit 1; row without count → exit 2
```

| Command | Exit 0 | Exit 1 | Exit 2 |
|---|---|---|---|
| `scripts/doc-lint --all` | clean | finding | absolute path |
| `scripts/privacy-scan --all [--repo-root D]` | clean | hit | — |
| `scripts/sync-from-upstream --src D [--check]` | synced or no drift | drift, binary, symlink | bad pin |
| `scripts/check --src D` | all three clean | any fail | bad args |
| `scripts/formal-check [--bend]` | proved | counterexample | — (3 = tool missing, skip) |
| `skills/intelli-route/bin/route WHEN \| --signals JSON` | route; `task` matching formal-verification adds loads | missing map or when | bad signals |
| `python3 -m unittest discover -s test` | green | red | — |

Every script and how they connect: [docs/flow.md](docs/flow.md).

## Route rule

[route-rule skill](skills/route-rule/SKILL.md) loads first each Chief of Staff turn and runs [scripts/route-rule](scripts/route-rule) on [routing.tsv](routing.tsv)
