# Product charter — ensembly / Game of Peram

**Status:** Binding product intent  
**Audience:** Operator, implementers, coding agents  
**Last updated:** 2026-07-13

---

## Mission

Remove **digital friction** so scarce human energy goes to **physical presence**, **judgment (approvals)**, and **growth** — while the swarm curates, prioritizes, balances, and classifies. Make the control plane legible and **fun** as the **Game of Peram**: a world you inhabit, not a backlog you endure.

This is **serious life infrastructure**. Not a joke. Not a hobby demo. Production-grade from day 0.

**Not the vault:** Portfolio memory of projects started and clustered lives in **`~/life-os`**. ensembly is the **digital clone** that works the digital layer continuously (or frequently) so you **pair** when you can — body work + HITL, not full-time digital thrash. Boundary: [LIFE-OS-BOUNDARY.md](LIFE-OS-BOUNDARY.md).

---

## Success (how we know it worked)

| Horizon | Signal |
|---------|--------|
| **Today** | Operator opens turn or game and *knows* the next physical act and the next authorization |
| **This week** | Day plans are used, not ignored; approve/deny actually gates risk; focus never lies |
| **This quarter** | Measurable less thrash (fewer “what should I do?” loops); balance (family/health) holds under career pressure |
| **This year** | Growth compounds: craft, relationships, health, and career move in one coherent story the swarm can restate |
| **2036** | Kernel still pure and private; hosts (game, Eve channels, desktop) swap; human judgment remains the scarce resource |

If a feature cannot connect to a row above, it is entertainment debt or architecture tourism — defer it.

---

## Product pillars

### 1. Impact (life)

- **Physical first** — Surface what only a body can do. Agents never cosplay errands.
- **Authorization honest** — Pending gates are first-class state, resumable, never buried in prose.
- **Balance enforced** — Relationships and health capacity are scheduled before career noise fills the day.
- **Privacy default-deny** — Useful locally; shareable only when classified public. Vault never rides a casual deploy.

### 2. Engagement (fun)

- **World > chrome** — Courtyard, sprites, props, motion, focus beacon; HUD is secondary glass.
- **Input feels immediate** — Keys, voice, gamepad; single source of truth so the avatar and labels never disagree.
- **Growth loop** — Claim physical/presence/craft beacons for XP; clear HITL gates; streak combos; balance axes (Body · Presence · Craft · Gates). Coach steers away from craft-only thrash. See `src/game/growth.js`.
- **Feedback loops** — XP toast, path glow, claimed beacons, quest board — every real act *registers*.
- **Craft over gimmick** — Prefer one polished growth interaction over five dead buttons.

### 3. Growth (ascent)

- Plans and graphs should make **progress visible**, not just tasks listed.
- The persona model optimizes for **capacity and goal attainment**, not inbox zero cosplay.
- Digital automation exists so the human can invest in **irreplaceable growth work** (presence, deep craft, health, relationships).

### 4. Production grade (day 0)

| Standard | Practice |
|----------|----------|
| Real entry paths | `npm test`, `npm run game`, `swarm turn/day/graph` work on clean clone (+ prebuilt WASM) |
| Shipped-path tests | Unit + smoke; no orphan modules |
| Durable HITL | Snapshots/approvals survive process restart |
| Host adapters | Eve (when built) is production channels + approval + cron — **not** a throwaway prototype |
| Observability | Operator can see mode, focus, pending, engine; agents leave traces in tests/docs |
| Reversibility | Undo where user intent can be wrong; no silent external mutate |

---

## Explicit non-goals (for now)

- AAA open world, multiplayer MMO, or voice room as **gate** to usefulness  
- Unattended finance or email  
- Shipping private persona  
- Rewriting the kernel into a prompt-only Eve agent  
- Legacy webpack SPA as the product  
- Absorbing `~/life-os` vault content into this git tree (vault remains portfolio of record)  
- Making the Obsidian vault the connector/daemon host (hooks/connectors are ensembly trajectory)

---

## Host strategy (serious, not sequential toys)

| Host | Role | Bar |
|------|------|-----|
| **CLI swarm** | Auditable day/turn/approve/graph | Fast, scriptable, CI-true |
| **Game of Peram** | Daily *felt* situational awareness | Fun, immersive, focus-correct |
| **Eve bridge** | Remote comms, remote approval, schedules | Production deploy, redacted I/O, durable park — when built, built to run life, not demo Slack |
| **Desktop (later)** | Same Rust world core | Share sim, thin shell |

No “prototype Eve then maybe real.” When Eve lands, it is the production remote surface for this operator’s life loop. Until then, CLI + game must already be production-quality daily tools.

---

## Operator contract

**You give:** attention for physical work + explicit yes/no on risk.  
**You get:** a day plan that respects capacity, a pending queue you can trust, a graph/world that tells the truth, and automation of digital thrash.  
**You never get:** surprise side effects, private data in public git, or a product that wastes your play energy on broken toys.

---

## Agent contract

Coding agents treat this charter + [AGENTS.md](../AGENTS.md) as law:

1. Prefer finishing production surfaces over opening new stubs.  
2. Never ship joke UX or desynced state.  
3. Measure done by **operator life impact**, not file count.  
4. When adding cloud/remote paths, enforce privacy redaction as hard as tests.  
5. Keep the Game of Peram **actually fun** — impeccable UX is not optional polish.

---

## References

| Doc | Role |
|-----|------|
| [AGENTS.md](../AGENTS.md) | Session binding rules |
| [PRIVACY.md](PRIVACY.md) | Data boundary |
| [EVE-FIT.md](EVE-FIT.md) | What Eve owns |
| [SWARM-DESIGN.md](SWARM-DESIGN.md) | Day loop |
| [arch-design/coming-next.md](arch-design/coming-next.md) | Roadmap |
| [WORLD-FOUNDATION.md](WORLD-FOUNDATION.md) | Immersive foundation |

---

**Footer plain rule:** Automate the digital; surface the physical; wait only for permission; make the truth playable — and ship it like the year depends on it.
