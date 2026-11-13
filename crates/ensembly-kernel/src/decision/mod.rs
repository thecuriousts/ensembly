//! E3 decision model: scam gate, propose-and-wait, and eval metrics.
//!
//! The decider is not live. Proposals wait for an operator confirm or
//! override. Timeout writes `expired` and takes no action. Writes go
//! through [`crate::KernelWriter`] only.
//!
//! [`fit`] and [`pick_band`] are public so tests and tooling can call them,
//! but they fit on whatever rows they are given. Passing held-out (or val)
//! rows into either leaks the evaluation set into the weights and the
//! threshold band. The eval path ([`run_eval`]) already restricts fitting
//! to labelled train rows; do not call these entry points on a mixed split.

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
