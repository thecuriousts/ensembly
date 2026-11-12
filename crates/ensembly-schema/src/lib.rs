//! Versioned shapes for ledger rows, company audits, and pack refs.
//!
//! A missing `v` means the v0 shape: the same fields, with no version member.
//! `"v": 1.0` passes the JSON Schema const for [`SCHEMA_VERSION`] and the reader refuses it.
//!
//! Regenerate the checked-in JSON Schema files with
//! `WRITE_SCHEMA=1 cargo test -p ensembly-schema --lib checked_in_schema_matches_types`.

pub const SCHEMA_VERSION: u32 = 1;

mod apps_record;
mod audit;
mod commit;
mod ledger;
mod pack;
mod reconcile;
mod record;
mod replay;
mod surface;
mod sync;
mod view;
mod wire;

pub use apps_record::{
    compare_ledger, import_plan, map_ledger_status, map_ledger_status_name, stage_token,
    AppEventPayload, CompareMismatch, CompareReport, MappedStage,
};
pub use audit::*;
pub use commit::{Commit, CommitError, CommitFault, CommitPhase, CommitResult, UnitOfWork};
pub use ledger::*;
pub use pack::*;
pub use reconcile::{
    apply_conflict_action, is_terminal, merge_copy_into_owner, reconcile, Conflict, ConflictAction,
    ConflictValue, CopySnapshot, IllegalConflictTransition, OwnerSnapshot, ReconcileError,
};
pub use record::{
    transition, Aggregate, AggregateId, ConflictState, EventEnvelope, EventId, EventKind, Evidence,
    Hash, IdempotencyKey, IllegalTransition, RecordEventPayload, Source, Status as RecordStatus,
    StockholmTime, TextForm, Version,
};
pub use replay::{
    current_rows, parse_event_log, replay, replay_log_file, restore_drill, restore_log,
    MaterializedAggregate, ReplayError,
};
pub use surface::{
    commit_json, feed_json, recover_json, snapshot_json, stockholm_record_now, JsonPayload,
    RecordSurfaceError,
};
pub use sync::{expire_if_stale, merge_offline, MergeOutcome, SyncConfig, SyncError};
pub use view::{
    view_append, ChangeEvent, Fact, Feed, RecordView, Snapshot, ViewContract, ViewError,
};
pub use wire::{decode_line, encode_line, Decoded};

/// Optional check applied by [`decode_line`] after serde accepts the object.
pub trait Checked: Sized {
    /// Read-side repair of legacy shapes, applied by [`decode_line`] before [`Checked::check`].
    fn normalize(self) -> Self {
        self
    }
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
