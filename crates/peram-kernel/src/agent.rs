//! Basic multi-agent workers — claim from G/CP only, report on MsgBus.
//! MultiAgentCtrl (Must): structured claim ≻ free-form chatter.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::critical_path::{next_hootl_digital, CriticalPathReport};
use crate::graph::{DepGraph, GateKind, TaskRealm, TaskStatus};
use crate::msg_bus::{AgentReport, AgentReportKind};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgentError {
    #[error("no HOOTL digital work on critical path")]
    NoWork,
    #[error("task not claimable: {0}")]
    NotClaimable(String),
    #[error("task not claimed by agent: {0}")]
    NotOwned(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentWorker {
    pub id: String,
}

impl AgentWorker {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// Claim next open digital HOOTL task on CP. Mutates graph.
    pub fn claim_next(
        &self,
        graph: &mut DepGraph,
        cp: &CriticalPathReport,
        now: DateTime<Utc>,
    ) -> Result<AgentReport, AgentError> {
        let Some(task_id) = next_hootl_digital(graph, cp) else {
            return Err(AgentError::NoWork);
        };
        self.claim(graph, cp, &task_id, now)
    }

    /// Claim a digital HOOTL task **only if it is on the critical path**.
    /// Law: agents claim via CP only — direct `claim` must not bypass `claim_next`.
    pub fn claim(
        &self,
        graph: &mut DepGraph,
        cp: &CriticalPathReport,
        task_id: &str,
        now: DateTime<Utc>,
    ) -> Result<AgentReport, AgentError> {
        let on_cp = cp.path.iter().any(|id| id == task_id)
            || cp
                .timings
                .iter()
                .any(|t| t.id == task_id && t.on_critical_path);
        if !on_cp {
            return Err(AgentError::NotClaimable(format!(
                "{task_id} not on critical path"
            )));
        }
        let node = graph
            .nodes
            .get_mut(task_id)
            .ok_or_else(|| AgentError::NotClaimable(task_id.into()))?;
        if node.realm != TaskRealm::Digital
            || node.gate != GateKind::None
            || node.status != TaskStatus::Open
        {
            return Err(AgentError::NotClaimable(task_id.into()));
        }
        // Predecessors must be done (coordination via G).
        let preds: Vec<_> = node.depends_on.clone();
        for p in &preds {
            let ok = graph
                .nodes
                .get(p)
                .map(|n| n.status == TaskStatus::Done)
                .unwrap_or(false);
            if !ok {
                return Err(AgentError::NotClaimable(format!(
                    "{task_id} blocked by {p}"
                )));
            }
        }
        let node = graph.nodes.get_mut(task_id).unwrap();
        node.status = TaskStatus::Claimed;
        node.claimed_by = Some(self.id.clone());
        Ok(AgentReport {
            agent_id: self.id.clone(),
            task_id: task_id.into(),
            kind: AgentReportKind::Claimed,
            detail: format!("claimed via CP by {}", self.id),
            at: now,
        })
    }

    /// Complete a claimed digital HOOTL task (simulates successful digital thrash clearance).
    pub fn complete(
        &self,
        graph: &mut DepGraph,
        task_id: &str,
        now: DateTime<Utc>,
    ) -> Result<AgentReport, AgentError> {
        let node = graph
            .nodes
            .get_mut(task_id)
            .ok_or_else(|| AgentError::NotOwned(task_id.into()))?;
        if node.claimed_by.as_deref() != Some(self.id.as_str())
            || node.status != TaskStatus::Claimed
        {
            return Err(AgentError::NotOwned(task_id.into()));
        }
        node.status = TaskStatus::Done;
        Ok(AgentReport {
            agent_id: self.id.clone(),
            task_id: task_id.into(),
            kind: AgentReportKind::Completed,
            detail: "digital task cleared (HOOTL)".into(),
            at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::critical_path::compute_critical_path;
    use crate::graph::{DurationEstimate, TaskNode};

    #[test]
    fn claim_and_complete_hootl_only() {
        let mut g = DepGraph::new();
        g.upsert_node(TaskNode {
            id: "a".into(),
            title: "inbox".into(),
            realm: TaskRealm::Digital,
            status: TaskStatus::Open,
            gate: GateKind::None,
            duration: DurationEstimate::minutes(10.0),
            urgency: 2,
            importance: 2,
            area: None,
            kind: Some("chore".into()),
            depends_on: vec![],
            claimed_by: None,
            deadline_at: None,
        });
        g.upsert_node(TaskNode {
            id: "b".into(),
            title: "wire".into(),
            realm: TaskRealm::Digital,
            status: TaskStatus::Open,
            gate: GateKind::Auth,
            duration: DurationEstimate::minutes(20.0),
            urgency: 5,
            importance: 5,
            area: Some("Finance".into()),
            kind: Some("finance_transfer".into()),
            depends_on: vec!["a".into()],
            claimed_by: None,
            deadline_at: None,
        });
        let cp = compute_critical_path(&g, 0).unwrap();
        let agent = AgentWorker::new("swarm-1");
        let now = Utc::now();
        let claim = agent.claim_next(&mut g, &cp, now).unwrap();
        assert_eq!(claim.task_id, "a");
        assert_eq!(g.nodes["a"].status, TaskStatus::Claimed);
        // Auth gate must not be claimable by agent.
        assert!(agent.claim(&mut g, &cp, "b", now).is_err());
        agent.complete(&mut g, "a", now).unwrap();
        assert_eq!(g.nodes["a"].status, TaskStatus::Done);
    }

    #[test]
    fn claim_rejects_off_critical_path() {
        // Long CP chain a→b vs short independent noise: open digital HOOTL off CP
        // must not be claimable via direct claim (Claim-via-CP law).
        let mut g = DepGraph::new();
        g.upsert_node(TaskNode {
            id: "a".into(),
            title: "a".into(),
            realm: TaskRealm::Digital,
            status: TaskStatus::Open,
            gate: GateKind::None,
            duration: DurationEstimate::minutes(60.0),
            urgency: 3,
            importance: 3,
            area: None,
            kind: None,
            depends_on: vec![],
            claimed_by: None,
            deadline_at: None,
        });
        g.upsert_node(TaskNode {
            id: "b".into(),
            title: "b".into(),
            realm: TaskRealm::Digital,
            status: TaskStatus::Open,
            gate: GateKind::None,
            duration: DurationEstimate::minutes(60.0),
            urgency: 3,
            importance: 3,
            area: None,
            kind: None,
            depends_on: vec!["a".into()],
            claimed_by: None,
            deadline_at: None,
        });
        g.upsert_node(TaskNode {
            id: "noise".into(),
            title: "noise".into(),
            realm: TaskRealm::Digital,
            status: TaskStatus::Open,
            gate: GateKind::None,
            duration: DurationEstimate::minutes(5.0),
            urgency: 1,
            importance: 1,
            area: None,
            kind: None,
            depends_on: vec![],
            claimed_by: None,
            deadline_at: None,
        });
        let cp = compute_critical_path(&g, 0).unwrap();
        assert!(
            cp.path.iter().any(|id| id == "a"),
            "expected a on CP path={:?}",
            cp.path
        );
        assert!(
            !cp.path.iter().any(|id| id == "noise"),
            "noise must be off CP path={:?}",
            cp.path
        );
        let noise_on = cp
            .timings
            .iter()
            .find(|t| t.id == "noise")
            .map(|t| t.on_critical_path)
            .unwrap_or(false);
        assert!(!noise_on, "noise must have slack (not on_critical_path)");
        let agent = AgentWorker::new("swarm-1");
        let now = Utc::now();
        let err = agent.claim(&mut g, &cp, "noise", now).unwrap_err();
        assert!(
            matches!(err, AgentError::NotClaimable(_)),
            "off-CP digital Open must refuse direct claim: {err:?}"
        );
        assert_eq!(g.nodes["noise"].status, TaskStatus::Open);
        assert!(agent.claim(&mut g, &cp, "a", now).is_ok());
        assert_eq!(g.nodes["a"].status, TaskStatus::Claimed);
    }
}
