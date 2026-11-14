//! Versioned shapes for record events, pulse packs, and (optionally) job applications / decision model.
//!
//! Job-hunt ledger, audit, and E3 decision types need feature **`opportunity-finder`**.
//!
//! A missing `v` means the v0 shape: the same fields, with no version member.
//! `"v": 1.0` passes the JSON Schema const for [`SCHEMA_VERSION`] and the reader refuses it.
//!
//! Enable job-hunt ledger/audit/adapters with feature **`opportunity-finder`**.
//!
//! Regenerate the checked-in JSON Schema files with
//! `WRITE_SCHEMA=1 cargo test -p ensembly-schema --features opportunity-finder --lib checked_in_schema_matches_types`.

pub const SCHEMA_VERSION: u32 = 1;

mod commit;
#[cfg(feature = "opportunity-finder")]
mod decision;
mod exit;
#[cfg(feature = "opportunity-finder")]
mod pack;
mod reconcile;
mod record;
mod replay;
mod sealed_io;
mod surface;
mod sync;
mod url;
mod view;
mod wire;

#[cfg(feature = "opportunity-finder")]
mod apps_record;
#[cfg(feature = "opportunity-finder")]
mod audit;
#[cfg(feature = "opportunity-finder")]
mod ledger;

#[cfg(feature = "opportunity-finder")]
pub use apps_record::{
    closed_record_status, closed_stage, compare_ledger, import_plan, map_ledger_status,
    map_ledger_status_name, stage_token, AppEventPayload, CompareMismatch, CompareReport,
    MappedStage,
};
#[cfg(feature = "opportunity-finder")]
pub use audit::*;
pub use commit::{Commit, CommitError, CommitFault, CommitPhase, CommitResult, UnitOfWork};
#[cfg(feature = "opportunity-finder")]
pub use decision::{
    assign_split, proposal_rid, snapshot_hash, DecisionEvent, DecisionEventType, FitLabel,
    GateVerdict, LabelRow, ModelDecision, SplitBucket, DEFAULT_TIMEOUT_HOURS, MODEL_VERSION,
    SPLIT_SEED, TRIAGE_QUESTION,
};
pub use exit::{
    EXIT_BLOCK, EXIT_ERROR, EXIT_FLAG, EXIT_PASS, EXIT_PULSE_REFUSED_WRITE,
    EXIT_UNAUDITED_OR_STALE, EXIT_USAGE,
};
#[cfg(feature = "opportunity-finder")]
pub use ledger::*;
#[cfg(feature = "opportunity-finder")]
pub use pack::*;
pub use reconcile::{
    apply_conflict_action, is_terminal, merge_copy_into_owner, reconcile, Conflict, ConflictAction,
    ConflictValue, CopySnapshot, IllegalConflictTransition, OwnerSnapshot, ReconcileError,
};
pub use record::{
    transition, Aggregate, AggregateId, ConflictState, EventEnvelope, EventId, EventKind, Evidence,
    Hash, IdempotencyKey, IllegalTransition, RecordEventPayload, Rfc3339Time, Source,
    Status as RecordStatus, TextForm, Version,
};
pub use replay::{
    current_rows, parse_event_log, replay, replay_log_file, restore_drill, restore_log,
    MaterializedAggregate, ReplayError,
};
pub use sealed_io::{
    check_seal, grew_without_rewrite, lock, read_bytes, seal_path, unlock, write_seal,
    SealedIoError,
};
pub use surface::{
    commit_json, configured_writer, feed_json, record_now_local, recover_json,
    require_configured_writer, snapshot_json, stockholm_record_now, validate_writer_identity,
    JsonPayload, RecordSurfaceError, WRITER_ENV,
};
pub use sync::{expire_if_stale, merge_offline, MergeOutcome, SyncConfig, SyncError};
pub use url::{check_submit_time, link_key};
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
