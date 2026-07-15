//! peram-kernel — Game of Peram life control plane (Rust iron-peak).
//!
//! Node `src/*` is legacy dogfood. New product logic lands here.
//!
//! Layers: privacy · realm · approvals · digital_flow · turn/rank_now ·
//! T1 SQLite store · sealed backup · T2 vault seal bridge.

pub mod approvals;
pub mod backup;
pub mod digital_flow;
pub mod privacy;
pub mod realm;
pub mod store;
pub mod turn;
pub mod vault;

pub use approvals::{
    apply_decision, apply_physical_decision, derive_status, list_pending, upsert_pending_from_actions,
    upsert_physical, ApprovalStatus, Snapshot, SnapshotStatus,
};
pub use backup::{
    create_backup_pack, read_backup_pack, restore_apply, restore_dry_run, write_backup_pack,
    BackupPack, RestoreDryRunReport,
};
pub use digital_flow::{
    activate, decide, execute_dry_run, flow_to_approval_record, map_flow_status_to_approval,
    run_cycle, DigitalFlow, FlowStatus,
};
pub use privacy::{classify_item, private_path_patterns, Classifiable, Classification, Visibility};
pub use realm::{classify_realm, Realm};
pub use store::{OpsBundle, OpsStore};
pub use turn::{context_at, rank_now, select_next_auth, select_next_physical, Action, ContextFrame, FocusPlan, ScheduleSlot};
pub use vault::{export_denied_for_class, seal, unseal, SealedBlob, VAULT_SUITE};

/// Kernel banner for CLI / hosts.
pub fn kernel_version() -> &'static str {
    "peram-kernel 0.3.0 rust-life-control t1-sqlite t2-seal"
}
