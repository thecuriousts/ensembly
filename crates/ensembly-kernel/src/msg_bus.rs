//! Typed message bus — ManualCmd ∪ AutoTrigger ∪ AgentReport on one channel.
//! Runtime ⊥ Agents communicate exclusively through these messages.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManualCmd {
    /// Load / replace life graph from actions.
    LoadGraph,
    /// Approve auth gate by action id (or pending auth id).
    Approve { id: String },
    /// Deny auth gate.
    Deny { id: String },
    /// Claim physical beacon.
    ClaimPhysical { id: String },
    /// Complete physical beacon.
    CompletePhysical { id: String },
    /// Force recompute CP + triggers.
    Recompute,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    GraphChanged,
    CriticalPathChanged,
    DeadlineApproaching,
    AuthNeeded,
    PhysicalBeacon,
    HootlWorkAvailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Trigger {
    pub kind: TriggerKind,
    pub task_id: Option<String>,
    pub detail: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentReportKind {
    Claimed,
    Completed,
    Failed,
    Progress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentReport {
    pub agent_id: String,
    pub task_id: String,
    pub kind: AgentReportKind,
    pub detail: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BusMessage {
    Manual { cmd: ManualCmd, at: DateTime<Utc> },
    Trigger { trigger: Trigger },
    Agent { report: AgentReport },
}

/// Ordered in-memory bus with clear ownership (Runtime drains).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MsgBus {
    queue: VecDeque<BusMessage>,
    /// Cap for back-pressure (drop oldest AutoTrigger only if exceeded — never drop Manual).
    pub capacity: usize,
    pub dropped_triggers: u64,
}

impl MsgBus {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            capacity: capacity.max(1),
            dropped_triggers: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn push(&mut self, msg: BusMessage) {
        let is_manual = matches!(msg, BusMessage::Manual { .. });
        while self.queue.len() >= self.capacity && !is_manual {
            // Prefer dropping oldest triggers under pressure.
            if let Some(front) = self.queue.front() {
                if matches!(front, BusMessage::Trigger { .. }) {
                    self.queue.pop_front();
                    self.dropped_triggers += 1;
                    continue;
                }
            }
            break;
        }
        if self.queue.len() >= self.capacity && is_manual {
            // Still enqueue manual — expand slightly; loud via dropped count elsewhere.
            self.capacity += 1;
        }
        if self.queue.len() < self.capacity || is_manual {
            self.queue.push_back(msg);
        } else if matches!(msg, BusMessage::Trigger { .. }) {
            self.dropped_triggers += 1;
        } else {
            self.queue.push_back(msg);
        }
    }

    pub fn pop(&mut self) -> Option<BusMessage> {
        self.queue.pop_front()
    }

    pub fn peek_all(&self) -> Vec<&BusMessage> {
        self.queue.iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backpressure_drops_triggers_keeps_manual() {
        let mut bus = MsgBus::new(3);
        let now = Utc::now();
        for i in 0..5 {
            bus.push(BusMessage::Trigger {
                trigger: Trigger {
                    kind: TriggerKind::GraphChanged,
                    task_id: None,
                    detail: format!("t{i}"),
                    at: now,
                },
            });
        }
        bus.push(BusMessage::Manual {
            cmd: ManualCmd::Recompute,
            at: now,
        });
        assert!(bus.dropped_triggers > 0);
        assert!(bus
            .peek_all()
            .iter()
            .any(|m| matches!(m, BusMessage::Manual { .. })));
    }
}
