# Peer review — engrim vs ensembly edge

**Status:** Binding peer thesis  
**Date:** 2026-09-07  
**Peer:** [engrim](https://github.com/timgordontg/engrim) (Tim Gordon) — useful reference, not a port target

---

## Peer

**engrim** is a local-first, project-scoped episodic memory product in Python: SQLite SoT, multi-harness hooks and MCP (Antigravity, Claude Code, Cursor, Windsurf), hybrid recall (FTS5 bm25 + model2vec static embeddings), a curated **hot pack** (~4k chars) for session boot, a separate flight-recorder log, agent **provenance** (`origin_agent`), and a safe-to-clear workflow (`review` before `/clear`).

It is the **Switzerland of AI memory** — decoupled from any single vendor, optimized for “switch models mid-project without re-explaining architecture.” Polished memory *product* with empirical dogfood (105-session case study, 99%+ context cost reduction on reload).

**Useful peer, not a port.** ensembly borrows ideas; it does not become a Python memory sidecar.

---

## Take (ideas worth carrying)

| Idea | engrim shape | ensembly lens |
|------|--------------|---------------|
| **Hot-pack budget** | `engrim context -b 4000` — priority-ordered boot slice | Pulse / channel exports should ship a **budget-capped** operator slice, not raw CRDT dump |
| **Dual log vs curated** | Flight recorder (turns) separate from curated `memories` table | Kernel **records applied bus messages**; `reflect` promotes patterns — never confuse attempt with fact |
| **Hybrid recall** | FTS5 + vector RRF fusion | Spike lexical + embedding scoring in `ensembly-memory`; deterministic default stays test oracle |
| **Provenance** | `origin_agent` on every record | Tag memory entries with harness id (Grok Bot, Cursor, local Grok) for fleet debugging |
| **Thin adapters** | `engrim setup --cursor`, hook runners, stdio MCP | Keep `ensembly-mcp` read-only; harness-specific wiring stays outside kernel |
| **Global ⊕ project** | `~/.engrim/memory.db` with per-project scope | Operator-global episodic trajectory **plus** project-scoped recall filters — life kernel is not repo-only |
| **Safe-to-clear UX** | `engrim review` — uncaptured decisions before wipe | Export pulse + surface “pending capture” before session end; complement harness `/clear` |

---

## Edge — where ensembly can exceed

engrim wins on **memory-as-product polish**: setup, hooks, hybrid search, continue-as-clear. ensembly’s thesis is **operator kernel** — memory is a facet of gates, ledger, and sync, not a standalone Switzerland app.

| # | Edge | Why it matters |
|---|------|----------------|
| **1** | **HITL/HOOTL gated writes** | Memory is **governed**, not only searchable. Approvals, claims, and denials land in T1 SQLite before episodic record. engrim `add` is harness-driven; ensembly `approve\|deny\|claim\|complete` is control truth. |
| **2** | **One-writer coherent SoT across multi-tool scatter** | Done / pending / denied is one ops ledger. Grok Bot, Cursor, and local Grok are clients — they do not each own a parallel pending store. Memory augments; kernel decides. |
| **3** | **Portable pulse packs / multi-host sync** | Bot ↔ laptop memory merge via `pulse-pack` without dual-writing ops. engrim is single-machine local; ensembly ships cross-host continuity as a product path. |
| **4** | **Temporal ledger / time-slice recall** | “What did I believe **as of** Tuesday?” — ledger + trajectory, not only top-k **now**. Channel-pulse fingerprints and runtime snapshots enable as-of-when replay. |
| **5** | **Live multi-agent fleet coordination** | Grok Bot + Cursor + local Grok as simultaneous clients over one kernel: HOOTL agents claim via CP; MsgBus records applied facts. engrim coordinates memory across harnesses; ensembly coordinates **work state**. |
| **6** | **Rust always-on reconciler** | No Python on the critical path. Channel-pulse reconcile, backup/restore, and runtime ticks are native, auditable, offline-capable. |
| **7** | **Complement harness memory layers** | Grok/Cursor retain their own context flavors; ensembly is the **operator layer underneath** — gates + episodic aux + pulse — not a replacement inbox. |

**Honest:** engrim is further along as a polished episodic memory product. ensembly’s edge is **operator kernel + gates + sync + time** — not winning the “install `pip install` and go” memory UX race on day one.

---

## Non-goals

| Refuse | Why |
|--------|-----|
| Python melt | Kernel stays Rust; no `pip install ensembly-memory` sidecar on the control path |
| Metaphor-as-architecture | “Switzerland,” “white hole,” etc. do not become schema names or APIs |
| Claiming CRDT/sync solved | Pulse-pack is file-copy merge today; peer-to-peer and multi-master remain deferred |
| Port engrim | No FTS5/model2vec drop-in; borrow patterns, ship ensembly law |
| Second chat OS | Memory serves kernel truth; capture stays in harness surfaces |

---

## Follow-ups

- [ ] **Hot-pack + provenance on pulse** — budget-capped export slice with harness origin tags (mirror engrim `context` + `origin_agent` on pulse-pack v2 shape)
- [ ] **Hybrid scoring spike in `ensembly-memory`** — lexical + embedding RRF behind `InferenceProvider`; deterministic Jaccard remains default and test oracle

---

**Binding context:** [PRODUCT-CHARTER.md](PRODUCT-CHARTER.md) · [MAP.md](MAP.md) · [DECISIONS.md](DECISIONS.md) (episodic memory layer, 2026-07-29)
