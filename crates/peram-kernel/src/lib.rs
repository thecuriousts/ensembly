//! peram-kernel — Game of Peram life control plane (Rust iron-peak).
//!
//! Operator CLI (`bin/swarm.js` + `src/*`) is the live day/turn/graph host.
//! Control-plane features expand here.
//!
//! Layers: privacy · realm · approvals · digital_flow · turn/rank_now ·
//! life-state S · DepGraph G · CP+P · UncertaintyDive · MsgBus · Runtime/Agents ·
//! T1 SQLite store · sealed backup · T2 vault seal bridge · episodic memory sink.

pub mod agent;
pub mod approvals;
pub mod backup;
pub mod pulse_pack;
pub mod critical_path;
pub mod digital_flow;
pub mod graph;
pub mod life_state;
pub mod memory_sink;
pub mod msg_bus;
pub mod privacy;
pub mod realm;
pub mod runtime;
pub mod store;
pub mod trigger;
pub mod turn;
pub mod uncertainty_dive;
pub mod vault;

pub use approvals::{
    apply_decision, apply_physical_decision, derive_status, list_pending, upsert_pending_from_actions,
    upsert_physical, ApprovalStatus, Snapshot, SnapshotStatus,
};
pub use backup::{
    create_backup_pack, read_backup_pack, restore_apply, restore_dry_run, write_backup_pack,
    BackupPack, RestoreDryRunReport,
};
pub use pulse_pack::{
    export_pulse_pack, import_pulse_pack, local_pulse_status, read_pulse_pack, write_pulse_pack,
    ArchiveEvent, MemoryTrace, PulseExportOpts, PulseImportOpts, PulseImportReport, PulsePack,
    PulsePackStatus, DEFAULT_ARCHIVE_SIDECAR, PULSE_PACK_FORMAT,
};
pub use critical_path::{compute_critical_path, explain_node, CriticalPathReport};
pub use digital_flow::{
    activate, decide, execute_dry_run, flow_to_approval_record, map_flow_status_to_approval,
    run_cycle, DigitalFlow, FlowStatus,
};
pub use graph::{DepGraph, GateKind, TaskNode, TaskRealm, TaskStatus};
pub use life_state::{LifeState, LoopRegime, OutcomeMetrics};
pub use memory_sink::{MemorySink, DEFAULT_AGENT_ID, DEFAULT_MEMORY_PATH};
pub use msg_bus::{BusMessage, ManualCmd, MsgBus};
pub use privacy::{classify_item, private_path_patterns, Classifiable, Classification, Visibility};
pub use realm::{classify_realm, Realm};
pub use runtime::{Runtime, TickReport};
pub use store::{OpsBundle, OpsStore};
pub use turn::{
    context_at, rank_now, select_next_auth, select_next_physical, Action, ContextFrame, FocusItem,
    FocusPlan, ScheduleSlot,
};
pub use uncertainty_dive::{plan_dive, DiveReport, DEFAULT_PROBE_BUDGET, DIVE_IR_VERSION};
pub use vault::{export_denied_for_class, seal, unseal, SealedBlob, VAULT_SUITE};

/// Kernel banner for CLI / hosts.
pub fn kernel_version() -> &'static str {
    "peram-kernel 0.5.1 rust-life-control s+g+cp dive msgbus hitl-hootl t1-sqlite t2-seal episodic-memory"
}
