//! Digital-flow IR — bill_pay / Bank HITL + dry-run execute.
//! Port of ensembly `src/digital-flow.js` with closed-gate semantics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::approvals::{ApprovalStatus, PendingAuth};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowStatus {
    Idle,
    PendingAuth,
    Approved,
    Denied,
    Executed,
    DryRunOk,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    DryRun,
    Live,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowHistory {
    pub event: String,
    pub at: DateTime<Utc>,
    pub actor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutated: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowResult {
    pub ok: bool,
    pub mode: String,
    pub message: String,
    pub mutated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigitalFlow {
    pub version: u32,
    pub id: String,
    pub kind: String,
    pub place: String,
    pub title: String,
    pub amount_label: Option<String>,
    pub payee_label: Option<String>,
    pub area: String,
    pub hitl_kind: String,
    pub status: FlowStatus,
    pub execution_mode: ExecutionMode,
    pub auth_id: String,
    pub action_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub history: Vec<FlowHistory>,
    pub last_result: Option<FlowResult>,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum FlowError {
    #[error("cannot activate from status {0:?}")]
    BadActivate(FlowStatus),
    #[error("cannot decide from status {0:?}")]
    BadDecide(FlowStatus),
    #[error("cannot execute from status {0:?} (approve first)")]
    BadExecute(FlowStatus),
}

impl DigitalFlow {
    pub fn bill_pay(id: &str, payee: &str, now: DateTime<Utc>) -> Self {
        Self {
            version: 1,
            id: id.into(),
            kind: "bill_pay".into(),
            place: "Bank".into(),
            title: format!("Pay {payee} at Bank"),
            amount_label: None,
            payee_label: Some(payee.into()),
            area: "Finance".into(),
            hitl_kind: "finance_transfer".into(),
            status: FlowStatus::Idle,
            execution_mode: ExecutionMode::DryRun,
            auth_id: format!("auth-{id}"),
            action_id: id.into(),
            created_at: now,
            updated_at: now,
            history: vec![],
            last_result: None,
            reason: "HITL required before digital mutate".into(),
        }
    }
}

/// Map flow status → approvals IR (only pending_auth is open).
pub fn map_flow_status_to_approval(status: &FlowStatus) -> ApprovalStatus {
    match status {
        FlowStatus::PendingAuth => ApprovalStatus::Pending,
        FlowStatus::Denied => ApprovalStatus::Denied,
        FlowStatus::Approved
        | FlowStatus::DryRunOk
        | FlowStatus::Executed
        | FlowStatus::Failed
        | FlowStatus::Idle => ApprovalStatus::Approved,
    }
}

pub fn flow_to_approval_record(flow: &DigitalFlow) -> PendingAuth {
    PendingAuth {
        id: flow.auth_id.clone(),
        action_id: Some(flow.action_id.clone()),
        title: flow.title.clone(),
        kind: flow.hitl_kind.clone(),
        area: Some(flow.area.clone()),
        realm: Some("digital".into()),
        status: map_flow_status_to_approval(&flow.status),
        created_at: flow.created_at,
        updated_at: flow.updated_at,
        reason: flow.reason.clone(),
        place: Some(flow.place.clone()),
        digital_flow_id: Some(flow.id.clone()),
        flow_status: Some(format!("{:?}", flow.status).to_ascii_lowercase()),
    }
}

fn reactivatable(s: &FlowStatus) -> bool {
    matches!(
        s,
        FlowStatus::Idle
            | FlowStatus::Denied
            | FlowStatus::Failed
            | FlowStatus::DryRunOk
            | FlowStatus::Executed
            | FlowStatus::Approved
    )
}

pub fn activate(flow: &DigitalFlow, actor: &str, now: DateTime<Utc>) -> Result<DigitalFlow, FlowError> {
    if flow.status == FlowStatus::PendingAuth || !reactivatable(&flow.status) {
        return Err(FlowError::BadActivate(flow.status.clone()));
    }
    let mut next = flow.clone();
    next.status = FlowStatus::PendingAuth;
    next.updated_at = now;
    next.last_result = None;
    next.history.push(FlowHistory {
        event: "activate".into(),
        at: now,
        actor: actor.into(),
        mode: None,
        ok: None,
        mutated: None,
    });
    Ok(next)
}

pub fn decide(
    flow: &DigitalFlow,
    decision: &str,
    actor: &str,
    now: DateTime<Utc>,
) -> Result<DigitalFlow, FlowError> {
    if flow.status != FlowStatus::PendingAuth {
        return Err(FlowError::BadDecide(flow.status.clone()));
    }
    let mut next = flow.clone();
    next.status = match decision {
        "approve" => FlowStatus::Approved,
        "deny" => FlowStatus::Denied,
        _ => return Err(FlowError::BadDecide(flow.status.clone())),
    };
    next.updated_at = now;
    next.history.push(FlowHistory {
        event: decision.into(),
        at: now,
        actor: actor.into(),
        mode: None,
        ok: None,
        mutated: None,
    });
    Ok(next)
}

pub fn execute_dry_run(
    flow: &DigitalFlow,
    actor: &str,
    now: DateTime<Utc>,
) -> Result<DigitalFlow, FlowError> {
    if flow.status != FlowStatus::Approved {
        return Err(FlowError::BadExecute(flow.status.clone()));
    }
    let mut next = flow.clone();
    let payee = next.payee_label.clone().unwrap_or_else(|| "bill".into());
    let message = format!("dry-run: would pay {payee} at {}", next.place);
    next.status = FlowStatus::DryRunOk;
    next.last_result = Some(FlowResult {
        ok: true,
        mode: "dry_run".into(),
        message: message.clone(),
        mutated: false,
    });
    next.updated_at = now;
    next.history.push(FlowHistory {
        event: "execute".into(),
        at: now,
        actor: actor.into(),
        mode: Some("dry_run".into()),
        ok: Some(true),
        mutated: Some(false),
    });
    Ok(next)
}

/// Full cycle: activate → approve|deny → (execute if approve). Deny never executes.
pub fn run_cycle(
    flow: &DigitalFlow,
    decision: &str,
    actor: &str,
    now: DateTime<Utc>,
) -> Result<(DigitalFlow, PendingAuth, bool), FlowError> {
    let mut f = activate(flow, actor, now)?;
    f = decide(&f, decision, actor, now)?;
    if decision == "deny" {
        let approval = flow_to_approval_record(&f);
        return Ok((f, approval, false));
    }
    f = execute_dry_run(&f, actor, now)?;
    let approval = flow_to_approval_record(&f);
    Ok((f, approval, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap()
    }

    #[test]
    fn cycle_approve_closes_gate() {
        let flow = DigitalFlow::bill_pay("flow-bill_pay", "electric", now());
        let (f, approval, executed) = run_cycle(&flow, "approve", "operator", now()).unwrap();
        assert_eq!(f.status, FlowStatus::DryRunOk);
        assert!(executed);
        assert_eq!(approval.status, ApprovalStatus::Approved);
        assert_ne!(approval.status, ApprovalStatus::Pending);
        assert!(!f.last_result.as_ref().unwrap().mutated);
    }

    #[test]
    fn deny_never_executes() {
        let flow = DigitalFlow::bill_pay("flow-bill_pay", "rent", now());
        let (f, approval, executed) = run_cycle(&flow, "deny", "operator", now()).unwrap();
        assert_eq!(f.status, FlowStatus::Denied);
        assert!(!executed);
        assert_eq!(approval.status, ApprovalStatus::Denied);
        assert!(f.last_result.is_none());
    }

    #[test]
    fn reactivate_after_dry_run() {
        let flow = DigitalFlow::bill_pay("flow-bill_pay", "water", now());
        let (f, _, _) = run_cycle(&flow, "approve", "operator", now()).unwrap();
        let again = activate(&f, "operator", now()).unwrap();
        assert_eq!(again.status, FlowStatus::PendingAuth);
        let (f2, a2, _) = run_cycle(&f, "approve", "operator", now()).unwrap();
        assert_eq!(f2.status, FlowStatus::DryRunOk);
        assert_eq!(a2.status, ApprovalStatus::Approved);
    }
}
