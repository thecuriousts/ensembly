# Legacy archive

This directory holds the **original react-boilerplate / webpack app** that shipped with the first commits of this repository. It is **not** the product.

The product is the persona-driven swarm control plane at the repository root:

- `bin/swarm.js` — CLI entry (`npm run swarm:day`)
- `src/` — prioritization, balance, privacy, looper day cycle
- `public/persona/` — public-safe operator projection
- `docs/` — privacy contract and design notes

Do not restore this stack as the default app without an explicit decision. Prefer extending `src/` and the day loop.
