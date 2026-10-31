# flowingstack evidence

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  ca["cloud agent<br/>plugin via local symlink"] --> rt["intelli-route"] --> d["5 defects"] --> fx["fixed, test first"]
  classDef input fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef task fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  class ca,rt input
  class d,fx task
```

| Defect found by routing | Test |
|---|---|
| WHEN lookup missed vendored map.md | test_route When |
| no formal-verification signal or loads | test_route FormalSignal |
| intelli-route SKILL and README out of date | test_route Docs |
| doctor verify-skill glob missed house skills | test_doctor house verify |
| TLA2TOOLS not documented for statechart handoff | test_route Docs |

Used:
- plugins: flowingstack (installed in a cloud agent via local symlink), pstack
- skills: test-driven-development, public-host-alias, verification-before-completion, source-honesty, principle-boundary-discipline, principle-encode-lessons-in-structure, writing-skills, bot-rules, create-formal-verification-skill
- flowingstack skills the cloud agent loaded: flowingstack, intelli-route, task-contract, grill-rubric, eval-freeze, run-ledger, doctor
