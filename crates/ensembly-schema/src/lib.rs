//! Versioned shapes for ledger rows, company audits, and pack refs.
//!
//! A missing `v` means the v0 shape: the same fields, with no version member.
//! `"v": 1.0` passes the JSON Schema const for [`SCHEMA_VERSION`] and the reader refuses it.
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
mod version_one {
    use schemars::schema::{Schema, SchemaObject};
    use schemars::{gen::SchemaGenerator, JsonSchema};

    /// Schema const for [`crate::SCHEMA_VERSION`]. Not a runtime value.
    ///
    /// `"v": 1.0` passes this const because JSON Schema compares numbers by
    /// value. [`crate::decode_line`] refuses it; only an integer
    /// [`crate::SCHEMA_VERSION`] is a supported `v`.
    pub struct VersionOne;

    impl JsonSchema for VersionOne {
        fn schema_name() -> String {
            "VersionOne".to_owned()
        }

        fn is_referenceable() -> bool {
            false
        }

        fn json_schema(_: &mut SchemaGenerator) -> Schema {
            SchemaObject {
                const_value: Some(serde_json::json!(crate::SCHEMA_VERSION)),
                ..Default::default()
            }
            .into()
        }
    }
}

#[cfg(test)]
mod tests;
