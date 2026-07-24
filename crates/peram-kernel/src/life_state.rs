//! Life-state space S — Runtime single source of truth for S + G (+ cached CP).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::critical_path::{compute_critical_path, CriticalPathReport};
use crate::graph::DepGraph;

/// HITL vs HOOTL regime at the control-plane level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopRegime {
    /// Swarm may clear digital thrash without human.
    Hootl,
    /// Waiting only for permission (auth) or physical presence.
    HitlWait,
}

/// Continuous evaluation metrics from Issue #1.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutcomeMetrics {
    pub correctness_events: u64,
    pub effectiveness_events: u64,
    pub efficiency_events: u64,
    pub hootl_completed: u64,
    pub hitl_surfaces: u64,
    pub agent_failures: u64,
}

impl OutcomeMetrics {
    pub fn record_success(&mut self, correctness: bool, effectiveness: bool, efficiency: bool) {
        if correctness {
            self.correctness_events += 1;
        }
        if effectiveness {
            self.effectiveness_events += 1;
        }
        if efficiency {
            self.efficiency_events += 1;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifeState {
    pub version: u64,
    pub graph: DepGraph,
    pub regime: LoopRegime,
    pub critical_path: Option<CriticalPathReport>,
    pub fingerprint: String,
    pub metrics: OutcomeMetrics,
    pub updated_at: DateTime<Utc>,
    /// Last MC sample count used.
    pub mc_samples: u32,
}

impl LifeState {
    pub fn empty(now: DateTime<Utc>) -> Self {
        Self {
            version: 1,
            graph: DepGraph::new(),
            regime: LoopRegime::Hootl,
            critical_path: None,
            fingerprint: String::new(),
            metrics: OutcomeMetrics::default(),
            updated_at: now,
            mc_samples: 128,
        }
    }

    pub fn with_graph(graph: DepGraph, now: DateTime<Utc>, mc_samples: u32) -> Result<Self, String> {
        let mut s = Self::empty(now);
        s.mc_samples = mc_samples;
        s.replace_graph(graph, now)?;
        Ok(s)
    }

    pub fn replace_graph(&mut self, graph: DepGraph, now: DateTime<Utc>) -> Result<(), String> {
        self.graph = graph;
        self.recompute(now)?;
        self.version += 1;
        self.updated_at = now;
        Ok(())
    }

    pub fn recompute(&mut self, now: DateTime<Utc>) -> Result<(), String> {
        let cp = compute_critical_path(&self.graph, self.mc_samples)?;
        self.fingerprint = self.graph.fingerprint();
        let needs_hitl = cp.path.iter().any(|id| {
            self.graph.nodes.get(id).map(|n| {
                matches!(
                    n.gate,
                    crate::graph::GateKind::Auth | crate::graph::GateKind::Physical
                ) && n.status != crate::graph::TaskStatus::Done
            })
            .unwrap_or(false)
        }) || self.graph.nodes.values().any(|n| {
            matches!(
                n.gate,
                crate::graph::GateKind::Auth | crate::graph::GateKind::Physical
            ) && matches!(
                n.status,
                crate::graph::TaskStatus::Open | crate::graph::TaskStatus::Claimed
            )
        });
        self.regime = if needs_hitl {
            LoopRegime::HitlWait
        } else {
            LoopRegime::Hootl
        };
        if needs_hitl {
            self.metrics.hitl_surfaces += 1;
        }
        self.critical_path = Some(cp);
        self.updated_at = now;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{DurationEstimate, GateKind, TaskNode, TaskRealm, TaskStatus};

    #[test]
    fn regime_hitl_when_auth_open() {
        let mut g = DepGraph::new();
        g.upsert_node(TaskNode {
            id: "pay".into(),
            title: "pay".into(),
            realm: TaskRealm::Digital,
            status: TaskStatus::Open,
            gate: GateKind::Auth,
            duration: DurationEstimate::minutes(30.0),
            urgency: 5,
            importance: 5,
            area: None,
            kind: None,
            depends_on: vec![],
            claimed_by: None,
            deadline_at: None,
        });
        let s = LifeState::with_graph(g, Utc::now(), 16).unwrap();
        assert_eq!(s.regime, LoopRegime::HitlWait);
        assert!(s.critical_path.is_some());
    }
}
