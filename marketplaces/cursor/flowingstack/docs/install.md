# flowingstack install and sync

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart TD
  mk["marketplace.json"] --> inst["install plugin"] --> dr["doctor --project"]
  dr -->|missing| fix["setup steps"] --> dr
  dr -->|exit 0| use["use skills"]
  up["upstream at pinned sha"] --> sync["sync-from-upstream"]
  pins[("pins.tsv")] & rw[("rewrites.tsv")] --> sync
  sync -->|drift binary symlink| stop["exit 1"]
  sync -->|bad source or pin| bad["exit 2"]
  sync -->|ok| sk[("skills/ NOTICE.md")] --> chk["check"] --> use
  classDef src fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef run fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  classDef stop fill:#ffe3e3,stroke:#e03131,color:#4a0a0a
  classDef out fill:#fff3bf,stroke:#f08c00,color:#4a3000
  classDef who fill:#e5dbff,stroke:#7048e8,color:#2b1a5e
  class mk,up,pins,rw src
  class inst,dr,sync,chk,fix run
  class stop,bad stop
  class sk,use out
```

| Step | Command | Fails when |
|---|---|---|
| install | Cursor marketplace | entry has fields past name source description |
| setup | `scripts/doctor --project .` | setup gap |
| sync | `scripts/sync-from-upstream` | drift, bad pin or source |
| check | `scripts/check` | any check fails |
