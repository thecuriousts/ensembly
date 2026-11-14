# ensembly

Thin operator kernel under capture harnesses — not a second chat OS.

Bots and harnesses **propose**. ensembly **authorizes, claims, and completes**. Pulse-pack syncs memory. One SQLite writer owns the ledger.

| | |
|--|--|
| **Law** | [PRODUCT-CHARTER.md](docs/PRODUCT-CHARTER.md) · [AGENTS.md](AGENTS.md) |
| **Map** | [docs/MAP.md](docs/MAP.md) · [GLOSSARY.md](docs/GLOSSARY.md) |
| **Cut** | [Musk algorithm](docs/MUSK-CUT-2026-09-04.md) — question, delete, simplify, accelerate, automate |
| **Score** | [docs/scoreboard.md](docs/scoreboard.md) — public CI first-pass, merges, open-to-merge time |
| **Essays** | [captain.kingsparrow.space/articles](https://captain.kingsparrow.space/articles) |

## Quickstart

**Use Grok Bot?** [Open in Grok Bot](https://cursor.com/grok-bot/link/v1/prompt?text=Set%20up%20ensembly%20for%20me%20on%20your%20own%20computer.%20Its%20source%20is%20https%3A%2F%2Fgithub.com%2Fthecuriousts%2Fensembly.%20Talk%20to%20me%20in%20plain%20words%3B%20I%20may%20not%20be%20a%20developer.%20%201.%20If%20~%2Fensembly%20does%20not%20exist%2C%20run%20%60git%20clone%20https%3A%2F%2Fgithub.com%2Fthecuriousts%2Fensembly%20~%2Fensembly%60.%20%20%20%20If%20it%20exists%2C%20check%20that%20%60git%20-C%20~%2Fensembly%20remote%20get-url%20origin%60%20points%20to%20thecuriousts%2Fensembly%20and%20that%20%60git%20-C%20~%2Fensembly%20status%20--porcelain%60%20prints%20nothing%2C%20then%20run%20%60git%20-C%20~%2Fensembly%20pull%20--ff-only%60.%20If%20any%20of%20that%20fails%2C%20ask%20me%20with%20a%20question%20card%20before%20going%20on.%202.%20Read%20~%2Fensembly%2Fdocs%2Fgrokbot%2FFIRST-RUN.md%20and%20follow%20every%20step%20in%20order%2C%20as%20one%20continuous%20run.%20Never%20skip%20a%20step.%20Never%20end%20your%20turn%20in%20the%20middle%20of%20setup%3B%20stop%20only%20to%20show%20me%20a%20question%20card.%20%20Never%20ask%20me%20for%20a%20password%2C%20token%20or%20API%20key.%20Never%20use%20sudo.%20Never%20delete%2C%20move%20or%20overwrite%20anything%20of%20mine%20without%20asking%20first.) fills in the install prompt (you press send), and Grok Bot installs and checks ensembly on its own computer. Fallback, the copy-paste prompt: [docs/install-with-grokbot.md](docs/install-with-grokbot.md).

**New to the terminal?** Follow [docs/START-HERE.md](docs/START-HERE.md): numbered steps for macOS, Linux and Windows, what to install first, what success looks like, and what to do when a step fails.

Prereqs: `git` and `cargo` on `PATH` (the installer tells you how to get them; it never installs toolchains or uses `sudo`). The first `cargo build` or `install.sh` run may download the pinned toolchain from `rust-toolchain.toml` via rustup — network use is cargo/rustup only, not ensembly telemetry.

```mermaid
%%{init: {"theme":"base","themeVariables":{"primaryColor":"#1e3a5f","primaryTextColor":"#ffffff","lineColor":"#f59e0b","fontFamily":"Inter"}}}%%
flowchart TB
  subgraph cli["plain CLI"]
    I1[install.sh --harness cli] --> N1[ensembly init]
    N1 --> S1[runtime sample]
    S1 --> T1[stranger-test.sh]
  end
  subgraph cursor["Cursor"]
    I2[install.sh --harness cursor] --> N2[init + doctor]
  end
  subgraph claude["Claude Code"]
    I3[install.sh --harness claude] --> N3[init + skills]
  end
  subgraph grok["Grok Bot"]
    I4[install.sh --harness grok] --> N4[load plugin from checkout]
  end
  classDef cli fill:#1e3a5f,stroke:#60a5fa,color:#ffffff
  classDef cur fill:#4c1d95,stroke:#a78bfa,color:#ffffff
  classDef cla fill:#065f46,stroke:#34d399,color:#ffffff
  classDef grk fill:#78350f,stroke:#fbbf24,color:#ffffff
  class I1,N1,S1,T1 cli
  class I2,N2 cur
  class I3,N3 cla
  class I4,N4 grk
```

| Harness | One install command (from repo root) |
|---------|--------------------------------------|
| **CLI** | `sh scripts/install.sh --harness cli` |
| **Cursor** | `sh scripts/install.sh --harness cursor` (copies the plugin; Cursor skips symlinks there) |
| **Claude Code** | `sh scripts/install.sh --harness claude` |
| **Grok Bot** | `sh scripts/install.sh --harness grok` |
| **All** | `sh scripts/install.sh --harness all` |

Then: `ensembly init` (writes `${XDG_CONFIG_HOME:-~/.config}/ensembly/config.toml`; see [docs/CONFIG.md](docs/CONFIG.md)).  

**Paths:** After `init`, the CLI and `ensembly-mcp` resolve ops DB and memory together from `paths.data_dir` when you omit `--db` and `--memory`. Existing files under `data/local/` or `private/state/` in the working directory still win (nothing is moved). With a single `--db` or `--memory`, the paired path is: an existing peer next to the flagged file, else an existing peer under cwd `data/local` (or `private/state` for ops), else a new path beside the flag (stderr `note:`). Single-flag mode never pairs from configured `data_dir`.

Verify like a stranger: `sh scripts/stranger-test.sh` (temp `HOME`, synthetic fixture, full workspace tests).  
Update an existing checkout: `sh scripts/install.sh --update --harness …`.

---

## What it owns

- **Authorize / claim / complete** — durable HITL/HOOTL state you own
- **T1 SQLite ledger** — one writer; no dual ops DB
- **Episodic memory** — CRDT aux; never decides
- **Pulse-pack** — portable memory sync between hosts

## What sits beside it

| Piece | Role |
|-------|------|
| **Grok Bot / Build / Cursor / …** | Capture — propose work |
| **[participatory-mesh](https://github.com/thecuriousts/participatory-mesh)** | Multi-device hand — after claim, CommandFabric **dispatches** an allowlisted act onto another participant |
| **life-os vault** (`$LIFEOS`) | Portfolio cards — not this repo ([boundary](docs/LIFE-OS-BOUNDARY.md)) |
| **`prototype/`** | Parked game / watch / Node — not SoT |

Honest mesh scope: multiplies *where* approved work can run. Does **not** own the life ledger. Bridge: [mesh-bridge](docs/thinking/mesh-bridge-2026-09-08.md).

---

## Drop in

See **Quickstart** above for `scripts/install.sh`. Developers hacking the kernel without installing:

Optional **opportunity-finder** pack (application ledger / company audit CLI): [`crates/ensembly-opportunity-finder/README.md`](crates/ensembly-opportunity-finder/README.md) — enable with `cargo … -p ensembly-kernel --features opportunity-finder`.

```bash
cargo test -p ensembly-kernel
cargo test -p ensembly-memory
cargo build -p ensembly-agents --bin ensembly-mcp
```

### Runtime

```bash
cargo run -p ensembly-kernel -- runtime load --fixture fixtures/issue-1-runtime.json
cargo run -p ensembly-kernel -- runtime status
cargo run -p ensembly-kernel -- runtime tick          # one HOOTL step
cargo run -p ensembly-kernel -- runtime approve pay-rent
cargo run -p ensembly-kernel -- runtime claim grocery-errand
cargo run -p ensembly-kernel -- runtime complete grocery-errand
cargo run -p ensembly-kernel -- runtime reflect
```

DB: `data/local/ensembly-ops.sqlite` · memory: `data/local/ensembly-memory.json`  
(Legacy `peram-*` opens in place — [RENAME.md](RENAME.md) · [DECISIONS.md](docs/DECISIONS.md))

### Pulse pack (memory only)

```bash
cargo run -p ensembly-kernel -- pulse-pack export --out /tmp/session.pulse.json
cargo run -p ensembly-kernel -- pulse-pack import --pack /tmp/session.pulse.json
```

Canonical host = single ops writer. Laptop imports packs — no dual-write. [PLAYBOOK.md](docs/PLAYBOOK.md)

### Agent wire (read-only)

```bash
cargo build -p ensembly-agents --bin ensembly-mcp
# grok mcp add --scope project ensembly -- cargo run -p ensembly-agents --bin ensembly-mcp
```

---

## Install with Grok Bot

Paste one prompt into Grok Bot and it installs, configures and checks ensembly for you: [docs/install-with-grokbot.md](docs/install-with-grokbot.md). Optional dedicated bot: [docs/ensembly-bot-template.md](docs/ensembly-bot-template.md).

---

## Plugins

Catalog `thecuriousts-plugins` ([.cursor-plugin/marketplace.json](.cursor-plugin/marketplace.json)).

```mermaid
%%{init: {"theme":"base","themeVariables":{"primaryColor":"#1e3a5f","primaryTextColor":"#ffffff","lineColor":"#f59e0b"}}}%%
flowchart LR
  C[thecuriousts-plugins]:::cat --> F[flowingstack]:::fs
  C --> E[ensembly]:::en
  classDef cat fill:#1e3a5f,stroke:#60a5fa,color:#ffffff
  classDef fs fill:#4c1d95,stroke:#a78bfa,color:#ffffff
  classDef en fill:#065f46,stroke:#34d399,color:#ffffff
```

| Plugin | Path | Job |
|--------|------|-----|
| [flowingstack](marketplaces/cursor/flowingstack/README.md) | `marketplaces/cursor/flowingstack` | Task contracts, run ledger, frozen eval sets, review checks |
| [ensembly](marketplaces/grokbot/ensembly/README.md) | `marketplaces/grokbot/ensembly` | Record layer for every bot |

---

## Repo map

```text
crates/ensembly-kernel/   authorize/claim · MsgBus · SQLite · pulse-pack
crates/ensembly-memory/   episodic CRDT (aux)
crates/ensembly-agents/   ensembly-mcp (read-only)
docs/                     charter · MAP · PLAYBOOK · mesh bridge · scoreboard
marketplaces/             flowingstack (cursor) · ensembly (grokbot)
prototype/                parked — not product
```

External companion: [participatory-mesh](https://github.com/thecuriousts/participatory-mesh)

---

## Privacy · license

Never push `private/`, `data/`, or secrets — [PRIVACY.md](docs/PRIVACY.md).

MIT ([LICENSE.md](LICENSE.md)). Private life data is not part of the grant.

**Rule:** automate the digital · surface the physical · wait only for permission · complement the harness.
