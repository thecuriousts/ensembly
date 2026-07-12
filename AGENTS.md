# AGENTS.md — binding rules for ensembly / Game of Peram

Read this before any non-trivial change. These are **product law**, not suggestions.

---

## 0. What this project is

**ensembly** is production-grade life infrastructure for one operator (and, later, people who share the same bar). It is **not** a demo, weekend toy, portfolio piece, or “MVP to throw away.”

It must be:

| Pillar | Meaning |
|--------|---------|
| **Impactful** | Changes real days: less digital thrash, clearer physical work, honest HITL, measurable balance |
| **Engaging** | Game of Peram is *played* — focus, motion, feedback, delight — not a dashboard with game skin |
| **Growth-oriented** | Capacity, relationships, health, craft, and career compound; the swarm serves *ascent*, not busywork |
| **Production-grade from day 0** | Ship paths that survive deploys, reloads, tests, privacy audits, and daily dogfood |

If a change would be fine “for a hobby” but would embarrass a product you bet your year on, **do not ship it**.

---

## 1. Hard refuses

| Refuse | Why |
|--------|-----|
| Prototype theater | No half-wired demos, fake data as the product surface, or “we’ll harden later” |
| Joke UX | No placeholder lorem, broken focus, desynced HUD/world, dead buttons, silent failures |
| Hobby architecture | No throwaway modules that cannot grow into desktop/WASM shared core or a real Eve bridge |
| Privacy laziness | Never commit or push `private/`, vaults, secrets, or unredacted life data |
| Unattended bank/email | External mutate only behind explicit human authorization |
| Kernel rewrite thrash | Do not burn the pure day/privacy/realm/approvals IR to chase a host framework |
| Scope cosplay | Multiplayer voice rooms, AAA art, or framework tourism before the daily loop is *lived* |

---

## 2. Production bar (every PR / agent session)

1. **Dogfood path** — There is a real command or URL the operator can run today (`npm test`, `npm run game`, `swarm turn`, etc.).
2. **Tests on the shipped path** — Pure logic unit-tested; smoke for launch surfaces; no “tests for code that isn’t wired.”
3. **Failure is loud and recoverable** — Status lines, errors, undo where it matters; no silent wrong focus/state.
4. **Privacy default-deny** — Classifier + gitignore + docs stay green; redaction at every cloud/channel boundary.
5. **Single source of truth** — Session/store owns operator intent (e.g. focus); renderers and WASM *mirror*.
6. **Fun is a requirement** — Input latency, visual hierarchy, sound/haptics later if needed; the world must feel *alive*.
7. **Impact is a requirement** — Features must map to: less friction, clearer next physical act, safer auth, better balance, or growth signal — or they wait.

---

## 3. Architecture law

```text
Kernel (local, pure, audited)     → day, privacy, realm, approvals IR, graph IR, looper
Host (game / CLI / desktop)       → input, paint, local store
Bridge (Eve optional, production) → channels, remote approval UX, cron — calls kernel, never owns vault
```

- **Eve / remote**: production bridge for communication + approve/deny + schedules — not a sandbox toy. See [docs/EVE-FIT.md](docs/EVE-FIT.md).
- **Game**: immersive world first; HUD secondary. See [docs/WORLD-FOUNDATION.md](docs/WORLD-FOUNDATION.md), [docs/PRODUCT-CHARTER.md](docs/PRODUCT-CHARTER.md).
- **Desktop later**: keep sim in Rust (`crates/peram-core`); thin hosts only.

---

## 4. Effort standard for agents

- Prefer **depth over breadth**: finish one surface to production quality before opening three stubs.
- Prefer **truth over theater**: if the game feels like software telemetry, fix the world — don’t document the dashboard.
- Prefer **operator life over codebase elegance** when they conflict: the day that improves is the acceptance test.
- When unsure, load: **higher-order-decision-architect**, **stellar-roadmap**, **impeccable**, **looper**, privacy docs.
- Do **not** answer with “quick prototype” language. Design for the version the operator will still use in 90 days.

---

## 5. Definition of done (session)

A session is not done when code compiles. It is done when:

1. The change is **testable** and tests (or smoke) pass on the real entry path.  
2. The operator can **feel or measure** the improvement (turn clarity, game feel, approval trust, privacy safety).  
3. Docs that agents will read next are **updated if law or map changed**.  
4. No new joke / hobby / prototype smell in the diff.

---

## 6. Canonical docs

| Doc | Role |
|-----|------|
| [docs/PRODUCT-CHARTER.md](docs/PRODUCT-CHARTER.md) | Why this exists; fun + impact + growth bar |
| [docs/PRIVACY.md](docs/PRIVACY.md) | Push boundary |
| [docs/EVE-FIT.md](docs/EVE-FIT.md) | Cloud bridge adopt/adapt/refuse |
| [docs/SWARM-DESIGN.md](docs/SWARM-DESIGN.md) | Day loop iron-peak |
| [docs/arch-design/coming-next.md](docs/arch-design/coming-next.md) | Roadmap SN cards |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Material architecture decisions |

---

**Footer plain rule:** Build as if this software will run the operator’s best years — because that is the point.
