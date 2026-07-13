# Premflow between life-os and ensembly — one tracking model

**Status:** Binding operator law (terminal-first life)  
**Date:** 2026-07-13  
**Lenses:** fusion-sage · looper · higher-order-decision-architect  
**Audience:** Operator living in TUIs (Grok Build, Cursor, shell); agents that might invent dual entry

**Not this doc:** Bidirectional sync, CRDTs, merging repos, rewriting premflow in Node, auto-claiming ensembly pickups from every premflow task.

Companion law: [LIFE-OS-BOUNDARY.md](LIFE-OS-BOUNDARY.md) · [PLAYBOOK.md](PLAYBOOK.md) · [MAP.md](MAP.md) · [PRIVACY.md](PRIVACY.md)

---

## 0. Why this exists

You already run **three surfaces** by habit:

| System | What you feel it is |
|--------|---------------------|
| **life-os** (`~/life-os`) | Portfolio wiki — projects started, clustered, energy, Archives |
| **ensembly** (this repo) | Day clone — next body, next auth, day plan, dashboard, game |
| **premflow** (code `~/Work/personal/premflow`, data `~/.premflow/`, **portfolio card** `~/life-os/Projects/premflow/`) | Terminal micro-capture — note, task dump, pomo, review |

Without an explicit SoT, the same intention gets written three times (premflow todo + ensembly physical + life-os `next_action`). That is the thrash ensembly was built to remove.

**Executive verdict (think harder):**  
**Share the data, not three products merged.** One **filesystem tree** holds all micro-capture (notes, tasks, journal, pomo log). Three **views** (premflow CLI, ensembly `flow` wrapper, life-os vault link) read/write that **same** tree. Day/HITL and portfolio **frontmatter** stay separate kernels — “all shared” means **all attention-scatting capture**, not wait-snapshots or Obsidian project cards.

```text
                    ┌─────────────────────────────┐
                    │  SHARED SoT (one inode tree)│
                    │  ~/.premflow/               │
                    │  todo.txt · log.txt ·       │
                    │  journal/ · config.txt      │
                    └──────────▲──────────────────┘
               invent/list │         │ same files
          ┌────────────────┼─────────┼────────────────┐
          │                │         │                │
   premflow binary   ensembly flow    life-os card
   (C CLI)           wrapper          capture/ → symlink
                                     Projects/premflow/
```

---

## 1. Source of truth by concern

**SoT = Source of Truth** — the system that **wins** if copies disagree; the place where the **bytes** live.

| Concern | **Primary SoT (bytes / invent)** | Views (same data or none) | Do **not** invent here |
|---------|----------------------------------|---------------------------|-------------------------|
| **Notes / tasks / journal / pomo log** (micro-capture) | **`~/.premflow/`** files (premflow format) | `premflow …` · `node bin/swarm.js flow …` · `~/life-os/Projects/premflow/capture/` (symlink) | A second `todo.txt` under ensembly or a vault-only task list |
| **Portfolio / project memory** (Eisenhower card, energy, Archive) | **life-os** vault card frontmatter + sessions | — | Putting portfolio status only in premflow todos |
| **Day next-act + HITL** | **ensembly** kernel + wait snapshot | turn / dashboard / game | premflow as day planner |
| **Clone internal schedule** | **ensembly** `private/clone/` | — | premflow as clone mind |
| **Swarm activity audit** | **ensembly** `data/local/activity.sqlite` | optional | Replacing the note pad |

### Shared micro-capture law (shipped)

| Path | Role |
|------|------|
| `~/.premflow/` | **Byte SoT** — premflow C binary already uses this (`DATA_DIR`) |
| `node bin/swarm.js flow …` | **Wrapper** — same invent/list/review; one mental entry from ensembly |
| `~/life-os/Projects/premflow/capture` → `~/.premflow` | **Vault view** — same tree in Obsidian; not a copy |
| `npm run flow:link` / `node bin/swarm.js flow link` | Create/repair the life-os symlink |

**Rule of invention:**

1. **Notes / inbox / pomo / review** → write **only** into `~/.premflow/` (via `premflow` **or** `swarm flow` — same files).  
2. **HITL / body on the day plan** → **ensembly** turn/claim/approve only.  
3. **Portfolio product status** → life-os `Projects/*/README.md` frontmatter only.  
4. Never keep two independent open lists for the same micro-task.

---

## 2. Explicit refuses

| Refuse | Why |
|--------|-----|
| **Triple equal tracking** of the same todo in premflow + ensembly turn + life-os `next_action` | Three SoTs = desync and thrash |
| **Merge life-os into ensembly** (or ensembly into vault) | Memory ≠ runtime ([LIFE-OS-BOUNDARY.md](LIFE-OS-BOUNDARY.md)) |
| **Premflow as day/approvals kernel** | No privacy classifier, no durable HITL snapshot, no realm split |
| **life-os as always-on connector / turn host** | Vault is portfolio memory, not process |
| **Bidirectional multi-master sync** of three independent DBs / three `todo.txt` copies | Latency + conflict theater; use **one directory + views** instead |
| **Dual-live independent todo lists** (copy vault todos separate from `~/.premflow/`) | Instant drift |
| **Auto-claim/complete ensembly** from every `premflow task done` | Body/HITL need human pair honesty |
| **Committing `~/.premflow/` or vault private into ensembly git** | Privacy default-deny |
| **Re-implementing premflow UI inside Game of Peram chrome** | World > dashboard; premflow stays the TUI dump valve |

---

## 3. What each tool owns (sharp edges)

### 3.1 Shared micro-capture (premflow bytes + ensembly wrapper + life-os link)

| Command | Use for |
|---------|---------|
| `premflow` / `node bin/swarm.js flow` | Help (same SoT) |
| `… note "…"` / `flow note "…"` | Fleeting thought |
| `… win "…"` | Signal win |
| `… task add\|list\|done` | Inbox — **not** HITL |
| `… pomo N` | Focus clock |
| `… review` | Curated recap |
| `node bin/swarm.js flow path` | Print shared root + link health |
| `node bin/swarm.js flow link` | Symlink life-os `capture` → `~/.premflow` |
| `node bin/swarm.js flow snapshot` | Read-only dump of shared files |

**Data home (one):** `~/.premflow/` (`todo.txt`, `log.txt`, `journal/`, `config.txt`).

**life-os portfolio card:** `~/life-os/Projects/premflow/README.md` + `sessions/` for the *product*.  
**life-os capture view:** `~/life-os/Projects/premflow/capture` → symlink to `~/.premflow` (same inodes).

| What the card is | What the card is **not** |
|------------------|---------------------------|
| Portfolio registration + session notes for premflow *code/product* | A second inbox that diverges from `~/.premflow/todo.txt` |
| Parent of the `capture/` symlink | Day next-act / HITL SoT |

Do **not** re-list open body/HITL gates on this card; update frontmatter `next_action` only for **premflow product** work (e.g. “ship redesign”), not for “buy milk” dumps.

### 3.2 ensembly — day runtime SoT

| Command | Use for |
|---------|---------|
| `npm run swarm:turn` / `node bin/swarm.js turn` | **Next body** + **next auth** |
| `claim` / `complete` / `approve` / `deny` | Pair actions on the day kernel |
| `npm run swarm:day` | Morning structure when the plan is stale |
| `npm run swarm:dashboard` | Life glance (stats, insights) — not micro-notes |
| `npm run swarm:graph` | Watch map |
| `activity append` / `log append` | Durable swarm audit (optional), not premflow replacement |

### 3.3 life-os — portfolio memory SoT

| Action | Use for |
|--------|---------|
| Project `README.md` frontmatter | `next_action`, energy, progress, review_date |
| `sessions/YYYY-MM-DD.md` | What was worked after a real block |
| Areas / Portfolio-MOC / Archives | Long horizon; cluster experiments |

---

## 4. Daily operator loop (terminal-first)

Designed for **laptop + TUI day** (Grok Build, Cursor, shell). Keep it under five minutes of ceremony.

### Morning (or cold start)

```bash
cd ~/Work/personal/ensembly
# 0) once per machine: vault sees same capture tree
npm run flow:link           # → ~/life-os/Projects/premflow/capture -> ~/.premflow

# 1) Day truth — invent next body + next auth here
npm run swarm:turn
npm run swarm:dashboard     # optional glance

# 2) Micro inbox — same files whether you type premflow or flow
node bin/swarm.js flow task list
# node bin/swarm.js flow task add "email thrash"
# do NOT re-add ensembly physical pickups into todo.txt
```

### Deep work block (Cursor / Grok Build)

```bash
node bin/swarm.js flow pomo 25   # or: premflow pomo 25 — same ~/.premflow log
# HITL → ensembly approve/deny, not a new todo line
```

### Body / HITL pair moments

```bash
# From ensembly turn primaries only:
node bin/swarm.js claim <physical-id>
# … real world …
node bin/swarm.js complete <physical-id>

node bin/swarm.js approve <auth-id>   # or deny
```

If you first scribbled the body act in premflow: **complete/delete the premflow task** when you claim/complete in ensembly so only one open list remains.

### End of block / evening

```bash
node bin/swarm.js flow win "…"
node bin/swarm.js flow review      # curated; data still ~/.premflow

node bin/swarm.js turn --stdout    # day queues
# vault: open Projects/premflow/capture/ in Obsidian = same todo/log/journal
# portfolio frontmatter only if product status moved (README next_action)
```

### Decision micro-tree (when unsure where to type)

```text
Is this a HITL risk or body-world act on today's plan?
  YES → ensembly turn / claim / approve
  NO  → Is this a portfolio project fact (status, energy, archive)?
          YES → life-os card / session
          NO  → premflow note | task | pomo
```

---

## 5. Anti-duplication habits (dogfood)

| Habit | Do |
|-------|----|
| **One open list for gates** | Pending auth + physical live only in ensembly snapshot |
| **One open list for inbox** | Fleeting tasks live only in `premflow task list` |
| **One card per project** | life-os project README; premflow never becomes the card |
| **Promote once** | Inbox → day gate: add to ensembly (day/persona path) *or* claim existing; mark premflow done |
| **Session notes lag truth** | life-os session written **after** work, not as a parallel todo mid-block |
| **Activity SQLite is audit** | Optional ensembly log; do not re-enter every premflow note |

---

## 6. Optional later (not day-0 ship)

| Idea | Direction | Refuse |
|------|-----------|--------|
| Nightly export of premflow `review` lines into life-os session stub | one-way archive | auto-bidirectional |
| `swarm` command that prints “open premflow tasks” read-only | read reflection | treating them as ensembly physical |
| `Projects/premflow/capture` → `~/.premflow` | **Shipped** (`flow link`) — same tree in vault | second copy of todo.txt |
| Dashboard “open inbox” card reading shared todo | optional | treating todos as HITL |
| DomainEvent / shared IR | when a second machine consumer exists ([SWARM-DESIGN.md](SWARM-DESIGN.md)) | graph DB / multi-writer |

---

## 7. Privacy (capture + insights)

### Is the life-os `capture` symlink “fine”?

**Yes for local operator use** — a symlink does **not** copy bytes into the vault; Obsidian/ensembly/premflow all see the same files under `~/.premflow/`.

**Guards still required:**

| Risk | Guard |
|------|--------|
| Accidental `git add` of capture or packed vault export | life-os `.gitignore`: `Projects/premflow/capture`; never force-add |
| Cloud Obsidian / backup tools following links | Prefer local vault only for this path; treat capture as **private/**-class |
| Dashboard / Eve / public IR embedding raw todos | **Refuse** raw lines; use `projectCaptureForShare` (counts + public-only text) |
| Agents logging `flow snapshot` into commits | Use `flow path --json` / `flow snapshot --json` (share-safe redaction) |

### Insights must not leak

| Allowed in shareable IR | Forbidden |
|-------------------------|-----------|
| Counts: public vs private todos, log volume, journal file count | Full private todo text (bank, tax, medical, family health, …) |
| Lines already classified **public** (OSS/craft signals) | Dumping `todo.txt` / `log.txt` into `public/watch/` or Eve digests |
| Aggregate “inbox has N private items” | Training/cloud upload of capture trees |

Ship: `src/premflow/redact.js` (`classifyCaptureLine`, `redactCaptureLine`, `projectCaptureForShare`) — uses ensembly `classifyItem` default-deny + finance keywords.

| Path | Git / push |
|------|------------|
| `~/.premflow/` | **Local only** — never commit into ensembly |
| `~/life-os/Projects/premflow/capture` | **gitignore** in life-os; symlink pointer only if ever tracked by mistake |
| `~/life-os` other private notes | Vault rules |
| ensembly `private/`, `data/local/` | gitignored; default-deny |

---

## 8. Thrive (2036)

**North star:** Premflow stays the **zero-friction terminal valve**; ensembly stays the **day/HITL kernel**; life-os stays the **portfolio memory**. Three tools, one invention site per concern — capacity compounds without triple bookkeeping.

| Refuse | Build toward |
|--------|----------------|
| Unified mega-app for notes+day+vault | Thin protocol of use + rare one-way export |
| Cloud “personal OS” SoT | Local files + local kernel + redacted bridges |

**Near-term confidence:** 90%. **Thrive bet:** 85%.

---

## Cross-links

| Peer | Path |
|------|------|
| Boundary | [LIFE-OS-BOUNDARY.md](LIFE-OS-BOUNDARY.md) |
| Play loop | [PLAYBOOK.md](PLAYBOOK.md) |
| Map | [MAP.md](MAP.md) |
| Decisions | [DECISIONS.md](DECISIONS.md) |
| Privacy | [PRIVACY.md](PRIVACY.md) |
| **life-os card** | `~/life-os/Projects/premflow/README.md` · `sessions/` |
| **life-os capture** | `~/life-os/Projects/premflow/capture` → `~/.premflow` |
| **premflow code + README cross-ref** | `~/Work/personal/premflow/README.md` |
| **Wrapper** | `src/premflow/*` · `node bin/swarm.js flow` |
