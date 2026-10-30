---
name: grill-rubric
description: Use when a change needs review before merge, when the author is about to approve their own work, or when two reviews of one change disagree
owner: flowingstack
source: master-workflow G3 G5
last-reviewed: 2026-10-30
---

# grill-rubric

## Flow

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  d["diff and contract"] --> p["interrogate, blast-radius if installed"]
  p --> f["findings TSV"] --> g["scripts/grill"]
  g --> ok["approve"]
  g --> ch["changes"]
  g --> sc["screen only"]
  classDef input fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef task fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef action fill:#fff3bf,stroke:#f08c00,color:#4a3000
  class d input
  class p,f,g task
  class ok,ch,sc action
```

## Contract

```text
// Signature
W        weights from references/rubric.md, fixed
F        findings: rubric id, severity
s        100 − Σ W(i) · factor(sev), floored, ≥ 0

// Axioms
A1  same F, same W → same s
A2  author = reviewer → verdict = screen
A3  blocker ∈ F → verdict ≠ approve
A4  verdict = approve ⟺ s ≥ 80 ∧ A2, A3 hold false
A5  id ∉ W ∨ sev ∉ {blocker, major, minor} → exit 2
```

| Verdict | Exit |
|---|---|
| approve | 0 |
| changes | 1 |
| bad input | 2 |
| screen | 3 |

## Checks

| Command | Expect |
|---|---|
| `scripts/grill --findings test/fixtures/grill/findings.tsv --author coder --reviewer grill` | `score=65 verdict=changes` |
| `python3 -m unittest discover -s test -p 'test_grill*'` | green |

## Notes

Self-review screens and never approves. The rubric hash in the output ties a score to one rubric version.
