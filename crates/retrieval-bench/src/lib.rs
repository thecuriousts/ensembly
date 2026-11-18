//! Deterministic retrieval benchmark (slice 1a). Measures latency and recall only.

pub mod generate;
pub mod harness;

pub use harness::machine_label;
pub mod queries;
pub mod report;
pub mod schema;
pub mod score;

pub const DEFAULT_SEED: u64 = 0x511a_b00c;
