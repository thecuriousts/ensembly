# ensembly — Game of Peram

**Persona-driven life swarm** that removes digital friction: the agent curates, prioritizes, balances, and classifies; **you** only pick up the **physical world** and grant **authorizations**. Watch the play state as a **game graph**.

Legacy react-boilerplate lives under `legacy/`. Product = control plane + operator turn + graph watch.

## Quick start

```bash
# Node >= 20 · Rust/wasm-pack for race-car engine (optional if pkg prebuilt)
npm test
npm run build:wasm             # Rust → public/game/pkg
npm run game                   # http://127.0.0.1:4173/game/
npm run game:smoke             # serve + fetch + drive session

npm run swarm:turn             # physical pickups + pending approvals
npm run swarm:day
npm run swarm:graph            # mermaid + public/watch (static)
node bin/swarm.js approve <id>
node bin/swarm.js deny <id>
```

### Play the Game of Peram (interactive)

```bash
npm run build:wasm   # once (needs rustc + wasm-pack)
npm run game         # open http://127.0.0.1:4173/game/
```

**Important:** open **http://127.0.0.1:4173/game/** (trailing slash). `/game` redirects. Assets mount at `/game/*`; pure modules at `/src/game/*`.

| Control | Action |
|---------|--------|
| Tab / j k / arrows | Navigate focus |
| Enter / Space | Select |
| A / Y | Approve pending HITL |
| D / N | Deny |
| ? | Help overlay |
| V | Voice mode (Web Speech or text proxy) |
| Gamepad | D-pad nav · A approve · B deny |

Stack: **Rust WASM** layout hot path · **WebGPU** with Canvas2D fallback · pure JS session/input/voice. See [docs/GAME-STACK.md](docs/GAME-STACK.md).

**Patterns.dev agent skills** ([catalog](https://www.patterns.dev/ai/skills/)) are installed under `.agents/skills/` and applied in the game kit (Command, Observer, performance). See [docs/PATTERNS-DEV.md](docs/PATTERNS-DEV.md).

**World foundation** — courtyard stage with sprites/props; **sim in Rust WASM** (`peram-core`), thin JS host. Desktop-ready crate split. See [docs/ENGINE.md](docs/ENGINE.md) · [docs/WORLD-FOUNDATION.md](docs/WORLD-FOUNDATION.md).

## Operator turn (primary human surface)

```bash
node bin/swarm.js turn --fixture fixtures/state-sample.json --stdout
```

You get:

1. **Physical world pickups** — errands, outdoor family, body/presence (agents cannot do these)  
2. **Pending authorizations** — approve/deny resumes a durable wait snapshot (`private/state/wait-snapshot.json`)  
3. Snapshot **status/phase** advances on decision (not just flags in a long plan)

## Day plan (digital automation)

Still produces Projects / Actions / Schedule & balance / Privacy split via looper phases.

## Game graph watch

```bash
node bin/swarm.js graph --stdout          # mermaid
node bin/swarm.js graph --html            # public/watch/index.html + graph.json
```

Serializable IR: nodes (phase, action, physical, hitl, schedule) + edges + grid layout positions. Inspired by [Stately graph](https://stately.ai/docs/packages/graph); no hard peer required.

## Roadmap

Architecture backlog (stellar-roadmap): [docs/arch-design/coming-next.md](docs/arch-design/coming-next.md) — Game of Peram thrive picture, SN cards, multiplayer/voice ascent.

## Persona & privacy

| Artifact | Path | Git |
|----------|------|-----|
| Full operator persona | `private/persona/` | **ignored** |
| Public projection | `public/persona/` | tracked |

- **Never pushed:** `private/`, `data/`, secrets  
- **Pushable:** code, tests, docs, public persona, public watch IR  
- Classifier: `src/privacy.js` (default-deny)

See [docs/PRIVACY.md](docs/PRIVACY.md).

## Architecture

```text
bin/swarm.js           day | turn | approve | deny | graph
src/day.js             day cycle
src/realm.js           physical vs digital
src/approvals.js       durable idle snapshot approve/deny
src/turn.js            operator turn surface
src/graph.js           game graph IR + mermaid + HTML
src/loop.js            looper phases / budgets
src/privacy.js         public vs private
public/watch/          graph watch surface
docs/arch-design/      stellar roadmap
```

## Skills

stellar-roadmap · fusion-sage · ai-optimization · higher-order-decision-architect · looper

## Sovereignty gist (public)

AI sovereignty doctrine (ZDR, model liquidity, owned context flywheel) as a short public gist:

- [public/thinking/sovereignty-gist.md](public/thinking/sovereignty-gist.md)
- `src/sovereignty-gist.js` (step ids + assurance helpers)

**Credit:** Study notes inspired by Palantir — [*Institutional Sovereignty in the Age of AI*](https://www.palantir.com/ai-sovereignty-is-your-alpha/) ([PDF](https://assets.ctfassets.net/xrfr7uokpv1b/yF0AXklHQd7K3SqKICNTM/e9f9167d1b3c7cce56ab3b8c4cc572da/Palantir_-_Institutional_Sovereignty_in_the_Age_of_AI.pdf)). Not affiliated with Palantir.

Detailed personal mapping stays **local-only** under gitignored `private/thinking/`.

## License

MIT (see LICENSE.md). Operator private data is not part of the license grant.
