//! Declarative triggers ≔ Δ(G) ∪ Δ(CP) ∪ (deadline → 0) ∪ AuthNeeded.

use chrono::{DateTime, Duration, Utc};

use crate::critical_path::CriticalPathReport;
use crate::graph::DepGraph;
use crate::msg_bus::{Trigger, TriggerKind};

#[derive(Debug, Clone, Default)]
pub struct TriggerContext<'a> {
    pub prev_fingerprint: Option<&'a str>,
    pub prev_cp_path: Option<&'a [String]>,
    pub now: DateTime<Utc>,
    /// Surface deadline within this horizon.
    pub deadline_horizon: Duration,
}

/// Derive inspectable triggers from graph + CP deltas.
pub fn derive_triggers(
    graph: &DepGraph,
    cp: &CriticalPathReport,
    ctx: &TriggerContext<'_>,
) -> Vec<Trigger> {
    let mut out = Vec::new();
    let fp = graph.fingerprint();
    if ctx.prev_fingerprint.map(|p| p != fp).unwrap_or(true) {
        out.push(Trigger {
            kind: TriggerKind::GraphChanged,
            task_id: None,
            detail: format!("G fingerprint changed ({})", short(&fp)),
            at: ctx.now,
        });
    }
    let path_changed = match ctx.prev_cp_path {
        Some(prev) => prev != cp.path.as_slice(),
        None => !cp.path.is_empty(),
    };
    if path_changed {
        out.push(Trigger {
            kind: TriggerKind::CriticalPathChanged,
            task_id: cp.path.first().cloned(),
            detail: cp.explain.clone(),
            at: ctx.now,
        });
    }

    for n in graph.nodes.values() {
        if let Some(dl) = n.deadline_at {
            let until = dl - ctx.now;
            if until <= ctx.deadline_horizon && until >= Duration::zero() {
                out.push(Trigger {
                    kind: TriggerKind::DeadlineApproaching,
                    task_id: Some(n.id.clone()),
                    detail: format!("deadline in {}s", until.num_seconds()),
                    at: ctx.now,
                });
            }
        }
    }

    if let Some(id) = crate::critical_path::next_auth_gate(graph, cp) {
        out.push(Trigger {
            kind: TriggerKind::AuthNeeded,
            task_id: Some(id),
            detail: "authorization gate on or near critical path".into(),
            at: ctx.now,
        });
    }
    if let Some(id) = crate::critical_path::next_physical_beacon(graph, cp) {
        out.push(Trigger {
            kind: TriggerKind::PhysicalBeacon,
            task_id: Some(id),
            detail: "physical beacon requires human presence".into(),
            at: ctx.now,
        });
    }
    if let Some(id) = crate::critical_path::next_hootl_digital(graph, cp) {
        out.push(Trigger {
            kind: TriggerKind::HootlWorkAvailable,
            task_id: Some(id),
            detail: "HOOTL digital work claimable via CP".into(),
            at: ctx.now,
        });
    }
    out
}

fn short(s: &str) -> String {
    if s.len() <= 48 {
        s.to_string()
    } else {
        format!("{}…", &s[..48])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::critical_path::compute_critical_path;
    use crate::graph::{DepGraph, DurationEstimate, GateKind, TaskNode, TaskRealm, TaskStatus};

    #[test]
    fn emits_auth_and_hootl() {
        let mut g = DepGraph::new();
        g.upsert_node(TaskNode {
            id: "dig".into(),
            title: "chore".into(),
            realm: TaskRealm::Digital,
            status: TaskStatus::Open,
            gate: GateKind::None,
            duration: DurationEstimate::minutes(20.0),
            urgency: 2,
            importance: 2,
            area: None,
            kind: None,
            depends_on: vec![],
            claimed_by: None,
            deadline_at: None,
        });
        g.upsert_node(TaskNode {
            id: "auth".into(),
            title: "pay".into(),
            realm: TaskRealm::Digital,
            status: TaskStatus::Open,
            gate: GateKind::Auth,
            duration: DurationEstimate::minutes(15.0),
            urgency: 4,
            importance: 4,
            area: Some("Finance".into()),
            kind: Some("finance_transfer".into()),
            depends_on: vec!["dig".into()],
            claimed_by: None,
            deadline_at: None,
        });
        let cp = compute_critical_path(&g, 0).unwrap();
        let triggers = derive_triggers(
            &g,
            &cp,
            &TriggerContext {
                prev_fingerprint: None,
                prev_cp_path: None,
                now: Utc::now(),
                deadline_horizon: Duration::hours(24),
            },
        );
        assert!(triggers.iter().any(|t| t.kind == TriggerKind::AuthNeeded));
        assert!(triggers
            .iter()
            .any(|t| t.kind == TriggerKind::HootlWorkAvailable));
    }
}
