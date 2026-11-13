//! evalbench E1: gap check (Python parity except 3 documented stricter cases, see
//! `check`), frozen synthetic task set, trace rows, first-pass score. Pure: no I/O, no
//! floats. The CLI crate owns files and processes.
//!
//! E1 STATUS: types are declared; every function body is an `E1:` stub for the builder.

pub mod answer;
pub mod check;
pub mod runner;
pub mod score;
pub mod taskset;
pub mod trace;

mod proofs;
