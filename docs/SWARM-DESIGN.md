# Swarm design (fusion + looper)

## Fused abstraction

**Life swarm control plane** = persona model + Eisenhower/balance prioritizer + privacy classifier + looper outer day cycle → one **daily self-organization artifact**.

Legacy react-boilerplate UI is **not** the product (`legacy/`). The product is `bin/swarm.js day`.

## Skills grounding

| Skill | Role here |
|-------|-----------|
| **looper** | Outer phases ORIENT→PLAN→EXECUTE→VERIFY→REVIEW_GATE→INTEGRATE→DONE; budgets; HITL gates (`src/loop.js`) |
| **ai-optimization** | Slim inputs: persona JSON + optional local state only; no vault dump into plans |
| **fusion-sage** | Single iron-peak: *balanced daily plan artifact* reusable by agents; surplus = schedule+privacy co-assembly |
| **higher-order-decision-architect** | Material choices: greenfield Node ESM vs upgrade legacy; default-deny privacy; HITL non-goal for unattended bank/email |

## Day loop (bounded)

```text
ORIENT  load persona (private full if present else public) + local state
PLAN    merge candidates, prioritize, ensure balance
EXECUTE assemble markdown artifact (no external side effects)
VERIFY  non-empty + Projects/Actions/Schedule sections + actions present
REPAIR  re-assemble only (budgeted)
REVIEW_GATE  list HITL-flagged actions
INTEGRATE write private/state/plans/YYYY-MM-DD-daily-plan.md
DONE
```

Hard budgets: `max_loop_iters=8`, `max_repair_rounds=3` (see looper skill defaults).

## Balance thesis (persona)

Non-negotiables (Relationships presence, Health energy) are scheduled **before** career deep work fills capacity. Career gets at most one primary DO_FIRST bet per day in the action set; noise (ELIMINATE) is dropped.

## Entry

```bash
npm run swarm:day
npm run swarm:day:stdout
node bin/swarm.js day --date 2026-07-12 --stdout --no-write
```

## ⚡ Fusion surplus

A shared `DomainEvent` + `LoopCard` JSON schema under `public/events/` would let life-os / collab-finder / ensembly interoperate without re-parsing markdown — implement when a second consumer appears.
