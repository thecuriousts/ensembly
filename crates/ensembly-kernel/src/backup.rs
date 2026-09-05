//! Backup / restore-dry-run for T1 ops ledger (product path).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

use crate::store::{OpsBundle, OpsStore, StoreError};
use crate::vault::{seal, unseal, SealedBlob, VaultError};

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("store: {0}")]
    Store(#[from] StoreError),
    #[error("vault: {0}")]
    Vault(#[from] VaultError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("restore dry-run failed: {0}")]
    DryRun(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPack {
    pub format: String,
    pub created_at: DateTime<Utc>,
    /// Sealed ops bundle (AES-GCM). Without unlock key, unreadable.
    pub sealed_ops: SealedBlob,
    pub note: String,
}

impl BackupPack {
    pub const FORMAT: &'static str = "peram-backup-pack-v1";
}

/// Create encrypted backup pack from open ops store.
pub fn create_backup_pack(store: &OpsStore, unlock: &[u8]) -> Result<BackupPack, BackupError> {
    let bundle = store.export_bundle()?;
    let plain = serde_json::to_vec_pretty(&bundle)?;
    let sealed = seal(&plain, unlock)?;
    Ok(BackupPack {
        format: BackupPack::FORMAT.into(),
        created_at: Utc::now(),
        sealed_ops: sealed,
        note: "T1 ops snapshot; unlock required. T2 vault packs are separate (peram-vault).".into(),
    })
}

pub fn write_backup_pack(path: impl AsRef<Path>, pack: &BackupPack) -> Result<(), BackupError> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(pack)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn read_backup_pack(path: impl AsRef<Path>) -> Result<BackupPack, BackupError> {
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

/// Restore dry-run: unseal into temp store, verify snapshot loads, **do not** write primary.
pub fn restore_dry_run(pack: &BackupPack, unlock: &[u8]) -> Result<RestoreDryRunReport, BackupError> {
    let plain = unseal(&pack.sealed_ops, unlock)?;
    let bundle: OpsBundle = serde_json::from_slice(&plain)
        .map_err(|e| BackupError::DryRun(format!("bundle parse: {e}")))?;
    if bundle.format != "peram-ops-bundle-v1" {
        return Err(BackupError::DryRun(format!(
            "unexpected bundle format {}",
            bundle.format
        )));
    }
    let temp = OpsStore::open_in_memory()?;
    temp.import_bundle(&bundle)?;
    let snap = temp
        .load_snapshot()?
        .ok_or_else(|| BackupError::DryRun("no wait_snapshot in pack".into()))?;
    let pending_open = snap
        .pending
        .iter()
        .filter(|p| matches!(p.status, crate::approvals::ApprovalStatus::Pending))
        .count();
    Ok(RestoreDryRunReport {
        ok: true,
        schema_version: bundle.schema_version,
        kv_keys: bundle.kv.len(),
        snapshot_phase: snap.phase,
        open_pending: pending_open,
        message: "restore dry-run OK — primary store not modified".into(),
    })
}

/// Apply restore into a target store (destructive for those keys).
pub fn restore_apply(store: &OpsStore, pack: &BackupPack, unlock: &[u8]) -> Result<(), BackupError> {
    let plain = unseal(&pack.sealed_ops, unlock)?;
    let bundle: OpsBundle = serde_json::from_slice(&plain)?;
    store.import_bundle(&bundle)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreDryRunReport {
    pub ok: bool,
    pub schema_version: i64,
    pub kv_keys: usize,
    pub snapshot_phase: String,
    pub open_pending: usize,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approvals::{
        apply_decision, upsert_pending_from_actions, Snapshot,
    };
    use crate::digital_flow::{run_cycle, DigitalFlow};
    use chrono::TimeZone;

    #[test]
    fn backup_restore_dry_run_preserves_closed_bill_gate() {
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 14, 0, 0).unwrap();
        let store = OpsStore::open_in_memory().unwrap();
        let mut snap = upsert_pending_from_actions(
            &[(
                "apply".into(),
                "Apply".into(),
                "job_application_submit".into(),
                true,
            )],
            None,
            now,
        );
        let flow = DigitalFlow::bill_pay("flow-bill_pay", "electric", now);
        let (flow, approval, _) = run_cycle(&flow, "approve", "operator", now).unwrap();
        store.save_flow(&flow).unwrap();
        // merge approval into snap
        snap.pending.push(approval);
        snap.status = crate::approvals::derive_status(&snap.pending);
        store.save_snapshot(&snap).unwrap();

        let pack = create_backup_pack(&store, b"test-unlock").unwrap();
        // wrong key fails
        assert!(restore_dry_run(&pack, b"nope").is_err());
        let report = restore_dry_run(&pack, b"test-unlock").unwrap();
        assert!(report.ok);
        assert!(report.kv_keys >= 1);

        // apply to fresh store and verify bill auth not pending
        let dest = OpsStore::open_in_memory().unwrap();
        restore_apply(&dest, &pack, b"test-unlock").unwrap();
        let loaded = dest.load_snapshot().unwrap().unwrap();
        let bill = loaded
            .pending
            .iter()
            .find(|p| p.id == "auth-flow-bill_pay")
            .expect("bill auth row");
        assert!(matches!(
            bill.status,
            crate::approvals::ApprovalStatus::Approved
        ));
        let open: Vec<_> = crate::approvals::list_pending(&loaded);
        assert!(!open.iter().any(|p| p.id == "auth-flow-bill_pay"));
    }

    #[test]
    fn file_pack_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = OpsStore::open_in_memory().unwrap();
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        store.save_snapshot(&Snapshot::empty(now)).unwrap();
        let pack = create_backup_pack(&store, b"k").unwrap();
        let path = dir.path().join("backup.peram.json");
        write_backup_pack(&path, &pack).unwrap();
        let read = read_backup_pack(&path).unwrap();
        let report = restore_dry_run(&read, b"k").unwrap();
        assert!(report.ok);
    }

    #[test]
    fn approve_then_backup_consistency() {
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let store = OpsStore::open_in_memory().unwrap();
        let snap = upsert_pending_from_actions(
            &[(
                "x".into(),
                "X".into(),
                "job_application_submit".into(),
                true,
            )],
            None,
            now,
        );
        let snap = apply_decision(&snap, "auth-x", "approve", "op", now).unwrap();
        store.save_snapshot(&snap).unwrap();
        let pack = create_backup_pack(&store, b"k").unwrap();
        let r = restore_dry_run(&pack, b"k").unwrap();
        assert_eq!(r.open_pending, 0);
    }
}
