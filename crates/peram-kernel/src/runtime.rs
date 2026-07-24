//! Runtime control plane — owns S+G, CP+P, triggers, MsgBus, HITL escalation.
//! Agents attach via typed messages only.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::agent::{AgentError, AgentWorker};
use crate::approvals::{
    apply_decision, apply_physical_decision, upsert_pending_from_actions, upsert_physical, Snapshot,
};
use crate::critical_path::{explain_node, next_auth_gate, next_hootl_digital, next_physical_beacon};
use crate::graph::{DepGraph, GateKind, TaskStatus};
use crate::life_state::{LifeState, LoopRegime};
use crate::msg_bus::{
    AgentReportKind, BusMessage, ManualCmd, MsgBus, TriggerKind,
};
use crate::trigger::{derive_triggers, TriggerContext};
use crate::turn::{Action, FocusItem, FocusPlan};

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Agent(#[from] AgentError),
    #[error(transparent)]
    Approval(#[from] crate::approvals::ApprovalError),
}

/// One tick outcome — loud, inspectable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickReport {
    pub regime: LoopRegime,
    pub cp_explain: String,
    pub triggers_emitted: usize,
    pub messages_drained: usize,
    pub hootl_claim: Option<String>,
    pub next_auth: Option<String>,
    pub next_physical: Option<String>,
    pub next_hootl: Option<String>,
    pub metrics: crate::life_state::OutcomeMetrics,
    pub bus_dropped_triggers: u64,
}

pub struct Runtime {
    pub state: LifeState,
    pub bus: MsgBus,
    pub snapshot: Snapshot,
    pub agent: AgentWorker,
    prev_fp: String,
    prev_cp_path: Vec<String>,
}

impl Runtime {
    pub fn new(now: chrono::DateTime<Utc>) -> Self {
        Self {
            state: LifeState::empty(now),
            bus: MsgBus::new(64),
            snapshot: Snapshot::empty(now),
            agent: AgentWorker::new("swarm-digital-1"),
            prev_fp: String::new(),
            prev_cp_path: vec![],
        }
    }

    pub fn load_actions(&mut self, actions: &[Action], now: chrono::DateTime<Utc>) -> Result<(), RuntimeError> {
        let graph = DepGraph::from_actions(actions, &Default::default())
            .map_err(|e| RuntimeError::Msg(e.to_string()))?;
        self.state
            .replace_graph(graph, now)
            .map_err(RuntimeError::Msg)?;
        // Sync HITL snapshot from gates.
        let hitl: Vec<_> = self
            .state
            .graph
            .nodes
            .values()
            .map(|n| {
                (
                    n.id.clone(),
                    n.title.clone(),
                    n.kind.clone().unwrap_or_else(|| "task".into()),
                    n.gate == GateKind::Auth,
                )
            })
            .collect();
        self.snapshot = upsert_pending_from_actions(&hitl, Some(self.snapshot.clone()), now);
        let physical: Vec<_> = self
            .state
            .graph
            .nodes
            .values()
            .filter(|n| n.gate == GateKind::Physical)
            .map(|n| (n.id.clone(), n.title.clone()))
            .collect();
        self.snapshot = upsert_physical(&physical, Some(self.snapshot.clone()), now);
        self.emit_triggers(now);
        Ok(())
    }

    pub fn enqueue_manual(&mut self, cmd: ManualCmd, now: chrono::DateTime<Utc>) {
        self.bus.push(BusMessage::Manual { cmd, at: now });
    }

    fn emit_triggers(&mut self, now: chrono::DateTime<Utc>) {
        let Some(cp) = self.state.critical_path.clone() else {
            return;
        };
        let ctx = TriggerContext {
            prev_fingerprint: if self.prev_fp.is_empty() {
                None
            } else {
                Some(self.prev_fp.as_str())
            },
            prev_cp_path: if self.prev_cp_path.is_empty() {
                None
            } else {
                Some(self.prev_cp_path.as_slice())
            },
            now,
            deadline_horizon: Duration::hours(48),
        };
        let triggers = derive_triggers(&self.state.graph, &cp, &ctx);
        for t in triggers {
            self.bus.push(BusMessage::Trigger { trigger: t });
        }
        self.prev_fp = self.state.fingerprint.clone();
        self.prev_cp_path = cp.path;
    }

    /// Drain bus + optional HOOTL agent claim/complete simulation for one open task.
    pub fn tick(&mut self, auto_agent: bool, now: chrono::DateTime<Utc>) -> Result<TickReport, RuntimeError> {
        let mut drained = 0;
        while let Some(msg) = self.bus.pop() {
            drained += 1;
            self.apply_message(msg, now)?;
        }

        let mut hootl_claim = None;
        if auto_agent {
            if let Some(cp) = self.state.critical_path.clone() {
                match self.agent.claim_next(&mut self.state.graph, &cp, now) {
                    Ok(report) => {
                        hootl_claim = Some(report.task_id.clone());
                        self.bus.push(BusMessage::Agent { report });
                        // Immediate complete for dry-run thrash clearance (loud via report).
                        if let Some(id) = hootl_claim.clone() {
                            let done = self.agent.complete(&mut self.state.graph, &id, now)?;
                            self.bus.push(BusMessage::Agent { report: done });
                            self.state.metrics.hootl_completed += 1;
                            self.state.metrics.record_success(true, true, true);
                        }
                        self.state.recompute(now).map_err(RuntimeError::Msg)?;
                        self.emit_triggers(now);
                        // Drain agent reports
                        while let Some(msg) = self.bus.pop() {
                            drained += 1;
                            self.apply_message(msg, now)?;
                        }
                    }
                    Err(AgentError::NoWork) => {}
                    Err(e) => {
                        self.state.metrics.agent_failures += 1;
                        return Err(e.into());
                    }
                }
            }
        }

        let cp = self.state.critical_path.clone();
        let next_auth = cp
            .as_ref()
            .and_then(|c| next_auth_gate(&self.state.graph, c));
        let next_physical = cp
            .as_ref()
            .and_then(|c| next_physical_beacon(&self.state.graph, c));
        let next_hootl = cp
            .as_ref()
            .and_then(|c| next_hootl_digital(&self.state.graph, c));

        Ok(TickReport {
            regime: self.state.regime,
            cp_explain: cp
                .as_ref()
                .map(|c| c.explain.clone())
                .unwrap_or_else(|| "no CP".into()),
            triggers_emitted: drained, // approximate after drain; callers use bus audit
            messages_drained: drained,
            hootl_claim,
            next_auth,
            next_physical,
            next_hootl,
            metrics: self.state.metrics.clone(),
            bus_dropped_triggers: self.bus.dropped_triggers,
        })
    }

    fn apply_message(&mut self, msg: BusMessage, now: chrono::DateTime<Utc>) -> Result<(), RuntimeError> {
        match msg {
            BusMessage::Manual { cmd, .. } => self.apply_manual(cmd, now),
            BusMessage::Trigger { trigger } => {
                // Triggers are declarative signals; AuthNeeded/Physical stay until human acts.
                if matches!(
                    trigger.kind,
                    TriggerKind::AuthNeeded | TriggerKind::PhysicalBeacon
                ) {
                    self.state.regime = LoopRegime::HitlWait;
                }
                Ok(())
            }
            BusMessage::Agent { report } => {
                match report.kind {
                    AgentReportKind::Completed => {
                        self.state.metrics.hootl_completed =
                            self.state.metrics.hootl_completed.max(1);
                    }
                    AgentReportKind::Failed => {
                        self.state.metrics.agent_failures += 1;
                    }
                    _ => {}
                }
                Ok(())
            }
        }
    }

    fn apply_manual(&mut self, cmd: ManualCmd, now: chrono::DateTime<Utc>) -> Result<(), RuntimeError> {
        match cmd {
            ManualCmd::LoadGraph | ManualCmd::Recompute => {
                self.state.recompute(now).map_err(RuntimeError::Msg)?;
                self.emit_triggers(now);
                Ok(())
            }
            ManualCmd::Approve { id } => {
                // Resolve graph auth node + approvals snapshot.
                if let Some(n) = self.state.graph.nodes.get_mut(&id) {
                    if n.gate == GateKind::Auth {
                        n.status = TaskStatus::Done;
                    }
                }
                // pending ids are auth-<action>
                let auth_id = if id.starts_with("auth-") {
                    id.clone()
                } else {
                    format!("auth-{id}")
                };
                self.snapshot = apply_decision(&self.snapshot, &auth_id, "approve", "operator", now)
                    .or_else(|_| apply_decision(&self.snapshot, &id, "approve", "operator", now))?;
                self.state.metrics.record_success(true, true, false);
                self.state.recompute(now).map_err(RuntimeError::Msg)?;
                self.emit_triggers(now);
                Ok(())
            }
            ManualCmd::Deny { id } => {
                let auth_id = if id.starts_with("auth-") {
                    id.clone()
                } else {
                    format!("auth-{id}")
                };
                self.snapshot = apply_decision(&self.snapshot, &auth_id, "deny", "operator", now)
                    .or_else(|_| apply_decision(&self.snapshot, &id, "deny", "operator", now))?;
                if let Some(n) = self.state.graph.nodes.get_mut(&id) {
                    n.status = TaskStatus::Blocked;
                }
                self.state.recompute(now).map_err(RuntimeError::Msg)?;
                self.emit_triggers(now);
                Ok(())
            }
            ManualCmd::ClaimPhysical { id } => {
                self.snapshot =
                    apply_physical_decision(&self.snapshot, &id, "claim", now)?;
                if let Some(n) = self.state.graph.nodes.get_mut(&id) {
                    n.status = TaskStatus::Claimed;
                    n.claimed_by = Some("human".into());
                }
                self.state.recompute(now).map_err(RuntimeError::Msg)?;
                Ok(())
            }
            ManualCmd::CompletePhysical { id } => {
                self.snapshot =
                    apply_physical_decision(&self.snapshot, &id, "complete", now)?;
                if let Some(n) = self.state.graph.nodes.get_mut(&id) {
                    n.status = TaskStatus::Done;
                }
                self.state.metrics.record_success(true, true, true);
                self.state.recompute(now).map_err(RuntimeError::Msg)?;
                self.emit_triggers(now);
                Ok(())
            }
        }
    }

    /// FocusPlan driven by CP when graph is loaded (augments Eisenhower).
    pub fn focus_plan(&self, base: FocusPlan) -> FocusPlan {
        let Some(cp) = &self.state.critical_path else {
            return base;
        };
        let mut plan = base;
        plan.coach_line = format!("{} · {}", plan.coach_line, cp.explain);

        if let Some(id) = next_physical_beacon(&self.state.graph, cp) {
            if let Some(n) = self.state.graph.nodes.get(&id) {
                plan.primary_physical = Some(FocusItem {
                    id: n.id.clone(),
                    title: n.title.clone(),
                    kind: "physical".into(),
                    reason: explain_node(cp, &id),
                });
            }
        }
        if let Some(id) = next_auth_gate(&self.state.graph, cp) {
            if let Some(n) = self.state.graph.nodes.get(&id) {
                let auth_id = format!("auth-{id}");
                plan.primary_auth = Some(FocusItem {
                    id: auth_id,
                    title: n.title.clone(),
                    kind: n.kind.clone().unwrap_or_else(|| "auth".into()),
                    reason: explain_node(cp, &id),
                });
            }
        }
        if let Some(id) = next_hootl_digital(&self.state.graph, cp) {
            if let Some(n) = self.state.graph.nodes.get(&id) {
                plan.primary_digital = Some(FocusItem {
                    id: n.id.clone(),
                    title: n.title.clone(),
                    kind: n.kind.clone().unwrap_or_else(|| "digital".into()),
                    reason: explain_node(cp, &id),
                });
            }
        }
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::turn::Action;

    fn sample_actions() -> Vec<Action> {
        vec![
            Action {
                id: "triage-inbox".into(),
                title: "Triage inbox".into(),
                area: Some("Craft".into()),
                kind: Some("chore".into()),
                realm: Some("digital".into()),
                urgency: 3,
                importance: 3,
                tags: vec!["digital".into()],
                public: Some(true),
                depends_on: None,
                deadline_at: None,
            },
            Action {
                id: "pay-rent".into(),
                title: "Pay rent".into(),
                area: Some("Finance".into()),
                kind: Some("finance_transfer".into()),
                realm: Some("digital".into()),
                urgency: 5,
                importance: 5,
                tags: vec!["finance".into()],
                public: Some(false),
                depends_on: Some(vec!["triage-inbox".into()]),
                deadline_at: None,
            },
            Action {
                id: "grocery-errand".into(),
                title: "Grocery".into(),
                area: Some("Health".into()),
                kind: Some("physical_errand".into()),
                realm: Some("physical".into()),
                urgency: 4,
                importance: 4,
                tags: vec!["physical".into()],
                public: Some(false),
                depends_on: Some(vec!["pay-rent".into()]),
                deadline_at: None,
            },
        ]
    }

    #[test]
    fn hootl_clears_then_surfaces_auth() {
        let now = Utc::now();
        let mut rt = Runtime::new(now);
        rt.load_actions(&sample_actions(), now).unwrap();
        let report = rt.tick(true, now).unwrap();
        assert_eq!(report.hootl_claim.as_deref(), Some("triage-inbox"));
        assert_eq!(rt.state.graph.nodes["triage-inbox"].status, TaskStatus::Done);
        assert_eq!(report.next_auth.as_deref(), Some("pay-rent"));
        assert_eq!(rt.state.regime, LoopRegime::HitlWait);

        rt.enqueue_manual(
            ManualCmd::Approve {
                id: "pay-rent".into(),
            },
            now,
        );
        let r2 = rt.tick(false, now).unwrap();
        assert_eq!(rt.state.graph.nodes["pay-rent"].status, TaskStatus::Done);
        assert_eq!(r2.next_physical.as_deref(), Some("grocery-errand"));
    }
}
