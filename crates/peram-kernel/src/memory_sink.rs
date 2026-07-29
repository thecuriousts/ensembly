//! MemorySink — bridge from the control plane (Runtime/MsgBus) to the
//! episodic learning layer (`peram-memory`).
//!
//! Law: the sink **records what actually happened** (applied bus messages,
//! tick reports, graph loads) so the swarm accumulates trajectory, skills,
//! and reflections across CLI invocations. It never decides: no approve,
//! deny, claim, or reprioritization passes through here. Aux, not SoT —
//! a sink failure must never fail a control operation (CLI warns loudly).

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use peram_memory::{CoherenceConfig, EpisodicMemory, MemoryError, Reflection, TrajectoryType};
use serde_json::json;

use crate::msg_bus::{BusMessage, ManualCmd};
use crate::runtime::TickReport;

/// Durable memory lives next to the T1 ops DB (gitignored, local-only).
pub const DEFAULT_MEMORY_PATH: &str = "data/local/peram-memory.json";
/// Identity of the swarm's shared episodic document. Survives reloads;
/// peers (P2P later) namespace their own replicas.
pub const DEFAULT_AGENT_ID: &str = "peram-swarm";

pub struct MemorySink {
    pub memory: EpisodicMemory,
}

/// Trajectory content for a bus message, built *before* apply so the runtime
/// can append only on success. Manual commands and agent reports are Actions
/// (they drive skill-pattern detection); triggers are Observations.
pub fn memory_content_of(msg: &BusMessage) -> serde_json::Value {
    match msg {
        BusMessage::Manual { cmd, at } => {
            let (intent, target) = match cmd {
                ManualCmd::LoadGraph => ("load_graph".to_string(), None),
                ManualCmd::Recompute => ("recompute".to_string(), None),
                ManualCmd::Approve { id } => ("approve".to_string(), Some(id.clone())),
                ManualCmd::Deny { id } => ("deny".to_string(), Some(id.clone())),
                ManualCmd::ClaimPhysical { id } => ("claim_physical".to_string(), Some(id.clone())),
                ManualCmd::CompletePhysical { id } => {
                    ("complete_physical".to_string(), Some(id.clone()))
                }
            };
            json!({
                "action": {
                    "intent": intent,
                    "target": target,
                    "actor": "operator",
                },
                "result": { "applied": true },
                "at": at.to_rfc3339(),
            })
        }
        BusMessage::Trigger { trigger } => json!({
            "trigger": {
                "kind": format!("{:?}", trigger.kind),
                "task_id": trigger.task_id,
                "detail": trigger.detail,
            },
        }),
        BusMessage::Agent { report } => json!({
            "action": {
                "intent": format!("agent:{:?}", report.kind).to_lowercase(),
                "target": report.task_id,
                "actor": report.agent_id,
            },
            "result": { "applied": true, "kind": format!("{:?}", report.kind) },
        }),
    }
}

impl MemorySink {
    /// Open (or create) the durable memory at `path`.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, MemoryError> {
        Ok(Self {
            memory: EpisodicMemory::open(path, DEFAULT_AGENT_ID)?,
        })
    }

    fn append(&mut self, entry_type: TrajectoryType, content: serde_json::Value) {
        let coherence = peram_memory::compute_coherence(self.memory.doc(), &content);
        self.memory.append(entry_type, content, coherence);
    }

    /// Record an applied bus message. Caller guarantees the apply succeeded.
    pub fn record_applied(&mut self, content: serde_json::Value) {
        let entry_type = if content.get("action").is_some() {
            TrajectoryType::Action
        } else {
            TrajectoryType::Observation
        };
        self.append(entry_type, content);
    }

    pub fn record_tick(&mut self, report: &TickReport) {
        self.append(
            TrajectoryType::Observation,
            json!({
                "tick": {
                    "regime": format!("{:?}", report.regime),
                    "drained": report.messages_drained,
                    "triggers_emitted": report.triggers_emitted,
                    "hootl_claim": report.hootl_claim,
                    "hootl_complete": report.hootl_complete,
                    "next_auth": report.next_auth,
                    "next_physical": report.next_physical,
                    "next_hootl": report.next_hootl,
                }
            }),
        );
    }

    pub fn record_load(&mut self, nodes: usize, edges: usize, regime: &str, at: DateTime<Utc>) {
        self.append(
            TrajectoryType::Observation,
            json!({
                "load": {
                    "nodes": nodes,
                    "edges": edges,
                    "regime": regime,
                    "at": at.to_rfc3339(),
                }
            }),
        );
    }

    /// Explicit reflection pass. Uses `PERAM_INFERENCE` when set: deterministic
    /// always runs first; optional providers may enrich the summary, or warn
    /// and fall back (never fails the control path).
    pub fn reflect(&mut self) -> Option<Reflection> {
        let mut reflection =
            peram_memory::reflect(self.memory.doc_mut(), &CoherenceConfig::default())?;
        let backend = peram_agents::InferenceBackend::from_env();
        let provider = peram_agents::resolve_provider(backend);
        match provider.enrich_summary(self.memory.doc(), &reflection) {
            Ok(Some(summary)) => {
                reflection.summary = summary;
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!(
                    "INFERENCE_WARN provider={} — {e}",
                    provider.name()
                );
            }
        }
        Some(reflection)
    }

    /// Compact recent-trajectory hint for AgentWorker claim detail (read-only).
    /// Never influences claim eligibility — recall only, not control.
    pub fn recall_hint(&self, hours: i64, limit: usize) -> String {
        let recent = self.memory.doc().get_recent_trajectory(hours);
        let slice: Vec<_> = recent.iter().rev().take(limit).collect();
        if slice.is_empty() {
            return "recall: (empty trajectory)".into();
        }
        let parts: Vec<String> = slice
            .iter()
            .map(|e| {
                format!(
                    "{:?}@{}",
                    e.entry_type,
                    e.timestamp.format("%H:%M:%S")
                )
            })
            .collect();
        format!("recall: {}", parts.join(" · "))
    }

    /// Reconcile with whatever is on disk, then persist atomically.
    pub fn sync_and_save(&mut self) -> Result<bool, MemoryError> {
        self.memory.sync_and_save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg_bus::{AgentReport, AgentReportKind, Trigger, TriggerKind};

    #[test]
    fn manual_commands_map_to_action_intents() {
        let msg = BusMessage::Manual {
            cmd: ManualCmd::Approve { id: "pay-rent".into() },
            at: Utc::now(),
        };
        let content = memory_content_of(&msg);
        assert_eq!(
            content.pointer("/action/intent").and_then(|v| v.as_str()),
            Some("approve")
        );
        assert_eq!(
            content.pointer("/action/target").and_then(|v| v.as_str()),
            Some("pay-rent")
        );
    }

    #[test]
    fn triggers_map_to_observations() {
        let msg = BusMessage::Trigger {
            trigger: Trigger {
                kind: TriggerKind::AuthNeeded,
                task_id: Some("pay-rent".into()),
                detail: "auth-pay-rent".into(),
                at: Utc::now(),
            },
        };
        let content = memory_content_of(&msg);
        assert!(content.get("action").is_none());
        assert!(content.get("trigger").is_some());
    }

    #[test]
    fn agent_reports_map_to_actions() {
        let msg = BusMessage::Agent {
            report: AgentReport {
                agent_id: "swarm-digital-1".into(),
                task_id: "triage-inbox".into(),
                kind: AgentReportKind::Claimed,
                detail: "claimed".into(),
                at: Utc::now(),
            },
        };
        let content = memory_content_of(&msg);
        assert!(content
            .pointer("/action/intent")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .starts_with("agent:"));
    }

    /// Fusion proof: a full runtime cycle (load → HOOTL claim/complete →
    /// HITL approve) lands in durable episodic memory, survives reload, and
    /// feeds an explicit reflection. Memory is aux: control results are
    /// identical with the sink attached.
    #[test]
    fn runtime_cycle_records_trajectory_and_reflects() {
        use crate::approvals::list_pending;
        use crate::graph::TaskStatus;
        use crate::turn::Action;
        use peram_memory::{coherence_report, TrajectoryType};

        let actions = vec![
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
        ];

        let dir = tempfile::tempdir().unwrap();
        let mem_path = dir.path().join("peram-memory.json");

        let now = Utc::now();
        let mut rt = crate::runtime::Runtime::new(now);
        rt.memory = Some(MemorySink::open(&mem_path).unwrap());

        rt.load_actions(&actions, now).unwrap();
        let r1 = rt.tick(true, now).unwrap();
        let r2 = rt.tick(true, now).unwrap();
        rt.enqueue_manual(ManualCmd::Approve { id: "pay-rent".into() }, now);
        rt.tick(false, now).unwrap();

        // Control plane results unchanged by the attached sink.
        assert_eq!(r1.hootl_claim.as_deref(), Some("triage-inbox"));
        assert_eq!(r2.hootl_complete.as_deref(), Some("triage-inbox"));
        assert_eq!(rt.state.graph.nodes["pay-rent"].status, TaskStatus::Done);
        assert!(list_pending(&rt.snapshot)
            .iter()
            .all(|p| p.id != "auth-pay-rent"));

        let sink = rt.memory.as_mut().unwrap();
        sink.sync_and_save().unwrap();

        // Reload from disk: durable, honest trajectory of what happened.
        let mem = peram_memory::EpisodicMemory::open(&mem_path, "reader").unwrap();
        let doc = mem.doc();
        let entries: Vec<_> = doc.trajectory.values().collect();
        assert!(
            entries.iter().any(|e| e.content.get("load").is_some()),
            "graph load recorded"
        );
        assert!(
            entries.iter().any(|e| e.content.get("tick").is_some()),
            "tick reports recorded"
        );
        assert!(
            entries.iter().any(|e| {
                e.entry_type == TrajectoryType::Action
                    && e.content.pointer("/action/intent").and_then(|v| v.as_str()) == Some("approve")
                    && e.content.pointer("/action/target").and_then(|v| v.as_str()) == Some("pay-rent")
            }),
            "operator approve recorded as action"
        );
        assert!(
            entries.iter().any(|e| {
                e.entry_type == TrajectoryType::Action
                    && e.content
                        .pointer("/action/actor")
                        .and_then(|v| v.as_str())
                        == Some("swarm-digital-1")
            }),
            "HOOTL agent work recorded"
        );

        // Explicit reflection over the durable trajectory.
        let mut sink2 = MemorySink::open(&mem_path).unwrap();
        let reflection = sink2.reflect().expect("enough entries to reflect");
        assert!(reflection.coherence > 0.0);
        assert!(reflection.trajectory_length >= 5);
        sink2.sync_and_save().unwrap();

        let report = coherence_report(&peram_memory::EpisodicMemory::open(&mem_path, "r2").unwrap().doc());
        assert_eq!(report.history.len(), 1, "reflection recorded in history");
    }

    /// Failed applies record nothing: memory is a log of what happened,
    /// not what was attempted.
    #[test]
    fn failed_apply_records_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mem_path = dir.path().join("peram-memory.json");

        let now = Utc::now();
        let mut rt = crate::runtime::Runtime::new(now);
        rt.memory = Some(MemorySink::open(&mem_path).unwrap());
        // No graph loaded: approving an unknown id must fail closed.
        rt.enqueue_manual(ManualCmd::Approve { id: "ghost".into() }, now);
        let result = rt.tick(false, now);
        assert!(result.is_err());

        let entries = rt.memory.as_ref().unwrap().memory.doc().trajectory.len();
        assert_eq!(entries, 0, "failed apply must not reach memory");
    }

    #[test]
    fn recall_hint_lists_recent_types() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.json");
        let mut sink = MemorySink::open(&path).unwrap();
        sink.record_load(1, 0, "Hootl", Utc::now());
        let hint = sink.recall_hint(24, 3);
        assert!(hint.contains("recall:"));
        assert!(hint.contains("Observation"), "got {hint}");
    }
}
