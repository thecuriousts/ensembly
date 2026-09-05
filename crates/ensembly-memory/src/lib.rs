//! ensembly-memory — episodic learning layer for the Game of Peram swarm.
//!
//! Fused from the IntelliArch prototype (local-first CRDT memory + coherence
//! engine), ported to ensembly's kernel discipline: synchronous, std-only IO,
//! durable engine state, deterministic merge.
//!
//! Boundary law: this crate **remembers and learns** — trajectory of what the
//! swarm did, skills synthesized from repeated patterns, goals proposed.
//! `ensembly-kernel` remains the control SoT (S/G/CP, approvals, gates). Nothing
//! here approves, denies, claims, or reprioritizes life work.

pub mod coherence;
pub mod crdt;
pub mod memory;

pub use coherence::{
    coherence_report, compute_coherence, detect_patterns, goal, propose_goals, reflect,
    semantic_similarity, synthesize_skill, trajectory_coherence, CoherenceConfig, CoherenceRecord,
    CoherenceReport, GoalProposal, Reflection, SkillPattern,
};
pub use crdt::{
    entry_id, ContextEntry, CrdtDocument, Goal, GoalStatus, GoalUpdate, Skill, TrajectoryEntry,
    TrajectoryType,
};
pub use memory::{EpisodicMemory, FilePersistence, MemoryError};

/// Crate banner for CLI / hosts.
pub fn memory_version() -> &'static str {
    "ensembly-memory 0.1.0 episodic crdt-trajectory skills goals coherence"
}
