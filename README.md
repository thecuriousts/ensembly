# Game of Peram

**ensembly** — life, played as a game.

Digital thrash is the trash mob. You keep the boss fights: **body-world pickups** and **authorization gates**. The swarm curates, prioritizes, and balances. You claim beacons, clear gates, and grow.

Production life infrastructure — not a toy. Bar: [PRODUCT-CHARTER.md](docs/PRODUCT-CHARTER.md) · [AGENTS.md](AGENTS.md)

**life-os vs this repo:** `~/life-os` is the clustered Projects/Areas **vault** (portfolio memory). **ensembly** is the **digital clone** that removes digital friction so you pair for physical + HITL. See [LIFE-OS-BOUNDARY.md](docs/LIFE-OS-BOUNDARY.md).

**Copilot:** Free to work portfolio code projects — internal schedule in `private/clone/`, proposals for human oversee, then PRs. See [CLONE-COPILOT.md](docs/CLONE-COPILOT.md).

---

## Drop in

```bash
# Node ≥ 20  ·  WASM prebuilt (rebuild only if you change Rust)
npm test
npm run game          # → http://127.0.0.1:4173/game/
npm run game:smoke
```

Open with the **trailing slash**: `/game/`.

Optional: `npm run build:wasm` after editing `crates/peram-core`.

---

## How to play & steer (productivity)

**Full operator guide (laptop + remote channels + CLI recipes):**  
**[docs/PLAYBOOK.md](docs/PLAYBOOK.md)** — play Game of Peram, run turns, claim/complete, **$SPN**, remote/Eve trajectory.

### Quick controls (game)

Default: **clean courtyard**. Focus chip + **$SPN** ticker (top-right) + key hint.

| Input | Move |
|-------|------|
| **Tab** / j k / arrows | Cycle focus (beacons) |
| **Enter** / Space / **C** | **Claim** beacon → XP · **$SPN** up |
| **A** / Y | **Approve** gate → XP |
| **D** / N | **Deny** gate → XP · **$SPN** down |
| **B** / Q | Growth **board** |
| **I** | Status **bars** (level, XP, **$SPN**) |
| **M** | Command menu (type / voice) |
| **?** / H | Codex (help) |
| **Esc** | Clear chrome |

### Progression

| Axis | What counts |
|------|-------------|
| **Body** | Physical pickups |
| **Presence** | Family / health / schedule |
| **Craft** | Deep work & ship |
| **Gates** | HITL approve / deny |

Claim → XP → streak → levels (**Ember → Horizon**). Coach steers off pure digital grind. **$SPN** prices the session from real events (`src/game/spn.js`).

### Engine room

| Layer | Role |
|-------|------|
| Courtyard world | Env · sprites · props |
| `peram-core` (Rust → WASM) | Layout & sim hot path |
| Thin JS host | Input · chrome · voice · **$SPN** paint |
| Session store | **Focus source of truth** (WASM only mirrors) |

Deep dives: [PLAYBOOK](docs/PLAYBOOK.md) · [GAME-STACK](docs/GAME-STACK.md) · [ENGINE](docs/ENGINE.md) · [WORLD-FOUNDATION](docs/WORLD-FOUNDATION.md)

---

## Campaign modes (CLI)

| Mode | Command | You get |
|------|---------|---------|
| **Turn** | `npm run swarm:turn` | **Next** body act + **next** auth gate, full queues |
| **Status IR** | `node bin/swarm.js turn --json` | Machine-readable `next` / queues (agents) |
| **Day** | `npm run swarm:day` | Plan: projects · actions · schedule · balance · privacy |
| **Map** | `npm run swarm:graph` | Watch: next-action panel + mermaid / `public/watch/` |
| **Gate** | `node bin/swarm.js approve <id>` | Clear a wait snapshot |
| | `node bin/swarm.js deny <id>` | |
| **Body** | `node bin/swarm.js claim <id>` | Claim physical pickup |
| | `node bin/swarm.js complete <id>` | Complete physical (leave open queue) |

```bash
node bin/swarm.js turn --fixture fixtures/state-sample.json --stdout
node bin/swarm.js turn --fixture fixtures/state-sample.json --json --no-write
```

Foundation critique (visualize → act): [docs/FOUNDATION-CRITIQUE.md](docs/FOUNDATION-CRITIQUE.md).
---

## Multiplayer later · Eve bridge

Remote ops (chat digests, gate buttons, cron wake-ups) map to **[Vercel Eve](https://vercel.com/eve)** as a production bridge — not a kernel rewrite.

| Fits Eve | Stays local |
|----------|-------------|
| Channels (Slack / web) | Day · privacy · realm pure logic |
| Remote approve / deny | Full persona vault |
| Schedules / digests | Game of Peram WASM world |

Full map: [EVE-FIT.md](docs/EVE-FIT.md) · Roadmap: [coming-next.md](docs/arch-design/coming-next.md)

---

## Fog of war (privacy)

| Loot | Path | Git |
|------|------|-----|
| Full persona | `private/persona/` | **never push** |
| Public projection | `public/persona/` | ok |

Also never push: `private/`, `data/`, secrets. Classifier: `src/privacy.js` (default-deny). Rules: [PRIVACY.md](docs/PRIVACY.md).

---

## Map of the repo

```text
bin/swarm.js           day · turn · approve · deny · graph
src/day.js             day campaign loop
src/realm.js           physical vs digital
src/approvals.js       durable gate waits
src/game/              session · growth · input · world
crates/peram-core/     shared Rust sim (WASM today, desktop later)
public/game/           playable host + prebuilt pkg
public/watch/          static map viewer
docs/                  charter · engine · Eve · roadmap
legacy/                old webpack app (not the game)
```

---

## Codex extras

- **Skills:** looper · stellar-roadmap · fusion-sage · Patterns.dev (command / observer)
- **Sovereignty gist:** [public/thinking/sovereignty-gist.md](public/thinking/sovereignty-gist.md)  
  Study notes inspired by Palantir — [*Institutional Sovereignty in the Age of AI*](https://www.palantir.com/ai-sovereignty-is-your-alpha/) ([PDF](https://assets.ctfassets.net/xrfr7uokpv1b/yF0AXklHQd7K3SqKICNTM/e9f9167d1b3c7cce56ab3b8c4cc572da/Palantir_-_Institutional_Sovereignty_in_the_Age_of_AI.pdf)). Not affiliated.

---

## License

MIT ([LICENSE.md](LICENSE.md)). Your private life data is not part of the grant.

**Rule of the realm:** automate the digital · surface the physical · wait only for permission · make the truth playable.
