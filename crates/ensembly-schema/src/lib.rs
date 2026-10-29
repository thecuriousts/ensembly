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
mod version_one {
    use schemars::schema::{Schema, SchemaObject};
    use schemars::{gen::SchemaGenerator, JsonSchema};

    /// Schema const for a version-1 line. Not a runtime value.
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
                const_value: Some(serde_json::json!(1)),
                ..Default::default()
            }
            .into()
        }
    }
}

#[cfg(test)]
mod tests;
