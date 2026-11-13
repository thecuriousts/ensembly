//! evalbench E1: gap check (Python parity on well-formed input; Rust's schema is
//! stricter, see `check`), frozen synthetic task set, trace rows, first-pass score.
//! Pure: no I/O, no floats. The CLI crate owns files and processes.

pub mod answer;
pub mod check;
pub mod runner;
pub mod score;
pub mod taskset;
pub mod trace;

mod proofs;
