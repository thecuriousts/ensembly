# ensembly

**Persona-driven autonomous life swarm** — daily self-organization that balances career, family presence, health capacity, and systems work under a **privacy-safe** control loop.

This repo is no longer the legacy react-boilerplate demo (moved to `legacy/`). The product is a **runnable swarm day cycle**.

## Quick start

```bash
# Node >= 20
npm test
npm run swarm:day              # writes private/state/plans/<date>-daily-plan.md
npm run swarm:day:stdout       # print plan only (no write)
node bin/swarm.js day --date 2026-07-12 --stdout --no-write
```

## What one run produces

A **daily swarm plan** with:

1. **Loop control** — looper phases, budgets, pause reason  
2. **Projects** — inception / prioritization (Eisenhower + balance weights)  
3. **Actions** — curated, bounded (not an unbounded dump)  
4. **Schedule & balance** — rhythm blocks + non-negotiable presence  
5. **Privacy split** — public vs private + HITL gates  

## Persona

| Artifact | Path | Git |
|----------|------|-----|
| Full operator persona | `private/persona/full.{md,json}` | **ignored** (local only) |
| Public-safe projection | `public/persona/projection.{md,json}` | tracked |

Full persona is derived from the operator’s local life context (`~/life-os/private/llm.txt`) and collab-finder distillation themes. Only the scrubbed projection is commit-eligible. See [docs/PRIVACY.md](docs/PRIVACY.md).

## Privacy contract (short)

- **Readable/copyable locally:** private inputs, full persona, private actions  
- **Never pushed:** `private/`, `data/local/`, `data/private/`, secrets  
- **Pushable:** source, tests, docs, public persona, public event examples  
- Classifier: `src/privacy.js` (default-deny)

## Architecture

```text
bin/swarm.js          CLI entry
src/day.js            day orchestration + runDailySwarm
src/loop.js           looper-shaped phases / budgets / HITL
src/prioritize.js     Eisenhower + balance weights
src/balance.js        non-negotiable inject + schedule
src/privacy.js        public vs private classification
src/ingest.js         load persona/state
src/plan-format.js    markdown artifact
public/persona/       public projection
private/              local-only (gitignored)
legacy/               old webpack app (not the product)
```

Design notes: [docs/SWARM-DESIGN.md](docs/SWARM-DESIGN.md) · decisions: [docs/DECISIONS.md](docs/DECISIONS.md)

## Skills

Implementation is grounded in:

- **looper** — bounded outer loop (globally available for Grok at `~/.grok/skills/looper`)  
- **ai-optimization** — slim context packs  
- **fusion-sage** — fused daily-plan abstraction + surplus  
- **higher-order-decision-architect** — privacy/architecture choices  

## License

MIT (see LICENSE.md). Operator private data is not part of the license grant and must not be published.
