# ensembly — Game of Peram

**Persona-driven life swarm** that removes digital friction: the agent curates, prioritizes, balances, and classifies; **you** only pick up the **physical world** and grant **authorizations**. Play and watch state as a **game world + graph**.

This is **production life infrastructure** — not a hobby demo or joke MVP. Fun, engagement, and real growth impact are requirements. See **[docs/PRODUCT-CHARTER.md](docs/PRODUCT-CHARTER.md)** and **[AGENTS.md](AGENTS.md)**.

Legacy react-boilerplate lives under `legacy/`. Product = **control plane** + **operator turn** + **immersive game** + **production Eve bridge** (when shipped — not a throwaway prototype).

## Quick start

```bash
# Node >= 20 · Rust/wasm-pack only if rebuilding WASM (pkg is prebuilt under public/game/pkg)
npm test
npm run game                   # http://127.0.0.1:4173/game/
npm run game:smoke             # serve + fetch + drive session
npm run build:wasm             # optional: Rust → public/game/pkg

npm run swarm:turn             # physical pickups + pending approvals
npm run swarm:day
npm run swarm:graph            # mermaid + public/watch (static)
node bin/swarm.js approve <id>
node bin/swarm.js deny <id>
```

### Play the Game of Peram (interactive)

```bash
npm run game         # open http://127.0.0.1:4173/game/
```

**Important:** open **http://127.0.0.1:4173/game/** (trailing slash). `/game` redirects. Assets mount at `/game/*`; pure modules at `/src/game/*`.

| Control | Action |
|---------|--------|
| Tab / j k / arrows | Navigate focus (session is source of truth; WASM mirrors) |
| Enter / Space | Select |
| A / Y | Approve pending HITL |
| D / N | Deny |
| ? | Help overlay |
| V | Voice mode (Web Speech or text proxy) |
| Gamepad | D-pad nav · A approve · B deny |

**Stack:** courtyard **world foundation** (env / sprites / props) · **Rust WASM** (`peram-core`) layout + sim · **WebGPU** with Canvas2D fallback · thin JS host (input, HUD, voice). Focus index lives in the JS session store only — the world never double-advances.

Docs: [GAME-STACK.md](docs/GAME-STACK.md) · [ENGINE.md](docs/ENGINE.md) · [WORLD-FOUNDATION.md](docs/WORLD-FOUNDATION.md) · [PATTERNS-DEV.md](docs/PATTERNS-DEV.md)

## Operator turn (primary human surface)

```bash
node bin/swarm.js turn --fixture fixtures/state-sample.json --stdout
```

You get:

1. **Physical world pickups** — errands, outdoor family, body/presence (agents cannot do these)  
2. **Pending authorizations** — approve/deny resumes a durable wait snapshot (`private/state/wait-snapshot.json`)  
3. Snapshot **status/phase** advances on decision (not just flags in a long plan)

## Day plan (digital automation)

Produces Projects / Actions / Schedule & balance / Privacy split via looper phases (`npm run swarm:day`).

## Game graph watch

```bash
node bin/swarm.js graph --stdout          # mermaid
node bin/swarm.js graph --html            # public/watch/index.html + graph.json
```

Serializable IR: nodes (phase, action, physical, hitl, schedule) + edges + layout. Inspired by [Stately graph](https://stately.ai/docs/packages/graph); no hard peer required.

## Vercel Eve (optional bridge)

**[docs/EVE-FIT.md](docs/EVE-FIT.md)** maps what [Vercel Eve](https://vercel.com/eve) should own vs what stays local:

| Concern | Eve? | Notes |
|---------|------|--------|
| User communication (Slack / web chat digests) | **Yes** | Channels = remote operator surface |
| Remote approve/deny of side effects | **Yes** | Tool `approval` parks durable session |
| Cron schedules (morning plan, HITL nag) | **Yes** | `schedules/` → Vercel Cron |
| Day / privacy / realm kernel | **No rewrite** | Eve tools *call* pure modules / CLI |
| Full private persona on cloud | **Refuse** | Projections only — [PRIVACY.md](docs/PRIVACY.md) |
| Game of Peram WASM world | **No** | Local/browser (later desktop) |

Kernel first; Eve is the production remote host for comms/approval/schedules (roadmap SN-5), not a kernel rewrite and not a disposable prototype.

## Roadmap

Architecture backlog: [docs/arch-design/coming-next.md](docs/arch-design/coming-next.md) — thrive picture, SN cards, Eve bridge, multiplayer ascent.

## Persona & privacy

| Artifact | Path | Git |
|----------|------|-----|
| Full operator persona | `private/persona/` | **ignored** |
| Public projection | `public/persona/` | tracked |

- **Never pushed:** `private/`, `data/`, secrets  
- **Pushable:** code, tests, docs, public persona, public watch IR, prebuilt `public/game/pkg`  
- Classifier: `src/privacy.js` (default-deny)

See [docs/PRIVACY.md](docs/PRIVACY.md).

## Architecture

```text
bin/swarm.js              day | turn | approve | deny | graph
src/day.js                day cycle
src/realm.js              physical vs digital
src/approvals.js          durable idle snapshot approve/deny
src/turn.js               operator turn surface
src/graph.js              game graph IR + mermaid + HTML
src/loop.js               looper phases / budgets
src/privacy.js            public vs private
src/game/                 pure session, input, voice, world foundation
crates/peram-core/        shared Rust world (WASM today; desktop later)
public/game/              interactive host + prebuilt WASM pkg
public/watch/             static graph watch
docs/EVE-FIT.md           Eve bridge decision map
docs/arch-design/         stellar roadmap
```

## Skills

stellar-roadmap · fusion-sage · ai-optimization · higher-order-decision-architect · looper · Patterns.dev (command/observer/perf under `.agents/skills/`)

## Sovereignty gist (public)

AI sovereignty doctrine (ZDR, model liquidity, owned context flywheel) as a short public gist:

- [public/thinking/sovereignty-gist.md](public/thinking/sovereignty-gist.md)
- `src/sovereignty-gist.js` (step ids + assurance helpers)

**Credit:** Study notes inspired by Palantir — [*Institutional Sovereignty in the Age of AI*](https://www.palantir.com/ai-sovereignty-is-your-alpha/) ([PDF](https://assets.ctfassets.net/xrfr7uokpv1b/yF0AXklHQd7K3SqKICNTM/e9f9167d1b3c7cce56ab3b8c4cc572da/Palantir_-_Institutional_Sovereignty_in_the_Age_of_AI.pdf)). Not affiliated with Palantir.

Detailed personal mapping stays **local-only** under gitignored `private/thinking/`.

## License

MIT (see LICENSE.md). Operator private data is not part of the license grant.
