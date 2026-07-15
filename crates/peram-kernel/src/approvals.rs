//! Durable HITL wait snapshot + physical claim lifecycle.
//! Port of ensembly `src/approvals.js` core transitions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalStatus {
    Open,
    Claimed,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotStatus {
    IdleWaiting,
    Clear,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAuth {
    pub id: String,
    pub action_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub area: Option<String>,
    pub realm: Option<String>,
    pub status: ApprovalStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digital_flow_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalRow {
    pub id: String,
    pub title: String,
    pub area: Option<String>,
    pub status: PhysicalStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub decision: String,
    pub at: DateTime<Utc>,
    pub actor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub status: SnapshotStatus,
    pub phase: String,
    pub pending: Vec<PendingAuth>,
    pub physical: Vec<PhysicalRow>,
    pub history: Vec<HistoryEntry>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum ApprovalError {
    #[error("approval not found: {0}")]
    NotFound(String),
    #[error("approval already {0}")]
    AlreadyResolved(String),
    #[error("invalid decision: {0}")]
    InvalidDecision(String),
}

impl Snapshot {
    pub fn empty(now: DateTime<Utc>) -> Self {
        Self {
            version: 1,
            status: SnapshotStatus::Clear,
            phase: "CLEAR".into(),
            pending: vec![],
            physical: vec![],
            history: vec![],
            updated_at: now,
        }
    }
}

pub fn derive_status(pending: &[PendingAuth]) -> SnapshotStatus {
    let open: Vec<_> = pending
        .iter()
        .filter(|p| p.status == ApprovalStatus::Pending)
        .collect();
    if open.is_empty() {
        return SnapshotStatus::Clear;
    }
    let resolved = pending.iter().any(|p| {
        matches!(
            p.status,
            ApprovalStatus::Approved | ApprovalStatus::Denied
        )
    });
    if resolved {
        SnapshotStatus::Partial
    } else {
        SnapshotStatus::IdleWaiting
    }
}

fn phase_for(status: &SnapshotStatus) -> &'static str {
    match status {
        SnapshotStatus::IdleWaiting => "HITL_WAIT",
        SnapshotStatus::Clear => "CLEAR",
        SnapshotStatus::Partial => "PARTIAL",
    }
}

pub fn list_pending(snap: &Snapshot) -> Vec<&PendingAuth> {
    snap.pending
        .iter()
        .filter(|p| p.status == ApprovalStatus::Pending)
        .collect()
}

/// Upsert pending from HITL actions.
pub fn upsert_pending_from_actions(
    actions: &[(String, String, String, bool)], // id, title, kind, hitl
    existing: Option<Snapshot>,
    now: DateTime<Utc>,
) -> Snapshot {
    let mut snap = existing.unwrap_or_else(|| Snapshot::empty(now));
    for (id, title, kind, hitl) in actions {
        if !hitl {
            continue;
        }
        let key = format!("auth-{id}");
        if let Some(prev) = snap.pending.iter().find(|p| p.id == key) {
            if matches!(
                prev.status,
                ApprovalStatus::Approved | ApprovalStatus::Denied
            ) {
                continue;
            }
        }
        let created = snap
            .pending
            .iter()
            .find(|p| p.id == key)
            .map(|p| p.created_at)
            .unwrap_or(now);
        snap.pending.retain(|p| p.id != key);
        snap.pending.push(PendingAuth {
            id: key,
            action_id: Some(id.clone()),
            title: title.clone(),
            kind: kind.clone(),
            area: None,
            realm: None,
            status: ApprovalStatus::Pending,
            created_at: created,
            updated_at: now,
            reason: "HITL required".into(),
            place: None,
            digital_flow_id: None,
            flow_status: None,
        });
    }
    snap.status = derive_status(&snap.pending);
    snap.phase = phase_for(&snap.status).into();
    snap.updated_at = now;
    snap
}

pub fn apply_decision(
    snap: &Snapshot,
    approval_id: &str,
    decision: &str,
    actor: &str,
    now: DateTime<Utc>,
) -> Result<Snapshot, ApprovalError> {
    let mut next = snap.clone();
    let item = next
        .pending
        .iter_mut()
        .find(|p| p.id == approval_id || p.action_id.as_deref() == Some(approval_id))
        .ok_or_else(|| ApprovalError::NotFound(approval_id.into()))?;
    if item.status != ApprovalStatus::Pending {
        return Err(ApprovalError::AlreadyResolved(format!("{:?}", item.status)));
    }
    item.status = match decision {
        "approve" => ApprovalStatus::Approved,
        "deny" => ApprovalStatus::Denied,
        other => return Err(ApprovalError::InvalidDecision(other.into())),
    };
    item.updated_at = now;
    let status_str = match item.status {
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Denied => "denied",
        ApprovalStatus::Pending => "pending",
    };
    let id = item.id.clone();
    next.history.push(HistoryEntry {
        id,
        decision: status_str.into(),
        at: now,
        actor: actor.into(),
    });
    next.status = derive_status(&next.pending);
    next.phase = phase_for(&next.status).into();
    next.updated_at = now;
    Ok(next)
}

pub fn apply_physical_decision(
    snap: &Snapshot,
    action_id: &str,
    decision: &str,
    now: DateTime<Utc>,
) -> Result<Snapshot, ApprovalError> {
    let mut next = snap.clone();
    let idx = next.physical.iter().position(|p| p.id == action_id);
    if idx.is_none() {
        next.physical.push(PhysicalRow {
            id: action_id.into(),
            title: action_id.into(),
            area: None,
            status: PhysicalStatus::Open,
            created_at: now,
            updated_at: now,
        });
    }
    let item = next
        .physical
        .iter_mut()
        .find(|p| p.id == action_id)
        .unwrap();
    match decision {
        "claim" => item.status = PhysicalStatus::Claimed,
        "complete" => item.status = PhysicalStatus::Completed,
        "release" => item.status = PhysicalStatus::Open,
        other => return Err(ApprovalError::InvalidDecision(other.into())),
    }
    item.updated_at = now;
    next.updated_at = now;
    Ok(next)
}

pub fn upsert_physical(
    actions: &[(String, String)], // id, title physical only
    existing: Option<Snapshot>,
    now: DateTime<Utc>,
) -> Snapshot {
    let mut snap = existing.unwrap_or_else(|| Snapshot::empty(now));
    for (id, title) in actions {
        if let Some(prev) = snap.physical.iter().find(|p| p.id == *id) {
            if prev.status == PhysicalStatus::Completed {
                continue;
            }
        }
        let created = snap
            .physical
            .iter()
            .find(|p| p.id == *id)
            .map(|p| p.created_at)
            .unwrap_or(now);
        let status = snap
            .physical
            .iter()
            .find(|p| p.id == *id)
            .map(|p| p.status.clone())
            .unwrap_or(PhysicalStatus::Open);
        snap.physical.retain(|p| p.id != *id);
        snap.physical.push(PhysicalRow {
            id: id.clone(),
            title: title.clone(),
            area: None,
            status,
            created_at: created,
            updated_at: now,
        });
    }
    snap.updated_at = now;
    snap
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap()
    }

    #[test]
    fn approve_closes_pending() {
        let snap = upsert_pending_from_actions(
            &[("apply".into(), "Apply".into(), "job_application_submit".into(), true)],
            None,
            now(),
        );
        assert_eq!(list_pending(&snap).len(), 1);
        let next = apply_decision(&snap, "auth-apply", "approve", "operator", now()).unwrap();
        assert!(list_pending(&next).is_empty());
        assert_eq!(next.pending[0].status, ApprovalStatus::Approved);
    }

    #[test]
    fn claim_complete_physical() {
        let snap = upsert_physical(&[("grocery".into(), "Grocery".into())], None, now());
        let c = apply_physical_decision(&snap, "grocery", "claim", now()).unwrap();
        assert_eq!(c.physical[0].status, PhysicalStatus::Claimed);
        let d = apply_physical_decision(&c, "grocery", "complete", now()).unwrap();
        assert_eq!(d.physical[0].status, PhysicalStatus::Completed);
    }
}
