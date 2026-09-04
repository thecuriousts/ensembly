# Foundation critique — visualize → pick up → take action

**Date:** 2026-07-13  
**Lens:** Does the shipped path help the *operator* (real human) and the *agent clone* (CLI/agent tooling) see the truth, claim one body-world act, and clear one auth gate — or does it only look like a control plane?

---

## Verdict (honest)

The kernel (day plan, privacy default-deny, realm split, durable wait snapshot, graph IR) is a **solid substrate**. The operator surfaces before this iteration **failed the last mile**: equal-weight checklists, markdown-only turn output, no durable close for physical work, and a watch page that was diagram-or-bomb with no “do this next” panel.

That is not foundation collapse — it is **actionability debt** on top of good IR.

---

## What already solidly supports visualize → pickup → act

| Choice | Why it passes |
|--------|----------------|
| **Realm split** (`physical` vs `digital` + HITL) | Correct product cut: body work is human; digital advances behind auth. |
| **Day plan + schedule from rhythm** | Capacity-aware day, not infinite backlog cosplay. |
| **Wait snapshot + approve/deny** | Durable idle-resume for authorizations; human and agent share one truth file. |
| **Pure modules + CLI thin host** | Testable offline kernel; agents can call the same functions as the human path. |
| **Graph IR (nodes/edges) without Stately peer** | Serializable map for watch/game; no hobby dependency thrash. |
| **Privacy default-deny** | Visualization never requires shipping the vault. |

These choices *lay* a foundation. They do not by themselves *close* a real morning loop.

---

## What fails the visualize → pickup → act test

| Failure | Operator cost | Agent cost |
|---------|---------------|------------|
| **Equal-weight pickup lists** | “Which one *now*?” — thrash or freeze | No single target id to pursue |
| **Markdown-only turn** | Fine for eyes; brittle for tools | Must regex prose; breaks on wording |
| **Auth closable, body work not** | Grocery stays forever in the queue after you did it | Clone cannot mark physical done |
| **Watch = Mermaid or error** | No next act, no copy-ready commands | Graph IR alone ≠ action surface |
| **Unquoted Mermaid labels** (pre-fix) | Diagram dead when titles have `()` / `&` | Visual truth offline for real plans |

---

## 2–4 fixes locked from this verdict (this iteration)

Chosen because each removes a specific failure above and is usable **today** on `node bin/swarm.js` / tests — not roadmap theater.

1. **Primary next physical + next authorization** on the turn surface (schedule-aware when schedule exists; injectable `now` for deterministic ranking). Markdown and IR both lead with *one* body act and *one* auth gate.
2. **Machine-readable turn status IR** (`turn-status.json` + `--json` on the real CLI turn path) so the agent clone reads queues and next-actions without parsing markdown.
3. **Physical claim / complete** durable decisions on the same wait-snapshot model as approve/deny — body work can leave the open queue.
4. **Actionable watch panel** (next physical + next auth + copy-ready commands) with **Mermaid-safe quoted labels** so real titles render and the panel is not diagram-or-nothing.

**Non-chosen this iteration (still valid later):** Eve bridge, game HUD redesign, WASM rewrite — they do not close today’s morning loop.

---

## Definition of “useful” for human + clone

| Actor | Success looks like |
|-------|-------------------|
| **You** | Open turn → one next body act (title, window if any, claim/complete command) → one next auth (approve/deny commands) → do it. |
| **Clone (agent)** | Read `turn-status.json` or `--json` → same ids → run claim/complete/approve/deny → re-read status. |

If either actor still has to invent ranking or re-parse prose, the foundation is incomplete.

---

## Binding follow-through

- Code: `src/turn.js`, `src/approvals.js`, `src/graph.js`, `bin/swarm.js`
- Dogfood: `node bin/swarm.js turn --fixture fixtures/state-sample.json --stdout`
- Status: `node bin/swarm.js turn --fixture fixtures/state-sample.json --json --no-write`
- Body close: `node bin/swarm.js claim <id>` / `complete <id>`
- Watch: `npm run swarm:graph` → `public/watch/index.html`
