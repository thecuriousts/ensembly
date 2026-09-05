# Product charter — ensembly operator kernel

**Status:** Binding product intent  
**Audience:** Operator, implementers, coding agents  
**Last updated:** 2026-09-05 (ensembly-* crate rename)

---

## Mission

Provide a **durable operator kernel** under external harnesses (Grok Bot, Grok Build, Cursor): **done / pending / denied** as data you own — gates, episodic memory, pulse sync — so capture tools do not re-litigate the same HITL state every session.

Remove **digital friction** so scarce human energy goes to **physical presence** and **judgment (approvals)**. This is **serious life infrastructure**. Not a joke. Not a hobby demo.

**Not the vault:** Portfolio memory lives in **`~/life-os`**. ensembly is the **kernel** the clone runs locally. Boundary: [LIFE-OS-BOUNDARY.md](LIFE-OS-BOUNDARY.md).

**Not a game product (at root):** Game of Peram browser client is **parked** in [`prototype/`](../prototype/README.md) — optional experiment, not SoT.

---

## Market position

Mass-market agent services optimize **capture at scale**. They cannot be the durable ledger for one operator's pending gates, learned workflows, and episodic traces.

| Layer | Role |
|-------|------|
| **Grok Bot / Build / Cursor** | Capture, codegen, chat — their surface |
| **ensembly** | White hole — gates, memory CRDT, pulse-pack, `ensembly-mcp` read wire |
| **prototype/** | Parked game/watch — not maintained as product |

**Thesis:** Agent platforms sell inference. ensembly sells the **operator layer underneath**: one pending ledger, HITL honesty, privacy default-deny, pulse sync without dual writers.

Cut record: [MUSK-CUT-2026-09-04.md](MUSK-CUT-2026-09-04.md).

---

## Success

| Horizon | Signal |
|---------|--------|
| **Today** | `cargo run -p ensembly-kernel -- runtime status` shows honest regime and pending gates |
| **This week** | Grok session ends with pulse export; laptop import merges memory without ops conflict |
| **This quarter** | Harness + kernel loop replaces re-explaining pending auth in chat |
| **This year** | Kernel pure and private; hosts swap; human judgment remains scarce |

If a feature cannot connect to a row above, defer it.

---

## Product pillars

### 1. Impact (life)

- **Physical first** — Kernel surfaces physical beacons; agents do not cosplay errands.
- **Authorization honest** — Auth gates are durable, resumable state in T1 SQLite.
- **Privacy default-deny** — Useful locally; shareable only when classified public.

### 2. Complementary (harness fit)

- **Kernel under capture** — Grok proposes; kernel records outcomes.
- **Read-only MCP** — Agents query memory; they do not own ops DB.
- **Pulse not dual-write** — Portable memory sync; canonical host holds ops.

### 3. Growth (ascent)

- Episodic `reflect` makes progress visible without chat amnesia.
- Automation exists so humans invest in irreplaceable work.

### 4. Production grade (day 0)

| Standard | Practice |
|----------|----------|
| Dogfood | `cargo test -p ensembly-kernel` on every change |
| Durability | T1 SQLite + sealed backup paths tested |
| Loud failure | CLI status lines; no silent gate drift |
| Single writer | One canonical host for the ops sqlite (`ensembly-ops.sqlite`; legacy `peram-ops.sqlite` discovered) |

---

## Non-goals (root repo)

- Browser game as primary surface
- Node wait-snapshot as co-equal SoT
- Live cloud sync / dual master
- Second chat OS or plugin sprawl
- Multiplayer / Eve / hub constellation as near-term build

See `prototype/` for preserved experiments.

---

## Canonical references

| Doc | Role |
|-----|------|
| [MAP.md](MAP.md) | Live kernel map |
| [PLAYBOOK.md](PLAYBOOK.md) | Operator dogfood |
| [PRIVACY.md](PRIVACY.md) | Push boundary |
| [DECISIONS.md](DECISIONS.md) | Architecture log |

**Footer:** Complement the harness. Own the gates.
