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
use crate::msg_bus::{AgentReportKind, BusMessage, ManualCmd, MsgBus, TriggerKind};
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

/// Strip optional `auth-` prefix so all surfaces speak action ids (e.g. `pay-rent`).
pub fn action_id_of(id: &str) -> &str {
    id.strip_prefix("auth-").unwrap_or(id)
}

/// One tick outcome — loud, inspectable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickReport {
    pub regime: LoopRegime,
    pub cp_explain: String,
    /// Real count of triggers pushed this tick's emit (not messages drained).
    pub triggers_emitted: usize,
    pub messages_drained: usize,
    /// Task claimed this tick (HOOTL). Complete is a separate tick action.
    pub hootl_claim: Option<String>,
    /// Task completed this tick (HOOTL), if any.
    pub hootl_complete: Option<String>,
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
    prev_auth: Option<String>,
    prev_physical: Option<String>,
    prev_hootl: Option<String>,
    last_triggers_emitted: usize,
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
            prev_auth: None,
            prev_physical: None,
            prev_hootl: None,
            last_triggers_emitted: 0,
        }
    }

    pub fn load_actions(
        &mut self,
        actions: &[Action],
        now: chrono::DateTime<Utc>,
    ) -> Result<(), RuntimeError> {
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
            self.last_triggers_emitted = 0;
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
            prev_auth: self.prev_auth.as_deref(),
            prev_physical: self.prev_physical.as_deref(),
            prev_hootl: self.prev_hootl.as_deref(),
            now,
            deadline_horizon: Duration::hours(48),
        };
        let triggers = derive_triggers(&self.state.graph, &cp, &ctx);
        let n = triggers.len();
        for t in triggers {
            self.bus.push(BusMessage::Trigger { trigger: t });
        }
        self.last_triggers_emitted = n;
        self.prev_fp = self.state.fingerprint.clone();
        self.prev_cp_path = cp.path.clone();
        self.prev_auth = next_auth_gate(&self.state.graph, &cp);
        self.prev_physical = next_physical_beacon(&self.state.graph, &cp);
        self.prev_hootl = next_hootl_digital(&self.state.graph, &cp);
    }

    /// Drain bus + optional HOOTL agent step (one claim **or** one complete per tick).
    pub fn tick(
        &mut self,
        auto_agent: bool,
        now: chrono::DateTime<Utc>,
    ) -> Result<TickReport, RuntimeError> {
        let mut drained = 0;
        let mut triggers_this_tick = 0;
        while let Some(msg) = self.bus.pop() {
            drained += 1;
            self.apply_message(msg, now)?;
        }

        let mut hootl_claim = None;
        let mut hootl_complete = None;
        if auto_agent {
            if let Some(cp) = self.state.critical_path.clone() {
                // Prefer complete of owned Claimed HOOTL digital (separate tick from claim).
                let owned_claimed: Option<String> = self
                    .state
                    .graph
                    .nodes
                    .values()
                    .find(|n| {
                        n.claimed_by.as_deref() == Some(self.agent.id.as_str())
                            && n.status == TaskStatus::Claimed
                            && n.realm == crate::graph::TaskRealm::Digital
                            && n.gate == GateKind::None
                    })
                    .map(|n| n.id.clone());

                let step = if let Some(id) = owned_claimed {
                    match self.agent.complete(&mut self.state.graph, &id, now) {
                        Ok(report) => {
                            hootl_complete = Some(report.task_id.clone());
                            self.state.metrics.hootl_completed += 1;
                            self.bus.push(BusMessage::Agent { report });
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    match self.agent.claim_next(&mut self.state.graph, &cp, now) {
                        Ok(report) => {
                            hootl_claim = Some(report.task_id.clone());
                            self.bus.push(BusMessage::Agent { report });
                            Ok(())
                        }
                        Err(AgentError::NoWork) => Ok(()),
                        // Blocked preds / not ready: quiet NoWork — not agent_failures.
                        Err(AgentError::NotClaimable(_)) => Ok(()),
                        Err(e) => Err(e),
                    }
                };

                match step {
                    Ok(()) => {
                        if hootl_claim.is_some() || hootl_complete.is_some() {
                            self.state.recompute(now).map_err(RuntimeError::Msg)?;
                            self.emit_triggers(now);
                            triggers_this_tick += self.last_triggers_emitted;
                            while let Some(msg) = self.bus.pop() {
                                drained += 1;
                                self.apply_message(msg, now)?;
                            }
                        }
                    }
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
            triggers_emitted: triggers_this_tick,
            messages_drained: drained,
            hootl_claim,
            hootl_complete,
            next_auth,
            next_physical,
            next_hootl,
            metrics: self.state.metrics.clone(),
            bus_dropped_triggers: self.bus.dropped_triggers,
        })
    }

    fn apply_message(
        &mut self,
        msg: BusMessage,
        now: chrono::DateTime<Utc>,
    ) -> Result<(), RuntimeError> {
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
                    AgentReportKind::Failed => {
                        self.state.metrics.agent_failures += 1;
                    }
                    _ => {}
                }
                Ok(())
            }
        }
    }

    fn apply_manual(
        &mut self,
        cmd: ManualCmd,
        now: chrono::DateTime<Utc>,
    ) -> Result<(), RuntimeError> {
        match cmd {
            ManualCmd::LoadGraph | ManualCmd::Recompute => {
                self.state.recompute(now).map_err(RuntimeError::Msg)?;
                self.emit_triggers(now);
                Ok(())
            }
            ManualCmd::Approve { id } => {
                // Snapshot first — fail closed before graph mutation (logical commit).
                let action_id = action_id_of(&id).to_string();
                let snap_next =
                    apply_decision(&self.snapshot, &action_id, "approve", "operator", now)?;
                if let Some(n) = self.state.graph.nodes.get_mut(&action_id) {
                    if n.gate == GateKind::Auth {
                        n.status = TaskStatus::Done;
                    }
                }
                self.snapshot = snap_next;
                self.state.recompute(now).map_err(RuntimeError::Msg)?;
                self.emit_triggers(now);
                Ok(())
            }
            ManualCmd::Deny { id } => {
                let action_id = action_id_of(&id).to_string();
                let snap_next =
                    apply_decision(&self.snapshot, &action_id, "deny", "operator", now)?;
                if let Some(n) = self.state.graph.nodes.get_mut(&action_id) {
                    if n.gate == GateKind::Auth {
                        n.status = TaskStatus::Blocked;
                    }
                }
                self.snapshot = snap_next;
                self.state.recompute(now).map_err(RuntimeError::Msg)?;
                self.emit_triggers(now);
                Ok(())
            }
            ManualCmd::ClaimPhysical { id } => {
                let action_id = action_id_of(&id).to_string();
                let snap_next =
                    apply_physical_decision(&self.snapshot, &action_id, "claim", now)?;
                if let Some(n) = self.state.graph.nodes.get_mut(&action_id) {
                    n.status = TaskStatus::Claimed;
                    n.claimed_by = Some("human".into());
                }
                self.snapshot = snap_next;
                self.state.recompute(now).map_err(RuntimeError::Msg)?;
                Ok(())
            }
            ManualCmd::CompletePhysical { id } => {
                let action_id = action_id_of(&id).to_string();
                let snap_next =
                    apply_physical_decision(&self.snapshot, &action_id, "complete", now)?;
                if let Some(n) = self.state.graph.nodes.get_mut(&action_id) {
                    n.status = TaskStatus::Done;
                }
                self.snapshot = snap_next;
                self.state.recompute(now).map_err(RuntimeError::Msg)?;
                self.emit_triggers(now);
                Ok(())
            }
        }
    }

    /// FocusPlan driven by CP when graph is loaded (augments Eisenhower).
    /// Approve coaching always uses **action id** (e.g. `pay-rent`), never `auth-*`.
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
                plan.primary_auth = Some(FocusItem {
                    id: n.id.clone(), // action id — approve uses this
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
    use crate::approvals::list_pending;
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
        // Tick 1: claim only
        let r1 = rt.tick(true, now).unwrap();
        assert_eq!(r1.hootl_claim.as_deref(), Some("triage-inbox"));
        assert_eq!(
            rt.state.graph.nodes["triage-inbox"].status,
            TaskStatus::Claimed
        );
        // Tick 2: complete
        let r2 = rt.tick(true, now).unwrap();
        assert_eq!(r2.hootl_complete.as_deref(), Some("triage-inbox"));
        assert_eq!(rt.state.graph.nodes["triage-inbox"].status, TaskStatus::Done);
        assert_eq!(r2.next_auth.as_deref(), Some("pay-rent"));
        assert_eq!(rt.state.regime, LoopRegime::HitlWait);

        // focus_plan coaches action id, not auth-*
        let plan = rt.focus_plan(FocusPlan {
            version: 1,
            at: now,
            location_label: None,
            biome: "test".into(),
            primary_physical: None,
            primary_auth: None,
            primary_digital: None,
            places: vec![],
            coach_line: "test".into(),
            physical_count: 0,
            pending_count: 0,
        });
        assert_eq!(
            plan.primary_auth.as_ref().map(|f| f.id.as_str()),
            Some("pay-rent")
        );

        // approve action id clears graph + snapshot (auth-* key still internal)
        rt.enqueue_manual(
            ManualCmd::Approve {
                id: "pay-rent".into(),
            },
            now,
        );
        let r3 = rt.tick(false, now).unwrap();
        assert_eq!(rt.state.graph.nodes["pay-rent"].status, TaskStatus::Done);
        assert!(list_pending(&rt.snapshot)
            .iter()
            .all(|p| p.id != "auth-pay-rent" || p.status != crate::approvals::ApprovalStatus::Pending));
        assert_eq!(r3.next_physical.as_deref(), Some("grocery-errand"));
    }

    #[test]
    fn approve_strips_auth_prefix() {
        let now = Utc::now();
        let mut rt = Runtime::new(now);
        rt.load_actions(&sample_actions(), now).unwrap();
        // Clear digital thrash
        rt.tick(true, now).unwrap();
        rt.tick(true, now).unwrap();
        rt.enqueue_manual(
            ManualCmd::Approve {
                id: "auth-pay-rent".into(),
            },
            now,
        );
        rt.tick(false, now).unwrap();
        assert_eq!(rt.state.graph.nodes["pay-rent"].status, TaskStatus::Done);
    }

    #[test]
    fn hitl_surfaces_edge_triggered() {
        let now = Utc::now();
        let mut rt = Runtime::new(now);
        rt.load_actions(&sample_actions(), now).unwrap();
        let after_load = rt.state.metrics.hitl_surfaces;
        assert_eq!(after_load, 1, "enter HitlWait once on load");
        rt.state.recompute(now).unwrap();
        rt.state.recompute(now).unwrap();
        assert_eq!(
            rt.state.metrics.hitl_surfaces, after_load,
            "recompute must not inflate hitl_surfaces"
        );
    }
}
