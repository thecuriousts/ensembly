# rubric

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  f["findings TSV"] --> s["score"] --> v["verdict"]
  classDef input fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef task fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef action fill:#fff3bf,stroke:#f08c00,color:#4a3000
  class f input
  class s task
  class v action
```

| id | criterion | weight |
|---|---|---|
| R1 | correct on the contract check | 30 |
| R2 | test seen red before source | 20 |
| R3 | blast radius named and checked | 15 |
| R4 | privacy scan clean | 15 |
| R5 | scope matches contract | 10 |
| R6 | writing rule and doc-lint clean | 10 |

| severity | factor |
|---|---|
| blocker | 0, the verdict decides |
| major | 1.0 |
| minor | 0.3 |

| status | until |
|---|---|
| weights R1 to R6, threshold 80, factors: provisional | reviewer noise floor measured in the first real-work evaluation |
