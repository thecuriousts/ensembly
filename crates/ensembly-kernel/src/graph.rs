//! Directed dependency graph G = (V, E) — coordination substrate for Issue #1.
//! Agents claim / request only through G (+ CP), never free-form chatter.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use thiserror::Error;

use crate::turn::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskRealm {
    Digital,
    Physical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Claimed,
    Done,
    Blocked,
}

/// Escalation surface — WaitOnlyForPermission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    None,
    /// High-stakes digital authorize (HITL AuthGate).
    Auth,
    /// Body-world pickup (HITL PhysicalBeacon).
    Physical,
}

/// Optimistic / most-likely / pessimistic duration (minutes) for PERT.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DurationEstimate {
    pub optimistic: f64,
    pub likely: f64,
    pub pessimistic: f64,
}

impl DurationEstimate {
    pub fn minutes(likely: f64) -> Self {
        let o = (likely * 0.7).max(1.0);
        let p = (likely * 1.6).max(o + 1.0);
        Self {
            optimistic: o,
            likely,
            pessimistic: p,
        }
    }

    /// Classical PERT expected duration.
    pub fn expected(&self) -> f64 {
        (self.optimistic + 4.0 * self.likely + self.pessimistic) / 6.0
    }

    /// Classical PERT variance σ² = ((p − o) / 6)².
    pub fn variance(&self) -> f64 {
        let span = (self.pessimistic - self.optimistic) / 6.0;
        span * span
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: String,
    pub title: String,
    pub realm: TaskRealm,
    pub status: TaskStatus,
    pub gate: GateKind,
    pub duration: DurationEstimate,
    pub urgency: i32,
    pub importance: i32,
    pub area: Option<String>,
    pub kind: Option<String>,
    /// Explicit predecessor ids (edges into this node).
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DepEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GraphError {
    #[error("unknown node: {0}")]
    UnknownNode(String),
    #[error("cycle detected involving: {0}")]
    Cycle(String),
    #[error("duplicate node: {0}")]
    Duplicate(String),
}

/// Directed dependency graph — single coordination SoT fragment.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DepGraph {
    pub nodes: HashMap<String, TaskNode>,
    pub edges: Vec<DepEdge>,
}

impl DepGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert_node(&mut self, node: TaskNode) {
        let id = node.id.clone();
        for pred in &node.depends_on {
            let edge = DepEdge {
                from: pred.clone(),
                to: id.clone(),
            };
            if !self.edges.iter().any(|e| e == &edge) {
                self.edges.push(edge);
            }
        }
        self.nodes.insert(id, node);
    }

    pub fn add_edge(&mut self, from: &str, to: &str) -> Result<(), GraphError> {
        if !self.nodes.contains_key(from) {
            return Err(GraphError::UnknownNode(from.into()));
        }
        if !self.nodes.contains_key(to) {
            return Err(GraphError::UnknownNode(to.into()));
        }
        let edge = DepEdge {
            from: from.into(),
            to: to.into(),
        };
        if !self.edges.iter().any(|e| e == &edge) {
            self.edges.push(edge);
            if let Some(n) = self.nodes.get_mut(to) {
                if !n.depends_on.iter().any(|d| d == from) {
                    n.depends_on.push(from.into());
                }
            }
        }
        Ok(())
    }

    pub fn predecessors(&self, id: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|e| e.to == id)
            .map(|e| e.from.as_str())
            .collect()
    }

    pub fn successors(&self, id: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|e| e.from == id)
            .map(|e| e.to.as_str())
            .collect()
    }

    /// Kahn topological order; Err on cycle.
    pub fn topo_order(&self) -> Result<Vec<String>, GraphError> {
        let mut indeg: HashMap<String, usize> = self.nodes.keys().map(|k| (k.clone(), 0)).collect();
        for e in &self.edges {
            if !self.nodes.contains_key(&e.from) || !self.nodes.contains_key(&e.to) {
                continue;
            }
            *indeg.entry(e.to.clone()).or_insert(0) += 1;
        }
        let mut q: VecDeque<String> = indeg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(k, _)| k.clone())
            .collect();
        // Stable: sort queue for determinism
        let mut q_vec: Vec<_> = q.drain(..).collect();
        q_vec.sort();
        q = q_vec.into();

        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(u) = q.pop_front() {
            order.push(u.clone());
            let mut succs: Vec<_> = self.successors(&u).into_iter().map(str::to_string).collect();
            succs.sort();
            for v in succs {
                if let Some(d) = indeg.get_mut(&v) {
                    *d = d.saturating_sub(1);
                    if *d == 0 {
                        q.push_back(v);
                    }
                }
            }
            // keep queue sorted for deterministic Kahn
            let mut rest: Vec<_> = q.drain(..).collect();
            rest.sort();
            q = rest.into();
        }
        if order.len() != self.nodes.len() {
            let leftover = self
                .nodes
                .keys()
                .find(|k| !order.contains(k))
                .cloned()
                .unwrap_or_else(|| "unknown".into());
            return Err(GraphError::Cycle(leftover));
        }
        Ok(order)
    }

    pub fn open_ids(&self) -> HashSet<String> {
        self.nodes
            .iter()
            .filter(|(_, n)| n.status == TaskStatus::Open || n.status == TaskStatus::Claimed)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Build G from turn Actions + optional explicit depends_on map.
    pub fn from_actions(
        actions: &[Action],
        depends: &HashMap<String, Vec<String>>,
    ) -> Result<Self, GraphError> {
        let mut g = Self::new();
        for a in actions {
            let realm = match a.realm.as_deref() {
                Some("physical") => TaskRealm::Physical,
                _ => {
                    if a.kind.as_deref() == Some("physical_errand")
                        || a.kind.as_deref() == Some("outdoor")
                        || a.kind.as_deref() == Some("health_body")
                    {
                        TaskRealm::Physical
                    } else {
                        TaskRealm::Digital
                    }
                }
            };
            let gate = gate_for_action(a, realm);
            let likely = duration_heuristic(a);
            let deps = depends
                .get(&a.id)
                .cloned()
                .or_else(|| a.depends_on.clone())
                .unwrap_or_default();
            g.upsert_node(TaskNode {
                id: a.id.clone(),
                title: a.title.clone(),
                realm,
                status: TaskStatus::Open,
                gate,
                duration: DurationEstimate::minutes(likely),
                urgency: a.urgency,
                importance: a.importance,
                area: a.area.clone(),
                kind: a.kind.clone(),
                depends_on: deps,
                claimed_by: None,
                deadline_at: a.deadline_at,
            });
        }
        // Materialize edges from depends_on (skip missing preds — loud later via audit).
        let ids: Vec<_> = g.nodes.keys().cloned().collect();
        for id in ids {
            let preds = g.nodes.get(&id).map(|n| n.depends_on.clone()).unwrap_or_default();
            for p in preds {
                if g.nodes.contains_key(&p) {
                    g.add_edge(&p, &id)?;
                }
            }
        }
        g.topo_order()?; // fail loud on cycle
        Ok(g)
    }

    pub fn fingerprint(&self) -> String {
        let mut ids: Vec<_> = self.nodes.keys().cloned().collect();
        ids.sort();
        let mut edges = self.edges.clone();
        edges.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
        let status: Vec<_> = ids
            .iter()
            .map(|id| {
                let n = &self.nodes[id];
                format!("{id}:{:?}:{:?}", n.status, n.gate)
            })
            .collect();
        let edge_s: Vec<_> = edges.iter().map(|e| format!("{}->{}", e.from, e.to)).collect();
        format!("{}|{}", status.join(","), edge_s.join(","))
    }
}

fn gate_for_action(a: &Action, realm: TaskRealm) -> GateKind {
    if realm == TaskRealm::Physical {
        return GateKind::Physical;
    }
    // AuthGate = high-stakes digital mutate only. Finance *prep* chores stay HOOTL.
    let hitl = matches!(
        a.kind.as_deref(),
        Some("job_application_submit")
            | Some("finance_transfer")
            | Some("external_email_send")
            | Some("calendar_mutate")
            | Some("git_push_shared")
            | Some("publish_private_data")
            | Some("bill_pay")
    );
    if hitl {
        GateKind::Auth
    } else {
        GateKind::None
    }
}

fn duration_heuristic(a: &Action) -> f64 {
    // Higher urgency/importance → slightly longer expected effort (minutes).
    let base = 30.0;
    let boost = (a.urgency + a.importance) as f64 * 5.0;
    base + boost
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act(id: &str, realm: &str, kind: &str) -> Action {
        Action {
            id: id.into(),
            title: id.into(),
            area: None,
            kind: Some(kind.into()),
            realm: Some(realm.into()),
            urgency: 3,
            importance: 3,
            tags: vec![],
            public: Some(false),
            depends_on: None,
            deadline_at: None,
        }
    }

    #[test]
    fn topo_and_edge() {
        let mut deps = HashMap::new();
        deps.insert("b".into(), vec!["a".into()]);
        deps.insert("c".into(), vec!["b".into()]);
        let g = DepGraph::from_actions(
            &[
                act("a", "digital", "chore"),
                act("b", "digital", "chore"),
                act("c", "physical", "physical_errand"),
            ],
            &deps,
        )
        .unwrap();
        let order = g.topo_order().unwrap();
        assert!(order.iter().position(|x| x == "a").unwrap() < order.iter().position(|x| x == "b").unwrap());
        assert_eq!(g.nodes["c"].gate, GateKind::Physical);
        assert_eq!(g.nodes["a"].gate, GateKind::None);
    }

    #[test]
    fn cycle_is_loud() {
        let mut deps = HashMap::new();
        deps.insert("a".into(), vec!["b".into()]);
        deps.insert("b".into(), vec!["a".into()]);
        let err = DepGraph::from_actions(
            &[act("a", "digital", "chore"), act("b", "digital", "chore")],
            &deps,
        )
        .unwrap_err();
        assert!(matches!(err, GraphError::Cycle(_)));
    }

    #[test]
    fn auth_gate_for_finance_transfer_only() {
        let pay = act("pay", "digital", "finance_transfer");
        let prep = {
            let mut a = act("prep", "digital", "chore");
            a.area = Some("Finance".into());
            a
        };
        let g = DepGraph::from_actions(&[pay, prep], &HashMap::new()).unwrap();
        assert_eq!(g.nodes["pay"].gate, GateKind::Auth);
        assert_eq!(g.nodes["prep"].gate, GateKind::None);
    }
}
