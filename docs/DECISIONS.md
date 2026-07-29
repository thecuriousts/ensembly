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

## Life-mirror play path + HITL digital-flow spine (2026-07-15)

**Verdict:** Game host must load a **life-derived** graph (operator turn IR beacons: next physical + next auth), not sample-only theater. **life-os** portfolio cards project into day candidates via pure IR (`src/lifeos/`). First digital flow class is **bill_pay / Bank**: activate → HITL pending → approve → **dry-run** execute hook (live bank mutate refused until explicit greenlight; never unattended).

| Adopt | Refuse |
|-------|--------|
| `buildPlayableGraphFromTurn` + beacons from real turn | Sample fixture as the only play surface |
| life-os `Projects/*/README` `next_action` projection | Merging vault into ensembly git |
| Digital-flow IR + injected executeHook (dry_run default) | Fake “Bank paid” UI without IR/HITL |
| `public/game/life-graph.json` local export (gitignored) | Unattended finance_transfer / bank API |

**Ship:** `src/play.js` · `src/digital-flow.js` · `src/lifeos/portfolio.js` · `node bin/swarm.js graph` · `digital-flow` · host `loadGraph` order life → watch → sample.  
**Near-term confidence:** 86%.

## Rust life-console restart (2026-07-15)

**Verdict:** Node ESM control plane is **legacy dogfood** (bugfix-only). **Iron-peak** moves to **Rust `peram-kernel`**: typed day/turn/privacy/HITL/digital-flow + **tiered durable data** + native console trajectory. Browser/WASM remains optional thin host; sample-graph is never product truth.

### Data plane (T0–T4)

| Tier | Store | Role |
|------|-------|------|
| **T0** | OS keyring | Unlock wrappers, connector tokens — never git, never plaintext SQLite |
| **T1** | SQLite (WAL, single-writer) | Wait snapshot, audit, flows, FocusPlan cache — crash-safe ops ledger |
| **T2** | PQ sealed vault (peram-vault law: ML-KEM-768 + AES-256-GCM) | High-sens blobs, recovery material — ciphertext SoT |
| **T3** | Content-addressed packs | Backups / media on operator volume |
| **T4** | Remote redacted views | Eve/Grok/share only after classifier |

### Threat model (design floor)

Stolen **laptop + network** access: disk theft without unlock must not yield vault plaintext; cloud sees redacted IR only; external mutate always HITL. Honest limit: unlocked session + malware-as-user cannot be fully defeated in pure software — minimize blast radius + audit + recovery.

### Polyglot / P2P / web3

| Welcome when better | Refuse |
|---------------------|--------|
| Rust kernel; premflow C capture SoT; life-os markdown portfolio; wealth-core math | Multi-language HITL SoTs |
| Encrypted backup packs; optional Shamir recovery shards; P2P sealed off-site **after** local backup green | On-chain life plaintext; multi-master CRDT for turn day-0; web3 as product personality |

### Host trajectory

1. **Now:** `peram` Rust CLI + kernel tests (parity with fixtures).  
2. **Next:** `peram-console` native (Hyprland) — one window, evolving world from `rank_now`.  
3. **Later:** WASM thin client + Eve redacted approve bridge.

| Adopt | Refuse |
|-------|--------|
| `crates/peram-kernel` as control SoT | Expanding product features in Node `src/` |
| Backup/restore as tested product paths | “Backup = hope” |
| Vault bridge to peram-vault law | Reinventing crypto in the game loop |
| FocusPlan = rank_now(ContextFrame) | Sample-graph theater as daily surface |

**Ship path:** `crates/peram-kernel` · `cargo test -p peram-kernel` · `cargo run -p peram-kernel -- turn` · `cargo run -p peram-kernel -- runtime load --fixture fixtures/issue-1-runtime.json`  
**Near-term confidence:** 75%. **Thrive bet:** 85%.

---

## Issue #1 HITL/HOOTL runtime core (2026-07-24)

**Verdict:** Game surface excluded from the Issue #1 core. Control plane owns **life-state S**, directed **DepGraph G**, **CP + P** (PERT expected + basic Monte Carlo), typed **MsgBus**, declarative **triggers**, and basic **HOOTL agents** that claim only through G/CP. Physical beacons + auth gates remain HITL (“wait only for permission”). Formal law: [`arch-design/formal_problem_definition.AppGenMathPhyLang.md`](../arch-design/formal_problem_definition.AppGenMathPhyLang.md).

| Adopt | Refuse |
|-------|--------|
| Runtime SoT for S+G+CP in `peram-kernel` | Unity/game layer as prerequisite for prioritization |
| Agents claim via CP only | Free-form agent-to-agent chatter as coordination |
| CLI `runtime *` + durable `life_state` in T1 SQLite | Eisenhower-only prioritization as final SoT |
| Explainable CP reasons on FocusPlan | Opaque priority scores without graph path |

**Ship path:** `cargo run -p peram-kernel -- runtime load|status|tick|approve|deny|claim|complete` · `npm run peram -- runtime …` · `fixtures/issue-1-runtime.json` · `cargo test -p peram-kernel`  
**Approve id:** action id (e.g. `pay-rent`); optional `auth-` prefix is stripped — snapshot may still key `auth-*` internally.  
**Top-level gates:** `peram approve|deny|claim|complete` require durable `life_state` (after `runtime load`) and always go through Runtime + `save_runtime_pair`. No snapshot-only legacy path — refuse loud if life_state missing.  
**Claim-via-CP:** `next_hootl_digital` returns only open digital HOOTL **on the CP path** (no off-path fallback). Direct `AgentWorker::claim` also requires CP path membership.  
**Auth/Physical off-CP surface (intentional asymmetry):** `next_auth_gate` / `next_physical_beacon` are **CP-first**, then fall back to earliest open Auth/Physical by id so HITL wait-state never hides a real gate. HOOTL agents remain Claim-via-CP only; Auth/Physical may surface off-CP. Do not make Auth/Physical CP-only without a product law change.  
**Tick honesty:** one HOOTL agent step per tick — claim **or** complete owned claim, never silent same-tick Done-as-exec.  
**Metrics:** honest `hootl_completed` / `hitl_surfaces` (edge-enter HitlWait) / `agent_failures` — no multi-axis C/E/E theater. MC samples default **0** (PERT σ always); set `mc_samples` when load needs Monte Carlo.  
**`--json` caveat:** stdout is JSON then a trailing `RUNTIME_OK …` line.  
**Remaining (not blocking this slice):** multi-agent conflict resolution at scale, adaptive/RL local policies, continuous outcome telemetry dashboards, Eve bridge for remote HITL.

---

## Episodic memory layer — peram-memory fused from IntelliArch (2026-07-29)

**Verdict:** The IntelliArch prototype's local-first CRDT memory + coherence engine is adopted as a new crate `crates/peram-memory`, bridged into the kernel via `memory_sink`. The kernel runtime **records** what happened (applied bus messages, tick reports, graph loads) into a durable, mergeable episodic document; explicit `runtime reflect` runs coherence scoring, skill synthesis, and goal proposals over that trajectory. Origin: IntelliArch `tries/` (agent_memory + coherence_engine), ported synchronous to kernel discipline.

| Adopt | Refuse |
|-------|--------|
| Memory as **aux, never SoT**: kernel S/G/CP + approvals stay the only control truth | Memory influencing gates, priorities, or CP |
| Record **applied** messages only (append after success) | Logging attempts/intents as facts |
| Durable engine state (pattern counts, coherence history **inside** the CRDT doc) | Process-local learning lost per CLI run |
| Explicit `runtime reflect` (operator/cron driven) | Implicit reflection inside control ticks |
| Memory save failure → loud stderr warn, control op unaffected | Aux failure failing a committed control op |
| Explicit `--memory <path>` open failure → fatal (operator asked) | Silent degrade on explicit intent |
| `data/local/peram-memory.json` (gitignored, T1 privacy boundary) | Memory under `public/` or commit-eligible paths |

**Ship path:** `cargo test -p peram-memory` · `cargo run -p peram-kernel -- runtime load|tick|…` (records) · `cargo run -p peram-kernel -- runtime reflect [--json]` · flags `--memory <path>` / `--no-memory`  
**Merge law:** entry-level CRDT merge is idempotent and commutative; re-merging held state is a true no-op (stable hash). Concurrent CLI writers reconcile via `sync_and_save` (load-merge-persist, atomic tmp+rename).  
**Reflection contract:** skipped loudly below 5 trajectory entries; inference providers may augment coherence scoring and summaries **behind `InferenceProvider` trait** — deterministic Jaccard stays the default and the test oracle; unavailable or slow backends warn and fall back, never block reflect.  
**Remaining (trajectory, not live):** live `grok -p` enrich (P2b), full Grok ACP client loop (P2c/P3b), secondary adapters (opencode, pi, Ollama feature), peer-to-peer replica sync, memory-informed coach lines (read-only, gated). Trait + deterministic + read-only `peram-mcp` ship in SN-8 P2a/P4a.

---

## Multi-provider inference and delegation (2026-07-29)

**Verdict:** After episodic memory landed (SN-7), the next slices must treat **toolchains as abundant and swappable** — not standardize on Ollama. Local Ollama is slow on the operator machine; it must never be a hard dependency, default path, or CI requirement.

| Adopt | Refuse |
|-------|--------|
| `InferenceProvider` trait in `peram-agents`; **deterministic Jaccard** as default + test oracle | Ollama/`reqwest` in default `peram-memory` / `peram-agents` build |
| Optional adapter crates or feature flags per backend (`judge-ollama`, `judge-grok-mcp`, …) | One-vendor inference lock-in |
| Runtime selection: `PERAM_INFERENCE=deterministic\|ollama\|grok-mcp\|opencode-acp\|…` | Silent hang when a daemon is down |
| Unavailable/slow provider → stderr warn + deterministic fallback | Reflect or tick failure because a model host is offline |
| `DelegationBackend` trait for Human-Out-Of-The-Loop hands; **Grok Agent Client Protocol as first** adapter; opencode secondary | opencode as only delegation path |
| Grok Model Context Protocol / opencode Model Context Protocol as **clients** calling external hosts | Embedding cloud keys in repo |
| `peram-mcp` exposes Model Context Protocol server (`memory_*`, `swarm_banner`; `kernel_status` deferred); consumes external Model Context Protocol/Agent Client Protocol as providers | Shadow Model Context Protocol servers without operator approval |
| Tests: deterministic only; adapter tests behind `#[ignore]` or feature | CI requiring Ollama, Grok, or opencode running |

**Ship order:** P2a trait + deterministic → P3a recall → P4a MCP export (register into Grok) → **P2b Grok MCP + P2c/P3b Grok ACP** → P2d secondary adapters (opencode, pi, Ollama feature) → P5 peer-to-peer decision.

**Grok-first rationale (2026-07-29):** Official xAI docs — MCP via [`grok mcp add`](https://docs.x.ai/build/features/mcp-servers) / `.grok/config.toml`; ACP via [`grok agent stdio`](https://docs.x.ai/build/cli/headless-scripting#acp); headless inference via `grok -p --output-format json`. Export `peram-mcp`; dogfood Grok before Ollama. Never commit `XAI_API_KEY` or `~/.grok/mcp_credentials.json`.