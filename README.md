# Game of Peram

**ensembly** — life, played as a game.

Digital thrash is the trash mob. You keep the boss fights: **body-world pickups** and **authorization gates**. The swarm curates, prioritizes, and balances. You claim beacons, clear gates, and grow.

Production life infrastructure — not a toy. Bar: [PRODUCT-CHARTER.md](docs/PRODUCT-CHARTER.md) · [AGENTS.md](AGENTS.md)

**New here?** Start with **[docs/MAP.md](docs/MAP.md)** — live capabilities, CLI vs game vs watch, `src` / `public/game` / Rust WASM ownership, and what **IR** means in this repo.

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

Deep dives: [MAP](docs/MAP.md) · [PLAYBOOK](docs/PLAYBOOK.md) · [GAME-STACK](docs/GAME-STACK.md) · [ENGINE](docs/ENGINE.md) · [WORLD-FOUNDATION](docs/WORLD-FOUNDATION.md)

---

## Campaign modes (CLI)

| Mode | Command | You get |
|------|---------|---------|
| **Turn** | `npm run swarm:turn` | **Next** body act + **next** auth gate, full queues |
| **Status IR** | `node bin/swarm.js turn --json` | Machine-readable `next` / queues (agents) |
| **Day** | `npm run swarm:day` | Plan: projects · actions · schedule · balance · privacy |
| **Map** | `npm run swarm:graph` | Watch: next-action panel + mermaid / `public/watch/` |
| **Dashboard** | `npm run swarm:dashboard` | Life progress: stats · insights · overview → `public/watch/dashboard.html` |
| **Flow** | `node bin/swarm.js flow …` | Shared notes/tasks/pomo via premflow (`~/.premflow`; vault: `Projects/premflow/capture`) |
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

## Remote · channels (not only the laptop)

Same kernel decisions, different host:

| Need | Today | Trajectory |
|------|-------|------------|
| Next act away from desk | `turn --json` over SSH · watch HTML | Channel digest (Slack/web) |
| Approve / deny on phone | CLI on any shell | Eve tool-approval buttons |
| Morning nudge | Manual / local cron + CLI | Eve schedules → redacted turn |

**Adopt** Eve for channels + remote HITL + cron; **refuse** vault/persona on cloud.  
Playbook §5: [PLAYBOOK.md](docs/PLAYBOOK.md#5-remote--channels-not-just-the-laptop) · fit map: [EVE-FIT.md](docs/EVE-FIT.md) · roadmap: [coming-next.md](docs/arch-design/coming-next.md)

---

## Fog of war (privacy)

| Loot | Path | Git |
|------|------|-----|
| Full persona | `private/persona/` | **never push** |
| Public projection | `public/persona/` | ok |

Also never push: `private/`, `data/`, secrets. Classifier: `src/privacy.js` (default-deny). Rules: [PRIVACY.md](docs/PRIVACY.md).

---

## Map of the repo

Full orientation (capabilities · hosts · layers · IR): **[docs/MAP.md](docs/MAP.md)**

```text
bin/swarm.js           day · turn · approve · deny · graph
src/                   control plane (pure): day · turn · privacy · realm · graph IR
src/game/              pure session kit: focus · growth · $SPN · input (SoT for focus)
crates/peram-core/     shared Rust world/layout sim → WASM (mirrors focus; not control plane)
public/game/           thin browser host shell + paint
public/game/pkg/       checked-in wasm-pack build of peram-core (not hand-written logic)
public/watch/          static consumer of graph / turn-status export
docs/                  charter · MAP · engine · Eve · roadmap
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
