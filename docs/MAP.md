# System map — capabilities, hosts, layers, IR

**Audience:** new readers, agents, and the operator who need a single orientation pass.  
**Not this doc:** how to play (→ [PLAYBOOK.md](PLAYBOOK.md)), race-car stack decision (→ [GAME-STACK.md](GAME-STACK.md)), WASM/desktop engine detail (→ [ENGINE.md](ENGINE.md)), product why (→ [PRODUCT-CHARTER.md](PRODUCT-CHARTER.md)).

**Last verified against shipped code:** 2026-07-13

---

## 0. One-sentence product

**ensembly** (Game of Peram) is a **local life swarm**: pure control-plane logic plans and prioritizes the day; thin hosts (CLI, browser game, static watch page) surface **one next body act**, **one next auth gate**, and a playable world — without pushing private life data.

```text
Kernel (local, pure, audited)     → day, privacy, realm, approvals IR, graph IR, looper
Host (game / CLI / desktop later) → input, paint, local store
Bridge (Eve optional, trajectory) → channels, remote approval UX, cron — calls kernel, never owns vault
```

---

## 1. Live capabilities (what runs today)

Only features with a **dogfood path** (command, URL, or unit-tested pure export) are listed as live. Roadmap (Eve channels, desktop host, multiplayer) is marked **trajectory**, not current.

### 1.1 Control plane (Node / pure modules under `src/`)

| Capability | Entry | What you get |
|------------|-------|--------------|
| **Day plan** | `npm run swarm:day` · `node bin/swarm.js day` | Bounded day cycle: projects, actions, schedule, balance, privacy partition → markdown plan under `private/state/plans/` |
| **Operator turn** | `npm run swarm:turn` · `node bin/swarm.js turn` | Next **physical** pickup + next **authorization** gate, full queues, human-readable markdown |
| **Turn status IR** | `node bin/swarm.js turn --json` | Machine-readable JSON: `next.physical`, `next.authorization`, queues, counts (agents/scripts) |
| **Approve / deny** | `node bin/swarm.js approve <id>` · `deny <id>` | HITL decision against durable wait snapshot (`private/state/wait-snapshot.json`) |
| **Claim / complete / release** | `claim` · `complete` · `release` | Physical pickup lifecycle (body work in progress → done / back to open) |
| **Graph export** | `npm run swarm:graph` · `node bin/swarm.js graph [--html]` | Serializable game graph (+ mermaid stdout, optional `public/watch/` HTML + JSON) |
| **Privacy classify** | pure: `src/privacy.js` · used by day/turn | Default-deny classifier; partition public vs private; never commit `private/` |
| **Realm split** | pure: `src/realm.js` | Physical pickups vs digital actions; HITL enrichment |
| **Prioritize / balance / loop** | pure: `prioritize.js`, `balance.js`, `loop.js`, `day.js` | Eisenhower + capacity balance + looper phases |
| **Persona boundary** | load via `ingest.js` | Full persona only under `private/persona/`; public projection under `public/persona/` |

### 1.2 Game of Peram (browser host)

| Capability | Entry | What you get |
|------------|-------|--------------|
| **Play session** | `npm run game` → `http://127.0.0.1:4173/game/` | Courtyard world, focus cycle, claim / approve / deny, growth board, **$SPN** ticker |
| **Session / focus SoT** | pure: `src/game/session.js`, `store.js` | Focus index, dispatch, undo/redo-shaped commands — **source of truth** |
| **Growth + $SPN** | pure: `src/game/growth.js`, `spn.js` | XP, quests, level titles, personal tape from real claim/HITL events |
| **Input map** | pure: `src/game/input.js`, `voice.js` | Keyboard, gamepad-shaped, voice vocabulary (Web Speech in host) |
| **World sim (hot path)** | WASM: `public/game/pkg/` from `crates/peram-core` | Layout tick, courtyard entities/props, draw buffer — **mirrors** focus, does not own it |
| **Smoke** | `npm run game:smoke` | Structural/smoke checks on game launch surface |

### 1.3 Watch (static web, not a second product)

| Capability | Entry | What you get |
|------------|-------|--------------|
| **Watch map** | `npm run swarm:graph` then open `public/watch/index.html` | Static next-action panel + mermaid/graph view; **consumer** of graph + turn-status export |
| **Life dashboard** | `npm run swarm:dashboard` → `public/watch/dashboard.html` | Overview · stats · rule-based insights · next body/auth · activity timeline (Dashboard IR v1) |

There is **no** separate multi-page web app. `public/watch/` is generated/served static artifacts from the CLI graph path. `legacy/` is the old webpack SPA — **not the product**.

### 1.4 Explicitly not live (trajectory)

| Item | Status |
|------|--------|
| Eve / Slack / phone remote HITL | Designed ([EVE-FIT.md](EVE-FIT.md)); not a shipped host in this repo |
| Desktop native (wgpu/Tauri) | Crate is host-agnostic; no desktop shell yet |
| Multiplayer voice rooms, WebNN scoring | Roadmap / refuse-until-ready |
| life-os vault merge | **Refused** ([LIFE-OS-BOUNDARY.md](LIFE-OS-BOUNDARY.md)) |

---

## 2. Host surfaces — CLI · game · watch · (Eve later)

Hosts are **thin**. They do not re-implement prioritization or privacy. Same kernel truth; different paint and I/O.

| Host | Role | Connects how | Not responsible for |
|------|------|--------------|---------------------|
| **CLI** (`bin/swarm.js`) | Operator + agent dogfood: day, turn, approve/deny, claim/complete, graph | Imports pure `src/*` (and filesystem under `private/` / `public/watch/`) | Rendering, WASM world, $SPN paint |
| **Game (browser)** | Immersive play surface | `scripts/serve-game.mjs` serves `public/game/` **and** `src/` as ESM; host loads session from `/src/game/*` and WASM from `/game/pkg/` | Day plan file I/O, durable wait snapshot (session is in-browser unless you wire fixtures) |
| **Watch (static web)** | Glanceable map of next act + graph | CLI **writes** `public/watch/{index.html,graph.json,turn-status.json}`; open in any browser / static server | Live simulation, claim/approve (read-only consumer) |
| **Eve (trajectory)** | Remote channels + approve buttons + cron | Would call kernel / CLI with **redacted** IR only | Vault, full persona, policy ownership |

### Connection diagram

```text
                    ┌──────────────────────────────────────┐
                    │  KERNEL (pure, local)                │
                    │  src/day · turn · privacy · realm    │
                    │  approvals · graph · loop · game/*   │
                    └───────────┬──────────────┬───────────┘
                                │              │
              import / Node CLI │              │ ESM import over HTTP
                                │              │ (serve-game maps /src → src/)
                    ┌───────────▼──┐    ┌──────▼──────────────────────────┐
                    │ bin/swarm.js │    │ public/game/ (thin host shell)  │
                    │ day turn …   │    │ main.js · engine.js · paint     │
                    └──────┬───────┘    │        │                        │
                           │           │        ▼ load                    │
                           │ graph     │  public/game/pkg/*.wasm          │
                           │ --html    │  (build of crates/peram-core)    │
                           ▼           └──────────────────────────────────┘
                    public/watch/
                    index.html · graph.json · turn-status.json
```

### Package scripts (shipped)

| Script | Purpose |
|--------|---------|
| `npm test` | Node pure-logic tests |
| `npm run test:rust` | `peram-core` crate tests |
| `npm run swarm:day` / `swarm:turn` / `swarm:graph` | CLI dogfood |
| `npm run game` / `game:serve` / `start` | Browser game server (port 4173) |
| `npm run game:smoke` | Game surface smoke |
| `npm run build:wasm` | Rebuild Rust → `public/game/pkg` (prebuilt pkg is checked in) |

---

## 3. Layer ownership (who owns what)

This is the common confusion zone: **`public/game` is not “the game logic.”** Much of what looks like “app code” there is host shell + **checked-in build artifacts**.

### 3.1 Ownership table

| Path | Owns | Does **not** own |
|------|------|------------------|
| **`src/`** (control plane) | Day plan, turn selection, privacy, realm, approvals snapshot IR, graph IR builder, looper, prioritization/balance | Canvas/DOM, WASM compile, remote channels |
| **`src/game/`** | Pure **session** kit: store, dispatch, focus, growth, $SPN math, input maps, voice parse, world *foundation* helpers (biome/sprite descriptors used by tests/host) | Actual browser paint, WASM binary, durable private state files |
| **`public/game/`** | Thin **browser host**: HTML/CSS, `main.js` boot, `engine.js` WASM loader, Canvas/WebGPU paint (`world-render.js`, `render.js`), `sample-graph.json` fixture | Policy, privacy classifier, day loop, durable approvals |
| **`public/game/pkg/`** | **Build output** of `crates/peram-core` (wasm-pack): `peram_core.js` + `peram_core_bg.wasm` + types — checked in so `npm run game` works without a Rust toolchain | Source of truth for rules (source is Rust); not the control plane |
| **`crates/peram-core`** | Deterministic **world/layout sim**: courtyard, entities, props, bind graph nodes, tick, draw buffer ABI, pack/layout math | Day/turn/privacy, HITL decisions, focus *authority* (mirrors only) |
| **`public/watch/`** | Static artifacts produced by graph export for glance UI | Live kernel, interactive claim/approve |
| **`bin/swarm.js`** | CLI wiring: argv, fixtures, read/write paths, stdout | Pure scoring logic (delegates to `src/`) |
| **`private/`** | Operator state, full persona, plans, wait snapshot — **gitignored / never push** | Public product surface |
| **`legacy/`** | Historical webpack React app | Anything current — do not extend as product |

### 3.2 Focus and “who is right?”

| Concern | Source of truth | Mirror |
|---------|-----------------|--------|
| Operator **focus** (which beacon) | JS session store (`src/game/session.js` via `createGameStore`) | WASM `world_set_focus` / draw buffer focus slot |
| **HITL / physical claim** (CLI durable) | Wait snapshot JSON + `src/approvals.js` | Game session events (in-browser until bridged) |
| **World positions / seek** | `peram-core` World | Canvas painter reads draw buffer only |
| **Privacy / day ranking** | `src/privacy.js`, `day.js`, `turn.js` | Never reimplemented in WASM or host paint |

Rule of thumb from AGENTS.md: **session/store owns operator intent; renderers and WASM mirror.**

### 3.3 Build path (Rust → pkg)

```bash
npm run build:wasm   # scripts/build-wasm.sh
# crates/peram-core  --wasm-pack-->  public/game/pkg/
```

You only rebuild when editing Rust. The checked-in `pkg/` is intentional (serve-without-build), not “hand-written game code.”

### 3.4 Why `public/game` imports `/src/game/...`

The static server maps:

- `/game/*` → `public/game/*` (host shell + pkg)
- `/src/*` → `src/*` (pure session modules)

So browser ESM loads **the same pure files** unit tests load. No duplicate session implementation in `public/`.

---

## 4. What “IR” means in this repo

### 4.0 Letters (what I and R stand for)

**IR = Intermediate Representation**

| Letter | Word | Plain meaning *here* |
|--------|------|----------------------|
| **I** | Intermediate | In the middle — not the raw private persona/vault, and not the final UI paint (CLI text, canvas, watch HTML). A stable “middle” form. |
| **R** | Representation | A structured encoding of the day’s truth (usually versioned JSON) that hosts and agents can read the same way. |

So: shared, machine-readable **middle shapes** between pure kernel logic and hosts/agents. **Not** LLVM compiler IR; not a compile pipeline — only the same I+R wording applied to life-swarm data contracts.

Think: **JSON (and sometimes markdown) contracts** that:

1. Are produced by pure functions or CLI flags  
2. Can be stored, piped, or opened without re-running ranking  
3. Stay free of private vault dumps when used at cloud/channel boundaries  

AGENTS.md’s “approvals IR, graph IR” means exactly these artifacts.

### 4.1 Catalog (shipped)

| IR name | Shape / home | Produced by | Consumed by |
|---------|--------------|-------------|-------------|
| **Turn status IR** | JSON `version: 1` with `next.physical`, `next.authorization`, queues, `counts` | `buildTurnStatus` · `node bin/swarm.js turn --json` | Agents, scripts, watch export (`turn-status.json`), future remote digests |
| **Wait snapshot IR** | JSON `version: 1` with `pending[]`, `physical[]`, `phase`, `status` | `src/approvals.js` serialize · default path `private/state/wait-snapshot.json` | `approve` / `deny` / `claim` / `complete` / `release` |
| **Graph IR** | JSON `version: 1` with `nodes[]`, `edges[]`, `meta` | `buildGameGraph` · `node bin/swarm.js graph` | Game sample load, watch `graph.json`, mermaid, WASM bind (ids/types/realms) |
| **Daily plan artifact** | Markdown under `private/state/plans/` | `day` command · `formatDailyPlan` | Human morning read; not the agent’s primary JSON contract |
| **Turn markdown** | Human surface from turn | `formatTurnMarkdown` / turn without `--json` | Operator eyes |

### 4.2 Examples

**Turn status IR** (truncated; from fixture):

```bash
node bin/swarm.js turn --fixture fixtures/state-sample.json --json --no-write
```

```json
{
  "version": 1,
  "date": "2026-07-13",
  "snapshotStatus": "idle_waiting",
  "phase": "HITL_WAIT",
  "next": {
    "physical": { "id": "evening-outdoor", "commands": { "claim": "node bin/swarm.js claim evening-outdoor" } },
    "authorization": { "id": "auth-apply-high-signal", "commands": { "approve": "…", "deny": "…" } }
  },
  "physical": [ /* queue */ ],
  "pending": [ /* HITL queue */ ],
  "counts": { "physical": 2, "pending": 1 }
}
```

**Wait snapshot IR** (durable HITL + physical claim state): see `fixtures/wait-snapshot.json` — `pending[]` with `status: "pending"`, optional `physical[]`, `phase`.

**Graph IR** (game / watch feed): see `public/game/sample-graph.json` or `public/watch/graph.json` — nodes typed `game` / `phase` / `action` / `physical` / `hitl`, edges with `kind`, optional layout positions.

**Graph → watch path:**

```bash
npm run swarm:graph
# writes public/watch/index.html, graph.json, turn-status.json
```

### 4.3 Not IR

- Canvas draw buffers (engine ABI bytes — see [ENGINE.md](ENGINE.md))  
- $SPN series points inside a live session (session state, not a durable cross-host contract)  
- Full persona JSON under `private/persona/` (private state; never a public IR)  

---

## 5. Module cheat sheet (`src/` exports)

Public Node API surface: `src/index.js`.

| Module | Role |
|--------|------|
| `privacy.js` | Classify / partition / path patterns |
| `prioritize.js` | Eisenhower + balanced score |
| `balance.js` | Area counts, schedule proposals |
| `loop.js` | Looper phases, budgets, HITL wait |
| `day.js` | Day plan assembly |
| `realm.js` | Physical vs digital |
| `approvals.js` | Wait snapshot IR mutate/list |
| `turn.js` | Operator turn + status IR + graph export runner |
| `graph.js` | Graph IR + mermaid + watch HTML |
| `ingest.js` | Persona / local state load (paths) |
| `game/*` | Session store, growth, $SPN, input, voice, focus-sync |
| `sovereignty-gist.js` | Public thinking-helper constants (not day kernel) |
| `activity/*` | Activity/log IR + storage port: memory (tests) · SQLite SoT under `data/local/activity.sqlite` |
| `dashboard.js` | Life progress Dashboard IR + insights + HTML/markdown projectors |

Rust entry: `crates/peram-core/src/{lib,world,layout}.rs` → WASM exports `world_*`, `layout_tick`, `pack_grid`, `engine_version`.

**Storage foundation:** local **SQLite** = durable SoT for activities/logs; **in-memory** = same port for tests; IndexedDB/Postgres/Mongo/graph DB are not day-0 SoT. Decision: [DECISIONS.md](DECISIONS.md#activity--log-storage-foundation-2026-07-13) · privacy path: [PRIVACY.md](PRIVACY.md).

---

## 6. Where to go next

| Need | Doc |
|------|-----|
| Play and steer the day | [PLAYBOOK.md](PLAYBOOK.md) |
| Why fun + impact bar | [PRODUCT-CHARTER.md](PRODUCT-CHARTER.md) |
| WASM/desktop engine split | [ENGINE.md](ENGINE.md) |
| Why Rust+WebGPU race-car | [GAME-STACK.md](GAME-STACK.md) |
| World biome / sprites extend | [WORLD-FOUNDATION.md](WORLD-FOUNDATION.md) |
| Privacy push boundary | [PRIVACY.md](PRIVACY.md) |
| Remote Eve adopt/refuse | [EVE-FIT.md](EVE-FIT.md) |
| Roadmap SN cards | [arch-design/coming-next.md](arch-design/coming-next.md) |
| Material decisions log | [DECISIONS.md](DECISIONS.md) |

**Footer:** If a surface is not in §1, treat it as trajectory until it has a command/URL and tests on the shipped path.
