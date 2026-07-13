# Material decisions (higher-order-decision-architect)

## Executive verdict

Ship a **greenfield Node ESM swarm control plane** with pure prioritization/balance/privacy units and a looper-shaped day loop; isolate legacy UI under `legacy/`; keep full persona local-only.

## Critical area

Privacy vs usefulness: the swarm must use rich private context without ever making private data commit-eligible.

## First principles

- Control plane ≠ chatty agent transcript; artifact must be auditable.
- Capacity (family + health) is the scarce resource; scheduling is the product.
- Default-deny is cheaper than scrubbing after a leak.

## Consequence chain (summary)

| Order | Effect | P | Impact |
|-------|--------|---|--------|
| 1 | Runnable daily plan from persona | high | H |
| 2 | Agents reuse plan for weekend delegation | med | H |
| 3 | Accidental private commit if ignore fails | low | H → mitigated by gitignore + classifier + docs |
| 3+ | Harmonious multi-area life OS mesh | med | H (2036 thrive) |

## Inversion / pre-mortem

- Fail: “upgrade legacy webpack app” thrash → **refused**.
- Fail: unattended email/bank → **non-goal**; HITL only.
- Fail: public persona contains DNA/debt/family medical → scrubbed projection only.

## Thrive ascent (2036)

North star: operator’s responsibilities stay in **harmonious balance** via a durable local swarm kernel; bridges (life-os, collab-finder, Grok skills) evolve.

| Refuse | Build toward |
|--------|----------------|
| Legacy SPA as product | Testable day-loop control plane |
| Pushing private llm/data | Public code + projection + events |
| Infinite ReAct autonomy | Looper budgets + HITL |

**Iron-peak:** `classifyItem` + `runDayLoop` + daily plan schema.

**Near-term confidence:** 80%. **Thrive bet:** 70%.

## Immediate actions (done in this goal)

1. Global looper for Grok  
2. Persona full + public projection  
3. Swarm entry + tests + privacy ignore  

## Eve host decision (2026-07-13)

**Verdict:** Treat [Vercel Eve](https://vercel.com/eve) as the **production remote bridge** (channels + approval + schedules), not a kernel rewrite and **not a disposable prototype**.

| Decision | Rationale |
|----------|-----------|
| **Adopt** Eve for channels + tool approval + cron schedules | Production remote operator surface: communication, approve/deny, cadence ([EVE-FIT.md](EVE-FIT.md)) |
| **Adapt** tools that call pure ensembly modules / CLI | Kernel stays tested and offline-capable; Eve owns durable UX |
| **Refuse** full persona / vault on Vercel Eve by default | Privacy default-deny + sovereignty gist |
| **Refuse** rewriting day/privacy/realm into Eve-only prompts | Iron-peak stays pure Node ESM |
| **Refuse** game sim on Eve | WASM world is local/desktop host |
| **Refuse** “prototype theater” | When Eve ships, it ships production-grade for real life use |

**Order:** CLI + game must already be daily-grade; Eve bridge is the next production host for remote life ops — built once, hardened, privacy-reviewed. Confidence: 85%.

## Product seriousness (2026-07-13)

**Verdict:** ensembly is **production life infrastructure** for operator growth — not a hobby demo.

Binding: [AGENTS.md](../AGENTS.md) · [PRODUCT-CHARTER.md](PRODUCT-CHARTER.md)

| Refuse | Build toward |
|--------|----------------|
| Joke UX, fake polish, throwaway stubs | Fun Game of Peram that tells the truth |
| “MVP then rewrite” | Day-0 production paths + tests + privacy |
| Feature tourism | Impact on real days and capacity |

## Operator actionability (2026-07-13)

**Verdict:** Kernel IR was solid; **last-mile act** (one next physical, one next auth, machine status, body claim/complete, watch panel) was the gap. Critique + shipped fixes: [FOUNDATION-CRITIQUE.md](FOUNDATION-CRITIQUE.md).

## life-os vs ensembly boundary (2026-07-13)

**Verdict:** **`~/life-os`** is the **clustered Projects/Areas vault** (started-and-organized portfolio memory). **ensembly** is the **digital clone / continuous friction swarm** (data, hooks/connectors trajectory, human as intermittent **pair** for physical + HITL). **Refuse** merging vault into product git; **refuse** treating life-os as runtime clone. Continuous connectors = ensembly trajectory, not vault duty. Experiment/archive satellites freely.

Full law: [LIFE-OS-BOUNDARY.md](LIFE-OS-BOUNDARY.md) · Binding entry: [../AGENTS.md](../AGENTS.md)

## Clone as copilot (2026-07-13)

**Verdict:** Digital clone is **free to copilot life-os portfolio projects**. Maintains **internal** ideas/schedule (`private/clone/`); **phase 1** = human oversees proposals, then clone **opens PRs**. Human remains merge authority, capacity veto, physical + HITL. Not unattended external mutate.

Law: [CLONE-COPILOT.md](CLONE-COPILOT.md)

## Activity / log storage foundation (2026-07-13)

**Executive verdict:** One local writer, one durable source of truth. **SQLite file** under gitignored `data/local/` is the durable SoT for activities and logs; **in-memory** is the same port for tests and hot session; **refuse** multi-master sync and cloud/DB-as-vault.

### Options considered

| Option | Role for activities/logs | Day-0 verdict |
|--------|--------------------------|---------------|
| **SQLite** (local file, `node:sqlite`) | Durable audit stream, zero server, WAL, survives reopen | **Adopt as SoT** |
| **In-memory** | Tests, ephemeral session buffer, same port API | **Adopt as adapter** |
| **Postgres** | Multi-user server DB | **Refuse as SoT** — ops weight, always-on, overkill for one operator |
| **MongoDB** | Document cloud/local server | **Refuse as SoT** — second runtime, sync temptation, privacy surface |
| **Graph DB** (Neo4j etc.) | Relationship queries | **Refuse as SoT** — day/graph relationships stay **Graph IR JSON**; no second query engine |
| **Redis / etc.** | Cache/queue | **Refuse as SoT** — ephemeral by design |
| **Browser IndexedDB** | Client cache for game host | **Mirror-only later** — never authoritative for private vault |
| **Multi-writer CRDT sync** | Multi-device conflict merge | **Refuse day-0** — latency + headache; scale later via **export/replica** of event stream, not dual live writers |

### First principles

- Privacy default-deny: durable private activity must live under **gitignored** local paths only.
- Kernel stays pure: control plane talks to a **storage port** (`append` / `list` / `close`), not vendor clients.
- Append-oriented IR compounds into surplus (audit → review → better steering) without blocking the main loop.
- One SoT kills sync drama: hosts may **export redacted IR**; they do not own a second private authority.

### Consequence chain

| Order | Effect | P | Impact |
|-------|--------|---|--------|
| 1 | Activities/logs survive process restart | high | H |
| 2 | Tests run without server DB credentials | high | H |
| 3 | Accidental dual-write to cloud vault | low if refused | H (mitigated by refuse + gitignore) |
| 3+ | Export/replica path for remote digests / desktop | med | H (2036 thrive) |

### Inversion / pre-mortem

- Fail: Mongo + SQLite dual SoT “for flexibility” → desync, privacy leaks → **refused**.
- Fail: IndexedDB as operator truth in browser-only mode → unreadable from CLI/agents → **refused**.
- Fail: Blocking the turn loop on DB errors → degrade with best-effort later; foundation keeps port narrow so callers can wrap.

### Thrive ascent (2036)

| Refuse | Build toward |
|--------|----------------|
| Multi-master activity vaults | Single local SQLite + versioned activity IR |
| Graph DB for day map | Graph IR JSON + optional export |
| Eve/cloud as vault | Redacted turn/activity digests only |
| Schema thrash every host | Stable `activity_events` + schema_migrations |

**Iron-peak:** versioned **activity/log IR** + port (`append`/`list`) + local SQLite file.  
**Ship path:** `src/activity/*` · default file `data/local/activity.sqlite` · CLI `activity` / `log`.  
**Near-term confidence:** 88%. **Thrive bet:** 80%.

## Premflow between life-os and ensembly (2026-07-13)

**Verdict:** **One filesystem SoT** for notes/tasks/journal/pomo: `~/.premflow/`. Views: premflow CLI, ensembly `flow` wrapper, life-os `Projects/premflow/capture` symlink. Day/HITL stays ensembly; portfolio frontmatter stays life-os. Refuse multi-master DBs and dual-live `todo.txt` copies.

Full law: [PREMFLOW-FIT.md](PREMFLOW-FIT.md). Ship: `src/premflow/*`, `node bin/swarm.js flow`, `npm run flow:link`.

## Life progress dashboard (2026-07-13)

**Verdict:** **Dashboard IR v1** (pure stats + rule-based insights + overview) projected to **static watch-family HTML** under `public/watch/dashboard.html`. Not a SPA, not the game center (world > chrome).

| Adopt | Refuse |
|-------|--------|
| Pure `buildDashboard` + CLI `dashboard` | React/Vite dashboard product |
| Insights with explicit `evidence` + steer | LLM pep-talk theater without data |
| Activity SQLite + turn/snapshot as sources | Dual SoT / cloud vault analytics |
| collab-finder **read-only later** | Writing collab-finder.db from ensembly |

**Ship:** `src/dashboard.js` · `npm run swarm:dashboard` · `public/watch/dashboard.{html,json}`  
**Near-term confidence:** 85%.