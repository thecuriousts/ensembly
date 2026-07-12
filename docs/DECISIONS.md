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

**Verdict:** Treat [Vercel Eve](https://vercel.com/eve) as an **optional bridge**, not a kernel rewrite.

| Decision | Rationale |
|----------|-----------|
| **Adopt** Eve for channels + tool approval + cron schedules | Matches operator needs: remote communication, remote control/approval, morning/evening cadence ([EVE-FIT.md](EVE-FIT.md)) |
| **Adapt** tools that call pure ensembly modules / CLI | Keeps unit tests + offline dogfood; Eve owns durability UX |
| **Refuse** full persona / vault on Vercel Eve by default | Privacy default-deny + sovereignty gist |
| **Refuse** rewriting day/privacy/realm into Eve-only prompts | Iron-peak stays pure Node ESM |
| **Refuse** game sim on Eve | WASM world is local host |

**Near-term:** document + dogfood CLI/game first; prototype Eve out-of-tree or `bridge/eve/` only after turn dogfood. Confidence: 80%.