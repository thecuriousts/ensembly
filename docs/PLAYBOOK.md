# Operator playbook — play, steer, produce (laptop + remote)

**Status:** Binding dogfood guide for the human pair and the digital clone  
**Product:** ensembly · **Game of Peram**  
**Last updated:** 2026-07-13  

This is how you **use** ensembly to maximize real-day productivity — not how the modules are named.  
Companion law: [MAP.md](MAP.md) (capabilities · hosts · layers · IR) · [PRODUCT-CHARTER.md](PRODUCT-CHARTER.md) · [LIFE-OS-BOUNDARY.md](LIFE-OS-BOUNDARY.md) · [PREMFLOW-FIT.md](PREMFLOW-FIT.md) (notes/tasks/pomo vs turn) · [EVE-FIT.md](EVE-FIT.md) · [CLONE-COPILOT.md](CLONE-COPILOT.md)

---

## 0. What you are optimizing

| Scarce resource | What ensembly does | What only you do |
|-----------------|--------------------|------------------|
| **Attention** | Surfaces **one next body act** + **one next auth** | Choose and execute |
| **Capacity** (family, health) | Day plan + schedule respect non-negotiables | Show up in the body-world |
| **Risk** | HITL gates; no unattended bank/email | Approve / deny |
| **Momentum** | XP, quests, **$SPN** tape from real progress | Keep the streak honest |

**North star:** Digital thrash is automated. You join as a **pair** for physical work and judgment — from the laptop **or** a remote channel.

---

## 1. Surfaces (pick by context)

| Surface | When | Command / URL |
|---------|------|----------------|
| **Game of Peram** | Feel the day, claim beacons, clear gates, watch **$SPN** | `npm run game` → `http://127.0.0.1:4173/game/` |
| **Operator turn (CLI)** | Fast “what now?” on any machine with the repo | `npm run swarm:turn` or `node bin/swarm.js turn --stdout` |
| **Status IR (JSON)** | Agents / scripts / future phone bots | `node bin/swarm.js turn --json` |
| **Watch map** | See next act + diagram without game loop | `npm run swarm:graph` → open `public/watch/index.html` |
| **Life dashboard** | Stats, insights, overview of progress | `npm run swarm:dashboard` → open `public/watch/dashboard.html` |
| **Shared notes/tasks/pomo** | One inbox with premflow + vault (`~/.premflow`) | `node bin/swarm.js flow …` · `npm run flow:link` · [PREMFLOW-FIT.md](PREMFLOW-FIT.md) |
| **Day plan** | Morning structure: projects, schedule, balance | `npm run swarm:day` |
| **Remote channels** (Eve trajectory) | Slack/web/phone: digest + approve/deny away from desk | See [§5 Remote](#5-remote--channels-not-just-the-laptop) · [EVE-FIT.md](EVE-FIT.md) |

All of these share one **kernel truth**: day plan, realm split, wait snapshot, growth events. Hosts are thin.

---

## 2. How to play Game of Peram (laptop)

### 2.1 Launch

```bash
cd ~/Work/personal/ensembly
npm test          # optional confidence
npm run game      # trailing slash required: /game/
```

Default loadout: **clean courtyard** — world first, chrome on demand.

### 2.2 Controls

| Input | Action |
|-------|--------|
| **Tab** / `j` `k` / arrows | Cycle focus (beacons) |
| **Enter** / Space / **C** | **Claim** focused beacon → XP |
| **A** / **Y** | **Approve** open HITL gate → XP |
| **D** / **N** | **Deny** open HITL gate → XP (and **$SPN** sells) |
| **B** / **Q** | Growth **board** (quests, balance axes, coach) |
| **I** | Status **bars** (level, XP, streak, **$SPN** inline) |
| **M** | Command menu (type / voice) |
| **?** / **H** | Help codex |
| **Esc** | Clear chrome |
| **V** | Voice start/stop |
| Gamepad | D-pad focus · face approve/deny |

### 2.3 Progression (what “winning” means)

| Axis | Counts as progress |
|------|--------------------|
| **Body** | Physical pickups (errands, outdoor, body-world) |
| **Presence** | Family / health / schedule beacons |
| **Craft** | Deep work & ship beacons |
| **Gates** | HITL approve / deny |

- Claim → XP → streak combos → levels (**Ember → Horizon**).  
- Coach steers you **off pure digital grind** toward body/presence when empty.  
- Engine: `src/game/growth.js`.

### 2.4 $SPN (your personal tape)

Top-right **stock-style ticker** **`$SPN`**:

| Real outcome | Tape |
|--------------|------|
| Claim body / presence / craft | **Up** |
| Approve gate | **Up** |
| Deny gate | **Down** (miss priced) |
| Craft-only with no body/presence | **Imbalance drag** |

**Use:** glance at green/red sparkline after a work block. If craft runs and **$SPN** drags, the day is out of balance — claim a body/presence beacon. Pure math: `src/game/spn.js`.

### 2.5 Session hygiene (game)

1. Open game after morning turn (or load sample graph when offline).  
2. Clear **open gates** first if they block digital path (A/D).  
3. Claim **physical/presence** before endless craft.  
4. Toggle board (**B**) only when you need quest list; stay in world when possible.  
5. Esc when chrome steals focus from the day.

---

## 3. Steer the day with CLI (productivity loop)

The CLI is the **auditable control plane** — same decisions as the game, scriptable on SSH, CI, or a second machine.

### 3.1 Morning (or any cold start)

```bash
# Full day structure (optional write under private/state when not --no-write)
npm run swarm:day

# What YOU must do next (markdown)
node bin/swarm.js turn --stdout

# What agents/bots must do next (JSON)
node bin/swarm.js turn --json --no-write
```

Read:

1. **Next body** — one physical act (claim/complete commands included).  
2. **Next auth** — one gate (approve/deny commands).  
3. Full queues below for context — **do not equal-weight** them; lead with primaries.

### 3.2 Execute body work

```bash
node bin/swarm.js claim <physical-id>     # in progress
# … do the real-world work …
node bin/swarm.js complete <physical-id> # leaves open queue
```

Durable snapshot: `private/state/wait-snapshot.json` (local only).  
Watch HTML refreshes on durable write so the map matches truth.

### 3.3 Clear digital risk

```bash
node bin/swarm.js approve <auth-id>
node bin/swarm.js deny <auth-id>
```

Never invent unattended bank/email. Gates exist so **you** decide.

### 3.4 Map + agent handoff

```bash
npm run swarm:graph    # public/watch/ — next-action panel + Mermaid + $SPN-era day map
```

Clone/agents: prefer `turn --json` over scraping markdown.

### 3.5 Fixture dry-run (safe demo)

```bash
node bin/swarm.js turn --fixture fixtures/state-sample.json --stdout --no-write
node bin/swarm.js turn --fixture fixtures/state-sample.json --json --no-write
```

---

## 4. Productivity recipes (not features for features’ sake)

### Recipe A — “I have 25 minutes”

1. `turn --stdout` → take **next body** if in a physical window, else next **auth** if blocking.  
2. Complete or approve/deny.  
3. Glance **$SPN** in game or re-run `turn --json` for clone.

### Recipe B — “Deep work block”

1. Confirm schedule block on day plan.  
2. Clear pending HITL so agents aren’t stuck.  
3. Claim craft beacons **after** a presence/body claim if coach warns.  
4. End block: `complete` any claimed body; re-open turn.

### Recipe C — “Clone does digital; I pair later”

1. Clone reads `turn --json` / `private/state/turn-status.json`.  
2. Clone advances digital work **behind** open gates only if allowed.  
3. You receive **one** auth + **one** body when free (laptop game or channel).  
4. Copilot PRs under [CLONE-COPILOT.md](CLONE-COPILOT.md) — you oversee proposals.

### Recipe D — “Energy is low”

1. Prefer **presence/body** primaries; ignore craft thrash.  
2. Deny low-value digital pressure.  
3. Watch **$SPN**: flat/down with craft-only = stop shipping cosplay.

---

## 5. Remote — channels, not just the laptop

### 5.1 What “remote” means here

You are **not** tied to the game canvas. The same kernel decisions travel:

| Remote need | Today (shipped) | Production bridge (trajectory) |
|-------------|-----------------|--------------------------------|
| See next act | `turn --stdout` / `--json` over SSH; open `public/watch/` on any device that can reach files | Channel digest (Slack/web): next body + next auth |
| Approve / deny away from desk | CLI on phone SSH / laptop | **Eve** (or equivalent) tool approval buttons → same snapshot IR |
| Morning nudge | Cron + CLI on a box you own | Eve schedules → redacted turn post |
| Claim/complete body | CLI | Channel commands mapped to `claim` / `complete` |
| Play the world | Browser on any machine with `npm run game` | Stays **local/browser** — Eve is not a game engine |

### 5.2 Eve / channel contract (when built)

From [EVE-FIT.md](EVE-FIT.md):

| **Adopt** | Channels for digests + remote HITL + cron |
| **Adapt** | Tools call pure ensembly modules / CLI |
| **Refuse** | Full persona/vault on the cloud; Eve as kernel rewrite; unattended finance/email |

Message shape for a channel (conceptual — implement against real CLI):

```text
Next body: Grocery errand · grocery-errand
  claim:  node bin/swarm.js claim grocery-errand
  done:   node bin/swarm.js complete grocery-errand

Next auth: Apply packet · auth-apply-high-signal
  [Approve] [Deny]
```

Privacy: **redact** titles if needed; never ship `private/persona` or vault medical/finance to a channel by default.

### 5.3 How to steer remotely *today* (no Eve yet)

1. **Phone + SSH / Termux / laptop left on:** run `turn --json` and `approve` / `complete`.  
2. **Shared watch file:** regenerate graph; open HTML when on home LAN.  
3. **Agent clone:** give it `turn --json` output; it proposes work; you approve PRs and HITL.  
4. **life-os card:** after a session, update `Projects/ensembly/sessions/YYYY-MM-DD.md` so portfolio memory matches runtime.

### 5.4 What never goes remote unattended

- Bank / email **mutate** without explicit human auth  
- Full private persona upload  
- Silent merge to main of high-risk changes without oversee (copilot phase 1)

---

## 6. One-day operating cadence

```text
Morning
  day plan (optional) → turn --stdout → pick primary body/auth
  optional: npm run game · glance $SPN open

Work blocks
  claim body when schedule says so
  approve/deny gates so digital path can move
  craft beacons only after capacity is honest

Away from desk
  turn --json / channel digest
  approve/deny on phone path when built
  clone continues digital under gates

Evening
  complete open claims
  turn once more — queues should shrink
  $SPN green with multi-axis balance = good day signal
  life-os session note if the clone or you shipped work
```

---

## 7. Agent / clone steering (for coding sessions)

Agents reading this repo:

1. Prefer **operator life impact** over new stubs ([AGENTS.md](../AGENTS.md)).  
2. Use **turn status IR** and durable snapshot — do not invent ranking.  
3. Portfolio code work: [CLONE-COPILOT.md](CLONE-COPILOT.md) — proposal → PR.  
4. life-os is **vault memory**; ensembly is **runtime** ([LIFE-OS-BOUNDARY.md](LIFE-OS-BOUNDARY.md)).  
5. After dogfood: update tests on the **shipped** path; keep privacy default-deny.

---

## 8. Quick command card

```bash
# Play
npm run game

# Next act
node bin/swarm.js turn --stdout
node bin/swarm.js turn --json

# Body
node bin/swarm.js claim <id>
node bin/swarm.js complete <id>

# Gates
node bin/swarm.js approve <id>
node bin/swarm.js deny <id>

# Map
npm run swarm:graph

# Day
npm run swarm:day
```

---

## 9. Related docs

| Doc | Role |
|-----|------|
| [README.md](../README.md) | Drop-in + short play table |
| [FOUNDATION-CRITIQUE.md](FOUNDATION-CRITIQUE.md) | Why next-action + status IR exist |
| [SWARM-DESIGN.md](SWARM-DESIGN.md) | Day loop iron-peak |
| [PRIVACY.md](PRIVACY.md) | What never ships |
| [GAME-STACK.md](GAME-STACK.md) · [ENGINE.md](ENGINE.md) | Technical game stack |
| [arch-design/coming-next.md](arch-design/coming-next.md) | Roadmap including remote SN cards |

---

**Footer:** Maximize productivity by clearing **one body act** and **one gate** at a time — from the courtyard, the CLI, or a channel — and let **$SPN** tell you if the day is balanced.
