# AGENTS.md — binding rules for ensembly

Read this before any non-trivial change. These are **product law**, not suggestions.

---

## 0. What this project is

**ensembly** is production-grade **operator kernel** infrastructure: durable gates, pulses, and ledger for one operator (and later, people who share the same bar). It is **not** a demo, weekend toy, portfolio piece, or “MVP to throw away.” It is **not** a competing chat OS — Grok/Cursor own capture; ensembly owns **done / pending / denied**.

### Substrate (2026-09-04 Musk cut) — binding

| Layer | SoT | Notes |
|-------|-----|-------|
| **Control / HITL / turn / digital-flow** | **`crates/peram-kernel` (Rust)** | Expand control here. CLI: `cargo run -p peram-kernel -- …` |
| **Episodic memory** | `crates/peram-memory` (Rust) | CRDT trajectory; kernel consults on `reflect` only |
| **Agent wire** | `crates/peram-agents` (Rust) | Read-only `peram-mcp` for Grok/Cursor |
| **Data** | T1 SQLite + T2 PQ vault (peram-vault law) | Backup/restore are product paths |
| **Parked prototype** | `prototype/` | Game/watch/Node/WASM — history preserved, not SoT |

Full cut: [docs/MUSK-CUT-2026-09-04.md](docs/MUSK-CUT-2026-09-04.md).

### life-os vs this repo (do not confuse)

| | **`~/life-os`** | **ensembly (this repo)** |
|--|-----------------|---------------------------|
| Role | Clustered **Projects/Areas vault** | **Operator kernel** — gates, memory, pulse sync |
| Not | Always-on connector runtime | Game client or second chat inbox |

Binding detail: [docs/LIFE-OS-BOUNDARY.md](docs/LIFE-OS-BOUNDARY.md).

**Clone as copilot (phase 1):** Portfolio code projects under human oversee → PRs. Law: [docs/CLONE-COPILOT.md](docs/CLONE-COPILOT.md).

It must be:

| Pillar | Meaning |
|--------|---------|
| **Impactful** | Changes real days: less digital thrash, clearer physical work, honest HITL |
| **Complementary** | Scales with Grok Bot / Grok Build / Cursor — kernel under harness, not beside it |
| **Growth-oriented** | Capacity and craft compound; kernel records truth harnesses can replay |
| **Production-grade from day 0** | Ship paths that survive deploys, reloads, tests, privacy audits, daily dogfood |

---

## 1. Hard refuses

| Refuse | Why |
|--------|-----|
| Prototype theater at root | No half-wired game/watch as primary product surface |
| Dual live writers | Cloud or laptop must not dual-write `peram-ops.sqlite` |
| Second chat OS | No competing capture/inbox — complement Grok |
| Kernel rewrite thrash | Simplify by deletion, not greenfield rewrite of working Rust |
| Privacy laziness | Never commit or push `private/`, vaults, secrets |
| Unattended bank/email | External mutate only behind explicit human authorization |
| Scope cosplay | Multiplayer, Eve bridge, AAA game before kernel loop is *lived* |

---

## 2. Production bar (every PR / agent session)

1. **Dogfood path** — `cargo run -p peram-kernel -- runtime …`, `cargo test -p peram-kernel`, `cargo test -p peram-memory`, `cargo build -p peram-agents --bin peram-mcp`.
2. **Tests on the shipped path** — Pure logic unit-tested; no tests only for `prototype/`.
3. **Failure is loud and recoverable** — Status lines, errors; no silent wrong state.
4. **Privacy default-deny** — Classifier + gitignore + docs stay green.
5. **Single source of truth** — Kernel owns ops DB and runtime gates; harnesses read/write via CLI/MCP/pulse only.
6. **Impact is a requirement** — Features map to less friction, safer auth, or better sync — or they wait.

---

## 3. Architecture law

```text
Kernel (local, pure, audited) → life-state, gates, MsgBus, T1 SQLite, pulse-pack, memory bridge
Harness (Grok / Cursor / …)   → capture, codegen, chat — calls kernel, never owns vault
Bridge (Eve optional, trajectory) → channels, remote approval UX — redacted IR only
Prototype (parked)            → game/watch/Node — optional client experiments
```

---

## 4. Effort standard for agents

- Prefer **depth over breadth**: finish kernel surfaces before reviving prototype.
- Prefer **truth over theater**: if dogfood needs npm game, the cut failed — fix docs or kernel path.
- When unsure, load: privacy docs, [MUSK-CUT](docs/MUSK-CUT-2026-09-04.md), [MAP](docs/MAP.md).
- Do **not** answer with “quick prototype” language at repo root.

---

## 5. Definition of done (session)

1. Change is **testable** on kernel/memory/agents paths.
2. Operator can **measure** improvement (gate clarity, pulse sync, harness fit).
3. Docs updated if law or map changed.
4. No new joke / hobby smell in root diff.

---

## 6. Canonical docs

| Doc | Role |
|-----|------|
| [docs/MAP.md](docs/MAP.md) | Live kernel capabilities, CLI, pulse sync |
| [docs/PRODUCT-CHARTER.md](docs/PRODUCT-CHARTER.md) | Why kernel exists; complement Grok |
| [docs/PLAYBOOK.md](docs/PLAYBOOK.md) | Dogfood: runtime + pulse |
| [docs/MUSK-CUT-2026-09-04.md](docs/MUSK-CUT-2026-09-04.md) | What was deleted/parked and why |
| [prototype/README.md](prototype/README.md) | Parked game/Node surfaces |
| [docs/LIFE-OS-BOUNDARY.md](docs/LIFE-OS-BOUNDARY.md) | life-os vault vs kernel |
| [docs/PRIVACY.md](docs/PRIVACY.md) | Push boundary |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Material architecture decisions |

**Footer:** Build the kernel you will still trust in 90 days — because harnesses come and go.
