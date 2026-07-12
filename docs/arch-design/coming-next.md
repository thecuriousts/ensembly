# Coming next — ensembly (Game of Peram)

**Audience:** Operator · implementer · agents  
**Style:** Short words. Diagrams over prose. Optimism grounded in evidence.  
**Contract:** [PRIVACY.md](../PRIVACY.md) · [SWARM-DESIGN.md](../SWARM-DESIGN.md) · [DECISIONS.md](../DECISIONS.md)  
**Method:** stellar-roadmap · fusion-sage · ai-optimization · higher-order-decision-architect · looper

*Last updated: 2026-07-13*

---

## 0. Mission (one sentence)

Remove digital friction so the swarm automates curate/plan/classify while the operator only picks up **physical world** work and grants **approvals** — and can watch the agent play the **Game of Peram** as a graph.

---

## 0b. Ten-year thrive picture (2036 — not survival, ascent)

Tailwinds: local-first agents, Stately-style durable HITL resume, human-in-the-loop product patterns (Eve approvals), graph viz as shared situational awareness.

```mermaid
flowchart TB
  subgraph kernelY["Kernel — ship computer"]
    K1[persona + balance + privacy]
    K2[looper day cycle]
    K3[idle snapshot approve deny]
  end
  subgraph bridgeY["Product bridge — command ops"]
    B1[operator turn surface]
    B2[game graph IR + layout]
    B3[watch room multiplayer later]
  end
  subgraph weather["Cosmic weather we punch through"]
    W1[vendor agent frameworks]
    W2[cloud host churn]
  end
  weather --> kernelY
  kernelY --> bridgeY
  bridgeY --> OUT[Human judgment + physical presence]
```

| 2036 role | What it is | Why it still wins |
|-----------|------------|-------------------|
| **Kernel** | Pure prioritize/balance/privacy/loop + durable wait snapshots | Host-agnostic; testable; privacy default-deny |
| **Bridge** | Turn surface, graph play view, optional Eve/Stately adapters | Swap renderers; keep iron-peak state machine |
| **Boundary** | Physical pickups + explicit approve/deny only | Human energy is scarce; agents do digital chores |

**Design bet:** Kernel forever = harmonious life-state control plane. Today’s renderer (CLI/markdown/HTML) is disposable. Multiplayer voice room is ascent, not a side quest to abandon.

---

## 1. Scorecard — what landed (swarm MVP → game altitude)

```mermaid
flowchart LR
  subgraph shipped["Shipped A"]
    D[day loop]
    P[privacy classify]
    B[balance inject]
    T[turn surface]
    G[game graph]
    W[WASM world]
  end
  subgraph open["Next altitude B/C"]
    E[Eve bridge]
    M[multiplayer voice]
  end
  shipped --> open
```

| Area | Grade | One line | Evidence |
|------|-------|----------|----------|
| Day self-org plan | A | Projects/Actions/Schedule from persona | `src/day.js`, `npm run swarm:day` |
| Privacy default-deny | A | Finance/medical private; pushable gated | `src/privacy.js`, `test/privacy.test.js` |
| Looper phases/budgets | A | ORIENT→…→DONE with budgets | `src/loop.js`, `test/loop.test.js` |
| Public/private persona split | A | Full local, projection public | `public/persona/`, `private/` gitignored |
| Physical pickup queue | A− | Realm tag + turn lists physical | `src/realm.js`, `src/turn.js` |
| Durable approve/deny | A− | Idle snapshot resume | `src/approvals.js` |
| Game graph watch | A− | Nodes/edges + mermaid/HTML | `src/graph.js`, `public/watch/` |
| Immersive game world | A− | Env/sprites/props + WASM focus SoT | `public/game/`, `crates/peram-core`, `npm run game` |
| Eve bridge map | B | Fit doc: channels/HITL/schedules only | [EVE-FIT.md](../EVE-FIT.md), SN-5 |
| Multiplayer voice room | C | Ascent only | SN-6 |

**Plain rule:** Digital automates; human touches physical world + authorizations.

---

## 2. System map (today + target)

```mermaid
flowchart TB
  persona[Persona public or private]
  state[Local state JSON]
  day[Day loop buildDayPlan]
  realm[Realm physical vs digital]
  priv[Privacy classify]
  snap[Wait snapshot HITL]
  turn[Operator turn]
  graph[Game graph IR]
  watch[Watch mermaid or HTML]
  persona --> day
  state --> day
  day --> realm
  day --> priv
  day --> snap
  realm --> turn
  snap --> turn
  day --> graph
  snap --> graph
  graph --> watch
  turn -->|approve deny| snap
```

**Fused abstraction:** *Game of Peram control plane* = day plan + realm split + idle-snapshot approvals + exportable graph. Trace: `src/day.js`, `src/approvals.js`, `src/graph.js`.

---

## 3. Operator data-flow (friction kill)

```mermaid
sequenceDiagram
  participant H as Human
  participant S as Swarm
  participant W as WaitSnapshot
  participant G as GraphWatch
  S->>S: curate prioritize balance privacy
  S->>W: pending authorizations
  S->>H: physical pickups + pending list
  H->>W: approve or deny
  W->>S: resume digital path
  S->>G: export play state
  H->>G: watch or join later
```

| Layer | Owns | Must not |
|-------|------|----------|
| Day loop | Digital plan assembly | External mutate without HITL |
| Turn surface | Physical queue + approval UI/CLI | Hide pending gates in prose only |
| Snapshot | Durable legal events | Lose wait state across sessions |
| Graph | Shared situational awareness | Require full multiplayer runtime day one |
| Privacy | Default-deny private paths | Commit `private/` |

---

## 4. Musk five-step — applied to backlog

| Step | Question | Verdict |
|------|----------|---------|
| 1. Requirements | What must human still do? | Physical presence + approve/deny only |
| 2. Delete | What digital friction dies? | Manual prioritization, rediscovering HITL in long plans |
| 3. Simplify | One turn command | `swarm turn` surfaces both queues |
| 4. Accelerate | Graph export pure + tested | No layout peer required for IR |
| 5. Automate | Day loop already ships | Keep; attach snapshot + graph |

---

## 5. Trajectory forces (evidence-weighted)

| Force | P(horizon) | Effect on us | Response | Confidence |
|-------|------------|--------------|----------|------------|
| Stately agent HITL idle resume | high | Pattern for durable wait | Mirror snapshot events; optional adapter later | 75% |
| Vercel Eve channels + approvals + schedules | high | Remote comms / HITL / cron digests | Bridge only; see EVE-FIT — not kernel rewrite | 85% |
| Graph viz (`@statelyai/graph`) | med | Play-view polish | IR first; layout peer optional | 70% |
| Voice multiplayer rooms | med | Watch + join | SN backlog; not gate MVP | 55% |
| Privacy regulation / family data | high | Leak cost extreme | Default-deny + ignore + classifier | 90% |

**Acceleration trigger:** When operator uses `swarm turn` daily for a week, invest in watch room + optional Eve/Stately adapters — do not shrink autonomy pillars.

---

## 6. Trajectory guardrails

```mermaid
flowchart TD
  subgraph avoid["Refuse — drag"]
    A1[Unattended bank or email]
    A2[Commit private persona]
    A3[Rewrite kernel onto Eve before dogfood]
    A4[Legacy webpack as product UI]
    A5[Upload private persona to Eve cloud]
  end
  subgraph build["Build toward 2036"]
    B1[Turn surface physical + HITL]
    B2[Idle snapshot resume]
    B3[Game graph plus WASM world]
    B4[Autonomous digital chores]
    B5[Eve channel bridge redacted]
  end
```

| Refuse | Build toward |
|--------|----------------|
| 24/7 unattended external mutate | Background digital work with HITL gates |
| Defeatist “game pillar dies” | Game of Peram as north-star play surface |
| Scope-creep multiplayer first | Dogfood day+turn+graph+game before voice room |
| Eve as persona vault | Eve as channel + approval + schedule bridge |

---

## 7. Blueprint cards SN-*

### SN-1 · Dogfood gate (no new product surface)

**Problem:** Agents expand scope before the day path is still green.

```mermaid
flowchart LR
  test[npm test] --> day[swarm day stdout]
  day --> priv[privacy tests]
```

| File | Work |
|------|------|
| `package.json` | keep scripts |
| `test/*` | green |

**Done when:** `npm test` pass; day plan still has Projects/Actions/Schedule.

**Verify:** `npm test && npm run swarm:day:stdout | head`

---

### SN-2 · Physical vs digital realm + pickups

**Problem:** Operator cannot see what only a body in the physical world can do.

```mermaid
flowchart LR
  action[Action] --> realm[classifyRealm]
  realm --> phys[physical queue]
  realm --> dig[digital auto]
```

| File | Work |
|------|------|
| `src/realm.js` | physical/digital classify |
| `src/turn.js` | list physical pickups |
| `fixtures/*` | physical-tagged samples |

**Done when:** Turn surface lists ≥1 physical item when state includes physical actions.

**Verify:** `node bin/swarm.js turn --fixture fixtures/state-sample.json`

---

### SN-3 · Durable approve/deny idle snapshot

**Problem:** HITL was only flags in a plan, not a resumable wait state.

```mermaid
stateDiagram-v2
  [*] --> IdleWaiting
  IdleWaiting --> Resolved: approve
  IdleWaiting --> Resolved: deny
  Resolved --> [*]
```

| File | Work |
|------|------|
| `src/approvals.js` | snapshot create/apply |
| `bin/swarm.js` | `approve` / `deny` / `turn` |
| `fixtures/wait-snapshot.json` | sample |

**Done when:** Approve/deny changes pending queue status on disk/JSON snapshot.

**Verify:** turn → approve id → turn shows advanced status.

---

### SN-4 · Game graph export + watch

**Problem:** Cannot watch the agent “play” as nodes/edges.

```mermaid
flowchart TB
  state[Day plus snapshot] --> ir[Graph IR]
  ir --> mermaid[Mermaid]
  ir --> html[public/watch]
```

| File | Work |
|------|------|
| `src/graph.js` | nodes/edges export |
| `public/watch/` | simple viewer |
| `test/graph.test.js` | shipped path |

**Done when:** Graph has phase/action nodes + edges; test passes; optional HTML exists.

**Verify:** `node bin/swarm.js graph --stdout`

---

### SN-5 · Eve bridge (channels · remote approval · schedules)

**Problem:** Operator needs **remote communication**, **approve/deny away from CLI**, and **cron digests** without rewriting the kernel or leaking private persona.

```mermaid
flowchart TB
  sched[Eve schedules cron] --> tools[Tools wrap swarm]
  tools --> ch[Channels Slack web]
  ch --> human[Operator]
  human -->|approve deny| appr[Eve tool approval]
  appr --> ir[approvals IR]
  tools --> kernel[day realm privacy pure]
  kernel -.->|never| vault[private persona vault]
```

| File | Work |
|------|------|
| [docs/EVE-FIT.md](../EVE-FIT.md) | **Fit map shipped** — adopt/adapt/refuse |
| `docs/DECISIONS.md` | Eve host decision logged |
| future `bridge/eve/` or out-of-tree | Prototype tools + one channel + morning schedule (redacted fixtures) |
| `src/approvals.js` | Keep IR; dual-write adapter when prototype lands |

**Adopt on Eve:** channels (user comms), tool `approval` (remote control), `schedules/` (cadence).  
**Keep local:** day/privacy/realm pure functions, WASM game, private vault.  
**Full map:** [EVE-FIT.md](../EVE-FIT.md).

**Done when (docs gate — met):** Fit matrix + privacy refuse rules + sequence in EVE-FIT.  
**Done when (code gate — later):** Prototype posts turn digest on a channel; one gated tool maps to approve/deny; morning cron fires without uploading `private/`.

**Verify:** Doc review now; later `eve eval` + privacy checklist on tool outputs.

---

### SN-6 · Multiplayer watch room + voice (ascent)

**Problem:** Operator wants to join, instruct by voice, while agent backgrounds chores.

```mermaid
flowchart TB
  bg[Background digital] --> room[Game room]
  human[Human enter] --> room
  room --> collab[Shared graph state]
```

| File | Work |
|------|------|
| backlog only | realtime + voice | 

**Done when:** Separate goal; graph IR already multiplayer-ready shape.

**Verify:** Future goal plan.

---

## 8. Scope lock

| Locked in | Deferred |
|-----------|----------|
| Day plan automation | Eve production deploy |
| Physical + approval turn + game world | Voice multiplayer room |
| Graph IR + mermaid/HTML + WASM play | `@statelyai/graph` layout peers required |
| Eve fit map (SN-5 docs) | Eve code bridge with real Slack |
| Privacy default-deny | Live bank/email; private vault on cloud |

---

## 9. Gantt sprint order

```mermaid
gantt
  title ensembly Game of Peram near-term
  dateFormat YYYY-MM-DD
  section Dogfood
  SN1_tests_day           :done, sn1, 2026-07-12, 1d
  section Turn
  SN2_physical_realm      :done, sn2, 2026-07-13, 1d
  SN3_approve_deny        :done, sn3, 2026-07-13, 1d
  section Watch
  SN4_game_graph          :done, sn4, 2026-07-13, 1d
  SN4b_wasm_world         :done, sn4b, 2026-07-13, 1d
  section Bridge
  SN5_eve_fit_docs        :done, sn5d, 2026-07-13, 1d
  SN5_eve_prototype       :sn5p, 2026-07-14, 14d
  section Ascent
  SN6_multiplayer_voice   :sn6, after sn5p, 30d
```

---

## 10. Monitoring signals

| Signal | Healthy | Act |
|--------|---------|-----|
| `npm test` | green | Fix before features |
| Turn physical count | matches tagged actions | Fix realm classifier |
| Pending after approve | decreases | Fix snapshot apply |
| Day plan sections | present | Fix day path |
| Private in git status | never | Fix gitignore |

---

## 11. Done log

| When | What |
|------|------|
| 2026-07-12 | Swarm MVP: day loop, privacy, persona split, looper global |
| 2026-07-13 | Tag `v0.1.0` at legacy tip; turn/graph altitude |
| 2026-07-13 | Immersive game world + WASM focus SoT; `npm run game` |
| 2026-07-13 | Eve fit map: channels/HITL/schedules adopt; kernel refuse rewrite ([EVE-FIT.md](../EVE-FIT.md)) |

---

## 12. File touch mindmap

```mermaid
mindmap
  root((ensembly))
    src
      day
      realm
      approvals
      turn
      graph
      privacy
      loop
      game
    crates
      peram-core
    bin
      swarm
    docs
      arch-design
      EVE-FIT
      PRIVACY
    public
      game
      watch
      persona
    test
```

---

## 13. References

| Source | Use |
|--------|-----|
| [Stately agent docs (next)](https://github.com/statelyai/agent/tree/next/docs) | HITL / durable agent patterns |
| [Stately graph package](https://stately.ai/docs/packages/graph) | Graph IR inspiration |
| [Stately graph layout](https://stately.ai/docs/packages/graph/layout) | Layout adapters later |
| [Vercel Eve](https://vercel.com/eve) | Optional bridge: channels, approvals, schedules |
| [EVE-FIT.md](../EVE-FIT.md) | Project decision map adopt/adapt/refuse |
| [Introducing eve](https://vercel.com/blog/introducing-eve) | Product primitives + durable HITL |
| [looper skill](~/.grok/skills/looper/SKILL.md) | Outer loop budgets/phases |
| [PRIVACY.md](../PRIVACY.md) | Push boundary |
| [SWARM-DESIGN.md](../SWARM-DESIGN.md) | Day cycle iron-peak |

---

**Footer plain rule:** Automate the digital; surface the physical; wait only for explicit permission.
