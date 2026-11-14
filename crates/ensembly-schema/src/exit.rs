//! Process exit codes shared across kernel surfaces (pulse, generic CLI).

pub const EXIT_PASS: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_PULSE_REFUSED_WRITE: i32 = 7;
