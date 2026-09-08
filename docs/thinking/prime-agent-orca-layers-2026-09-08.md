# Prime Agent × Orca × ensembly — layer map (2026-09-08)

Sources:
- [PrimeIntellect-ai/prime-agent](https://github.com/PrimeIntellect-ai/prime-agent) — self-improving RLM harness (Continual Harness + daemon sessions)
- [stablyai/orca](https://github.com/stablyai/orca) — ADE for parallel CLI agents / worktrees

Scope: **placement**, not a product dependency. Patterns only. No CI burn. Complements [nemoclaw-fuse-2026-09-08.md](nemoclaw-fuse-2026-09-08.md) and binding [PRODUCT-CHARTER.md](../PRODUCT-CHARTER.md) · [MAP.md](../MAP.md).

## Stack (top → bottom)

```
┌─────────────────────────────────────────────────────────────┐
│ L5  Operator intent                                         │
│     CoS plate · four-slot · Mission Brief · HITL body/auth  │
├─────────────────────────────────────────────────────────────┤
│ L4  Fleet / ADE surface          ← stablyai/orca sits HERE  │
│     Parallel worktrees · mobile steer · which harness runs  │
│     Does NOT own done/pending/denied                        │
├─────────────────────────────────────────────────────────────┤
│ L3  Coding / research harness    ← prime-agent sits HERE    │
│     OpenCode · Grok Build · Cursor CLI · pi · Claude/Codex  │
│     Capture + long-running work; proposes; never authorizes │
├─────────────────────────────────────────────────────────────┤
│ L2  Wire / adapters                                         │
│     ensembly-mcp (read) · ACP/CLI hooks · pulse-pack files  │
├─────────────────────────────────────────────────────────────┤
│ L1  Operator kernel (ensembly)   ← OWN THIS                 │
│     ensembly-kernel  gates · MsgBus · ops SQLite (one writer)│
│     ensembly-memory  CRDT trajectory (aux; never decides)   │
│     ensembly-agents  MCP satellite + InferenceProvider trait│
├─────────────────────────────────────────────────────────────┤
│ L0  Portfolio vault              ← ~/life-os (not in git)   │
│     Readable self-model · project cards · session notes     │
└─────────────────────────────────────────────────────────────┘
```

Monopoly sentence (unchanged): ensembly owns **coherent state across scatter**, not another process UI.

## Where each product sits

| Product | Layer | Role in ensembly world | Owns | Does not own |
|---------|-------|------------------------|------|--------------|
| **stablyai/orca** | **L4 ADE / fleet UI** | Desktop+mobile orchestrator: spin N agents in worktrees, steer from phone, SSH remotes, annotate diffs | Session layout, which CLI agent runs where, parallel compare/merge UX | Ops ledger, gates, pulse SoT, life-os vault |
| **prime-agent** | **L3 harness** | Long-running coding/research worker: RLM REPL, Continual Harness `/refine`, daemon attach, family-tree A2A, bounded `/autonomous` | Session trajectory inside its own harness, supplemental prompts/skills local to session | Kernel authorize/deny, cross-host ops SQLite, portfolio truth |
| **OpenCode / Grok Build / Cursor CLI / pi** | **L3 peers** | Same layer as prime-agent — interchangeable rented intelligence | Capture + codegen under policy | Same as above |
| **ensembly-kernel** | **L1** | White hole: done / pending / denied, runtime tick, HITL/HOOTL, pulse-pack export/import | `ensembly-ops.sqlite` (canonical host only) | Chat UX, worktree ADE |
| **ensembly-memory** | **L1 aux** | Episodic CRDT; `reflect` learns; never fires gates | `ensembly-memory.json` | Authorization |
| **ensembly-mcp** | **L2** | Read-only tools for any L3 harness | Query surface | Writes to ops |
| **life-os** | **L0** | Human-readable self model | Markdown cards / sessions | Runtime clone |

## Components map (concrete)

### Orca (L4) — what we use vs refuse

| Orca component | Ensembly mapping | Keep? |
|----------------|------------------|-------|
| Parallel git worktrees + multi-agent fan-out | Fleet parallel dogfood on mzapan; Steward/OpenCode already | **Keep** as ADE |
| Agent roster (OpenCode, Grok, Cursor, Pi, …) | L3 harness menu — Orca picks runner, kernel does not | **Keep** |
| Mobile companion / notifications | Operator away-from-desk steer (not a gate) | **Keep** (HITL still clears auth) |
| `orca` CLI (`worktree`, snapshot, click) | Scripted ADE; may feed evidence into pulse later | **Keep** as client tooling |
| Design Mode / computer-use | Capture evidence; still untrusted input | **Keep** with Manual rails |
| Embedding Orca *as* ensembly UI | Second process OS / Musk-cut theater | **Refuse** |

### Prime Agent (L3) — what we borrow vs refuse

| Prime component | Ensembly mapping | Keep? |
|-----------------|------------------|-------|
| Continual Harness `/refine` (evidence-backed supplemental state; base prompt immutable; rollback snapshots) | Pattern for **inspectable** preference/skill updates → life-os note or memory reflect — never silent weight edits | **Borrow pattern** |
| Daemon + attach/detach + heartbeats/schedules | Long jobs without chat amnesia; similar need to Grok ACP daemon | **Borrow** as worker ops |
| Bounded `/autonomous` + quality gates | HOOTL budgets; gate pass ≠ task success (honest) | **Borrow** |
| Family-tree A2A (parent/sibling/child only) | Safer than free-mesh swarms; MsgBus still records *applied* facts | **Borrow bound**; refuse orphan free-for-all |
| Persistent Python REPL as control tool | L3 implementation detail | Optional worker; **refuse** on kernel critical path |
| `~/.prime/agent/models.json` local models | Cost routing: thrash → local open-weight | **Keep** as worker config |
| Making prime-agent the ensembly runtime | Second chat OS; Python on control path | **Refuse** |
| Continual Harness writing ops SQLite | Dual writer / judgment-as-evidence | **Refuse** |

## Evidence → Knowledge → Action (with these two)

| Layer | Examples including Orca / Prime |
|-------|----------------------------------|
| **Evidence** | Orca worktree diffs, harness traces, prime session JSONL, mail, pulse archive |
| **Knowledge** | life-os cards; ensembly-memory CRDT; prime Continual Harness *only as session-local* until human promotes |
| **Action** | HITL approve/deny/claim/complete on kernel; CoS/Fleet start-stop; Orca only *starts* L3 workers |

**Law:** Context informs; it does not authorize. Orca starting five agents is not five approvals.

## Host topology (unchanged)

| Host | Role |
|------|------|
| **Grok Bot computer** | Canonical L1 writer (`ensembly-ops.sqlite`); pulse export |
| **mzapan** | L4 Orca + L3 workers (OpenCode / Grok Build / optional prime-agent); pulse import client; dogfood only |
| **Neither** | Dual-write ops; tunnel leader sockets as SoT |

## Integrate (local-only slices)

| Slice | Done when |
|-------|-----------|
| A. This note committed | Docs PR; Tools/harnesses: OpenCode/local (Steward) |
| B. Fleet WHERE one-liner: Orca = L4 ADE, not kernel | WHERE / playbook link |
| C. Optional mzapan smoke: `prime-agent` as *one more* L3 under Orca | Runs; no ensembly crate dep |
| D. Prefer promote path: harness refine → life-os note or `runtime reflect` | Human-visible; no silent ops write |

## Refuse (Odysseus)

- Orca or Prime as required runtime for `ensembly-kernel`
- Folding ADE or Continual Harness into root crates
- Harness memory authorizing gates
- Dual ops SQLite writers (laptop + bot)
- “Second chat OS” / Eve-style thickening at root

## One-line for Fleet / CoS

**Orca = L4 fleet ADE (already dogfooding). Prime Agent = optional L3 long-running harness peer to OpenCode/Grok Build. Ensembly stays L1 white hole under both.**
