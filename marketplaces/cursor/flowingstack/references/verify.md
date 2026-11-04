# verify

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart LR
  a["acceptance criteria"] --> w["work"] --> r["review pass, read only"]
  r --> d["confirmed defects"]
  r --> s["suspected defects"]
  r --> n["not checked"]
  r --> m["requirement marks with evidence"]
  classDef input fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef task fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef action fill:#fff3bf,stroke:#f08c00,color:#4a3000
  class a input
  class w,r task
  class d,s,n,m action
```

```text
// Signature
Q        requirements, written done before work starts
mark(q)  Verified | Not met | Not checked
e(q)     evidence: command and output, source, or calculation

// Axioms
V1  Q = ∅ → no work starts
V2  review pass ≠ work pass; reviewer lists confirmed, suspected, not checked apart
V3  ∀ q ∈ Q: mark(q) given ∧ (mark(q) = Not checked ∨ e(q) ≠ ∅)
V4  claim without its arithmetic or test → Not checked
V5  review is read only: no publish, send or edit
```

Source: X article Meet Your Cheapest Employee, 26 Oct 2026, prompt 4, skill 2 and section 7.
