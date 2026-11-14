//! Process exit codes shared across kernel surfaces (pulse, generic CLI).

pub const EXIT_PASS: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
/// Reserved by the opportunity-finder audit gate (pulse must not reuse).
pub const EXIT_FLAG: i32 = 3;
/// Reserved by the opportunity-finder audit gate (pulse must not reuse).
pub const EXIT_BLOCK: i32 = 4;
/// Reserved by the opportunity-finder audit gate (pulse must not reuse).
pub const EXIT_UNAUDITED_OR_STALE: i32 = 5;
pub const EXIT_PULSE_REFUSED_WRITE: i32 = 7;
