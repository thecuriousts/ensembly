//! Versioned shapes for ledger rows, company audits, and pack refs.
//!
//! Regenerate the checked-in JSON Schema files with
//! `WRITE_SCHEMA=1 cargo test -p ensembly-schema --lib checked_in_schema_matches_types`.

pub const SCHEMA_VERSION: u32 = 1;

mod audit;
mod ledger;
mod pack;
mod wire;

pub use audit::*;
pub use ledger::*;
pub use pack::*;
pub use wire::{decode_line, encode_line, Decoded};

/// Optional check applied by [`decode_line`] after serde accepts the object.
pub trait Checked {
    fn check(&self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
