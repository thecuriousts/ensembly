//! E3 decision model: scam gate, propose-and-wait, and eval metrics.
//!
//! The decider is not live. Proposals wait for an operator confirm or
//! override. Timeout writes `expired` and takes no action. Writes go
//! through [`crate::KernelWriter`] only.

pub mod eval;
pub mod gate;
pub mod metrics;
pub mod model;
pub mod store;
pub mod workflow;

pub use eval::{load_labels, run_eval, EvalReport};
pub use gate::{audit, FlagList, GateResult, GATE_REASON};
pub use metrics::{answered_metrics, wilson, AnsweredMetrics, WilsonInterval};
pub use model::{baseline, features, fit, pick_band, score, Band, FittedModel, Score};
pub use store::load_events;
pub use workflow::{apply, phase, silence_took_action, Phase, Signal, TransitionError};
