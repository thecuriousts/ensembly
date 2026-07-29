# Glossary — expanded abbreviations

**Rule:** no bare acronyms in law, docs, or review-facing text. Expand at first use in every document; keep this file as the single lookup. Add rows when a new term ships.

---

## Control plane (kernel)

| Term | Full text | What it means here |
|------|-----------|---------------------|
| **SoT** | Source of Truth | The one place a decision is made and stored. `crates/peram-kernel` is the control Source of Truth; everything else mirrors or advises. |
| **S** | Life-State | Durable operator state object (graph, regime, metrics, fingerprint) owned by the runtime. |
| **G** | Dependency Graph (`DepGraph`) | Directed graph of life actions with gates, realms, and dependencies. |
| **CP** | Critical Path | Longest-path chain through the dependency graph — the sequence that actually gates the day. |
| **P** | PERT + Monte Carlo | Program Evaluation and Review Technique (expected duration/σ per node) plus optional Monte Carlo sampling of path length. |
| **S+G+CP+P** | State + Graph + Critical Path + PERT | Shorthand for the whole Issue #1 runtime bundle. |
| **MsgBus** | Message Bus | Typed in-memory queue inside the runtime; every effect (manual command, trigger, agent report) travels as a message. |
| **HITL** | Human-In-The-Loop | Work that waits for an explicit human decision (approve/deny gates, physical claims). |
| **HOOTL** | Human-Out-Of-The-Loop | Digital thrash agents may clear autonomously because it is safe to do so. |
| **Digital thrash** | — | Low-value digital chores (triage, drafts) that clutter the critical path; HOOTL agents claim these. |
| **T1** | Tier 1 storage | Local ops SQLite (`data/local/peram-ops.sqlite`) — durable, gitignored, never pushed. |
| **T2** | Tier 2 storage | Sealed/encrypted tier: vault bridge, backup packs (AES-GCM sealed blobs). |
| **IR** | Intermediate Representation | Versioned JSON/markdown contracts between pure kernel logic and hosts/agents (turn status IR, wait snapshot IR, graph IR). See MAP.md §4. |
| **Regime** | Loop Regime | Runtime mode: `Hootl` (agents clear digital work) vs `HitlWait` (blocked on a human gate/beacon). |

## Learning layer (peram-memory)

| Term | Full text | What it means here |
|------|-----------|---------------------|
| **CRDT** | Conflict-free Replicated Data Type | Data structure whose replicas merge without coordination; our trajectory/skills/goals document converges under concurrent CLI writers. |
| **Trajectory** | Episodic trajectory | Append-only log of what actually happened: observations, actions, reflections. |
| **Coherence** | Coherence score | 0–1 similarity measure (word-set Jaccard) between consecutive trajectory entries; proxy for "does what I'm doing hang together". |
| **Skill synthesis** | — | When the same 3-action pattern repeats past threshold, it is crystallized into a named, reusable `Skill`. |
| **Reflect** | Reflection pass | Explicit `runtime reflect`: scores coherence, runs skill synthesis, proposes goals. Never implicit inside a control tick. |
| **Aux** | Auxiliary | The memory layer's legal status: it records and learns; it never decides gates, critical path, or priorities. |
| **Judge** | Inference provider (roadmap P2) | Pluggable backend for coherence scoring and reflection summaries. **Default:** deterministic Jaccard (zero network). **Optional adapters:** Ollama HTTP, Grok Model Context Protocol, opencode Agent Client Protocol / Model Context Protocol, pi, others — selected at runtime, never a hard dependency. Unavailable provider warns and falls back to deterministic. |
| **InferenceProvider** | Inference provider trait | Rust trait in `peram-memory`: score coherence, summarize reflection. Test oracle is always the deterministic implementation. |
| **DelegationBackend** | Delegation backend trait | Rust trait for Human-Out-Of-The-Loop digital hands. **First adapter:** Grok Agent Client Protocol. Secondary: opencode Agent Client Protocol, pi. |

## Agents and protocols (roadmap P3–P5)

| Term | Full text | What it means here |
|------|-----------|---------------------|
| **LLM** | Large Language Model | Any model via an InferenceProvider adapter. Fast path on this machine: Grok CLI `grok -p` ([docs](https://docs.x.ai/build/cli/headless-scripting)). Not a crate dependency. |
| **pi** | pi (agent runtime) | External agent/toolchain; secondary DelegationBackend adapter when wired. |
| **Grok ACP** | Grok Agent Client Protocol | Official: `grok agent stdio` — JSON-RPC NDJSON ([docs](https://docs.x.ai/build/cli/headless-scripting#acp)). |
| **Grok MCP** | Grok Model Context Protocol | Official: `grok mcp add` / `.grok/config.toml` ([docs](https://docs.x.ai/build/features/mcp-servers)). Tools namespaced `server__tool`. |
| **MCP** | Model Context Protocol | Standard JSON-RPC tool/resource surface for model hosts (Cursor, opencode, Eve). Planned as read-only `memory_*` / `kernel_status` tools first. |
| **ACP** | Agent Client Protocol | Agent-to-agent session protocol (used by `opencode acp`); planned path for delegating HOOTL digital work. |
| **JSON-RPC** | JSON Remote Procedure Call | Request/response envelope used by MCP and ACP. |
| **NDJSON** | Newline-Delimited JSON | One JSON value per line; the stdio framing for our protocol plumbing. |
| **P2P** | Peer-to-Peer | Direct replica sync between machines; the CRDT merge is ready, transport is undecided (needs a product decision). |
| **Eve** | Vercel Eve | Production remote bridge candidate: channels, remote approval buttons, cron schedules — redacted IR only, never vault/persona. |

## Game and world

| Term | Full text | What it means here |
|------|-----------|---------------------|
| **WASM** | WebAssembly | `crates/peram-core` compiled to `public/game/pkg/` for the browser world sim. |
| **$SPN** | Spandan (game ticker) | Personal tape of real claim/HITL events rendered in the game host. |
| **Beacon** | Physical Beacon | Next physical-world action surfaced on the map; only a body can clear it. |
| **AuthGate** | Authorization Gate | Next pending approval; cleared only by an explicit human approve/deny. |

---

*Lost once already (IntelliArch `01_supervised/GLOSSARY.md`, uncommitted). Now committed here so it cannot silently vanish.*
