# peram-memory

Episodic learning layer for the Game of Peram swarm: a durable, mergeable memory of **what actually happened** — trajectory, skills, goals — plus a coherence engine that reflects over it.

**Boundary law:** this crate remembers and learns. It never decides. `peram-kernel` remains the control Source of Truth (gates, critical path, priorities); nothing here approves, denies, claims, or reprioritizes. Terms expanded: [docs/GLOSSARY.md](../../docs/GLOSSARY.md).

## What is inside

| Module | Role |
|--------|------|
| `crdt` | Conflict-free Replicated Data Type document: trajectory / skills / goals / context with idempotent, commutative merge. Durable pattern counts + coherence history live *inside* the document, so learning survives across command-line invocations. |
| `memory` | `EpisodicMemory` + atomic file persistence (write-temp-then-rename) and `sync_and_save` (load-merge-persist) for concurrent writers. |
| `coherence` | Explicit `reflect`: word-set Jaccard coherence scoring, repeated-pattern skill synthesis, goal proposals. Deterministic — no model required. |

## Usage — as a library

```rust
use peram_memory::{EpisodicMemory, TrajectoryType, reflect, CoherenceConfig};

// Open (or create) a durable memory; identity survives reloads.
let mut mem = EpisodicMemory::open("data/local/peram-memory.json", "peram-swarm")?;

// Record what happened (the kernel's memory_sink does this for you).
mem.append(TrajectoryType::Action, serde_json::json!({
    "action": {"intent": "approve", "target": "pay-rent"},
    "result": {"applied": true}
}), 0.8);

// Explicit reflection: coherence + skill synthesis + goal proposals.
if let Some(r) = reflect(mem.doc_mut(), &CoherenceConfig::default()) {
    println!("{} — skills: {}", r.summary, r.known_skills);
}

// Reconcile with any concurrent writer, then persist atomically.
mem.sync_and_save()?;
```

## Usage — from the kernel CLI (normal path)

You do not drive this crate directly; the kernel records for you:

```bash
# Recorded automatically into data/local/peram-memory.json:
cargo run -p peram-kernel -- runtime load --fixture fixtures/issue-1-runtime.json
cargo run -p peram-kernel -- runtime tick

# Reflect over the accumulated trajectory (skips loudly below 5 entries):
cargo run -p peram-kernel -- runtime reflect          # human-readable
cargo run -p peram-kernel -- runtime reflect --json   # machine-readable
```

Global flags: `--memory <path>` (explicit path; open failure is fatal) · `--no-memory` (disable recording for one invocation).

## Semantics worth knowing

- **Applied-only recording:** the kernel appends a trajectory entry only after a bus message applies successfully — memory logs what happened, never what was attempted.
- **Merge law:** re-merging state you already hold is a true no-op (stable hash). Concurrent command-line writers converge via `sync_and_save`; no write is lost.
- **Aux failure semantics:** a memory save failure warns on stderr and never fails a committed control operation. An explicit `--memory` path that fails to open *is* fatal — you asked for it.
- **Privacy:** the default path lives under `data/local/` (gitignored, Tier-1 boundary). Memory contains real action ids/titles; redaction is required before any remote/export surface (roadmap P4).
- **Inference (roadmap P2):** `reflect` uses deterministic Jaccard by default. Optional `InferenceProvider` adapters (Ollama, Grok Model Context Protocol, opencode Agent Client Protocol, pi, …) are runtime-selected — never a hard dependency. See SN-8 in [coming-next.md](../../docs/arch-design/coming-next.md).

## Tests

```bash
cargo test -p peram-memory    # 9 tests: merge convergence/idempotence, persistence
                              # roundtrip, concurrent writers, reflection threshold,
                              # durable skill synthesis, report, similarity
```
