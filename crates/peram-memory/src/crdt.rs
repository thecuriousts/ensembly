//! CRDT document — trajectory / skills / goals / context with entry-level,
//! idempotent, commutative merge. This is the durable learning substrate:
//! concurrent writers (CLI invocations today, peers later) converge without
//! coordination. The kernel runtime remains the control SoT; this document
//! only *remembers*.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::coherence::CoherenceRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub entry_type: TrajectoryType,
    pub content: serde_json::Value,
    pub coherence: f32,
    pub agent_id: String,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrajectoryType {
    Observation,
    Action,
    Reflection,
    SkillSynthesis,
    GoalUpdate,
    ContextUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub pattern: String,
    pub sequence: Vec<serde_json::Value>,
    pub trigger: serde_json::Value,
    pub preconditions: Vec<serde_json::Value>,
    pub effects: Vec<serde_json::Value>,
    pub confidence: f32,
    pub usage_count: u64,
    pub last_used: DateTime<Utc>,
    pub version: u64,
    pub agent_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub description: String,
    pub priority: f32,
    pub goal_type: String,
    pub skill_name: Option<String>,
    pub status: GoalStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub agent_id: String,
    pub progress: f32,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Active,
    Completed,
    Failed,
    Paused,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub updated_at: DateTime<Utc>,
    pub agent_id: String,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoalUpdate {
    pub description: Option<String>,
    pub priority: Option<f32>,
    pub status: Option<GoalStatus>,
    pub progress: Option<f32>,
    pub skill_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrdtDocument {
    pub agent_id: String,
    pub trajectory: HashMap<String, TrajectoryEntry>,
    pub skills: HashMap<String, Skill>,
    pub goals: HashMap<String, Goal>,
    pub context: HashMap<String, ContextEntry>,
    /// Pattern-signature sighting counts (G-counter per key). Durable so skill
    /// synthesis accumulates across CLI invocations, not just within a process.
    #[serde(default)]
    pub pattern_counts: HashMap<String, u64>,
    /// Bounded coherence history (merge = union by id, keep newest 100).
    #[serde(default)]
    pub coherence_history: Vec<CoherenceRecord>,
    pub version: u64,
    pub last_updated: DateTime<Utc>,
    pub vector_clock: HashMap<String, u64>,
    #[serde(skip)]
    pub hash: String,
}

impl CrdtDocument {
    pub fn new(agent_id: impl Into<String>) -> Self {
        let agent_id = agent_id.into();
        let mut vector_clock = HashMap::new();
        vector_clock.insert(agent_id.clone(), 0);
        Self {
            agent_id,
            trajectory: HashMap::new(),
            skills: HashMap::new(),
            goals: HashMap::new(),
            context: HashMap::new(),
            pattern_counts: HashMap::new(),
            coherence_history: Vec::new(),
            version: 0,
            last_updated: Utc::now(),
            vector_clock,
            hash: String::new(),
        }
    }

    /// Entry-level merge: idempotent and commutative, so peers (or repeated
    /// CLI processes sharing one file) can re-exchange full states freely.
    /// No same-agent version guard — concurrent writers of one logical
    /// document would lose writes under a version shortcut. Merging state we
    /// already hold is a true no-op (no last_updated churn, stable hash).
    pub fn merge(&mut self, other: &CrdtDocument) {
        let prior_hash = self.hash.clone();
        for (id, entry) in &other.trajectory {
            match self.trajectory.get(id) {
                Some(existing) if existing.timestamp >= entry.timestamp => {}
                _ => {
                    self.trajectory.insert(id.clone(), entry.clone());
                }
            }
        }

        for (name, skill) in &other.skills {
            match self.skills.get(name) {
                Some(existing) if existing.version >= skill.version => {}
                _ => {
                    self.skills.insert(name.clone(), skill.clone());
                }
            }
        }

        for (id, goal) in &other.goals {
            match self.goals.get(id) {
                Some(existing) if existing.updated_at >= goal.updated_at => {}
                _ => {
                    self.goals.insert(id.clone(), goal.clone());
                }
            }
        }

        for (key, entry) in &other.context {
            match self.context.get(key) {
                Some(existing) if existing.version >= entry.version => {}
                _ => {
                    self.context.insert(key.clone(), entry.clone());
                }
            }
        }

        for (sig, count) in &other.pattern_counts {
            let current = self.pattern_counts.get(sig).copied().unwrap_or(0);
            if *count > current {
                self.pattern_counts.insert(sig.clone(), *count);
            }
        }

        let to_add: Vec<CoherenceRecord> = other
            .coherence_history
            .iter()
            .filter(|r| !self.coherence_history.iter().any(|k| k.id == r.id))
            .cloned()
            .collect();
        self.coherence_history.extend(to_add);
        self.coherence_history.sort_by_key(|r| r.timestamp);
        if self.coherence_history.len() > 100 {
            let drain = self.coherence_history.len() - 100;
            self.coherence_history.drain(0..drain);
        }

        for (agent, version) in &other.vector_clock {
            let current = self.vector_clock.get(agent).copied().unwrap_or(0);
            if *version > current {
                self.vector_clock.insert(agent.clone(), *version);
            }
        }

        self.recompute_hash();
        if self.hash != prior_hash {
            self.last_updated = Utc::now();
            self.recompute_hash();
        }
    }

    pub fn append_trajectory(&mut self, mut entry: TrajectoryEntry) -> String {
        self.increment_version();
        let seq = self.vector_clock.get(&self.agent_id).copied().unwrap_or(0);
        entry.id = entry_id("e", &self.agent_id, seq, &entry.content, entry.timestamp);
        entry.timestamp = if entry.timestamp > Utc::now() {
            entry.timestamp
        } else {
            Utc::now()
        };
        entry.agent_id = self.agent_id.clone();
        let id = entry.id.clone();
        self.trajectory.insert(id.clone(), entry);
        id
    }

    pub fn add_skill(&mut self, mut skill: Skill) {
        skill.agent_id = self.agent_id.clone();
        skill.created_at = Utc::now();
        skill.updated_at = Utc::now();
        self.skills.insert(skill.name.clone(), skill);
        self.increment_version();
    }

    pub fn update_skill(&mut self, name: &str, confidence: f32, usage_count: u64) -> bool {
        if let Some(skill) = self.skills.get_mut(name) {
            skill.confidence = confidence;
            skill.usage_count = usage_count;
            skill.last_used = Utc::now();
            skill.version += 1;
            skill.updated_at = Utc::now();
            self.increment_version();
            true
        } else {
            false
        }
    }

    pub fn add_goal(&mut self, mut goal: Goal) -> String {
        self.increment_version();
        let seq = self.vector_clock.get(&self.agent_id).copied().unwrap_or(0);
        goal.id = entry_id(
            "g",
            &self.agent_id,
            seq,
            &serde_json::json!(goal.description),
            Utc::now(),
        );
        goal.agent_id = self.agent_id.clone();
        goal.created_at = Utc::now();
        goal.updated_at = Utc::now();
        let id = goal.id.clone();
        self.goals.insert(id.clone(), goal);
        id
    }

    pub fn update_goal(&mut self, id: &str, updates: GoalUpdate) -> bool {
        if let Some(goal) = self.goals.get_mut(id) {
            if let Some(desc) = updates.description {
                goal.description = desc;
            }
            if let Some(priority) = updates.priority {
                goal.priority = priority;
            }
            if let Some(status) = updates.status {
                goal.status = status;
            }
            if let Some(progress) = updates.progress {
                goal.progress = progress;
            }
            if let Some(skill) = updates.skill_name {
                goal.skill_name = Some(skill);
            }
            goal.updated_at = Utc::now();
            self.increment_version();
            true
        } else {
            false
        }
    }

    pub fn set_context(&mut self, key: &str, value: serde_json::Value) {
        let version = self.context.get(key).map(|e| e.version + 1).unwrap_or(1);
        let entry = ContextEntry {
            key: key.to_string(),
            value,
            updated_at: Utc::now(),
            agent_id: self.agent_id.clone(),
            version,
        };
        self.context.insert(key.to_string(), entry);
        self.increment_version();
    }

    pub fn get_context(&self, key: &str) -> Option<&serde_json::Value> {
        self.context.get(key).map(|e| &e.value)
    }

    pub fn get_recent_trajectory(&self, hours: i64) -> Vec<&TrajectoryEntry> {
        let cutoff = Utc::now() - chrono::Duration::hours(hours);
        let mut out: Vec<_> = self
            .trajectory
            .values()
            .filter(|e| e.timestamp > cutoff)
            .collect();
        out.sort_by_key(|e| e.timestamp);
        out
    }

    pub fn get_active_goals(&self) -> Vec<&Goal> {
        self.goals
            .values()
            .filter(|g| g.status == GoalStatus::Active)
            .collect()
    }

    pub fn encode_state(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn decode_state(data: &[u8]) -> Result<Self, serde_json::Error> {
        let mut doc: Self = serde_json::from_slice(data)?;
        doc.recompute_hash();
        Ok(doc)
    }

    pub fn increment_version(&mut self) {
        self.version += 1;
        self.last_updated = Utc::now();
        *self.vector_clock.entry(self.agent_id.clone()).or_insert(0) += 1;
        self.recompute_hash();
    }

    fn recompute_hash(&mut self) {
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_vec(self).unwrap_or_default());
        self.hash = hex::encode(hasher.finalize());
    }
}

/// Deterministic, merge-stable entry id: same writer + same clock seq + same
/// content ⇒ same id, so re-appends after a crash dedupe on merge.
pub fn entry_id(
    prefix: &str,
    agent_id: &str,
    seq: u64,
    content: &serde_json::Value,
    at: DateTime<Utc>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(agent_id.as_bytes());
    hasher.update(seq.to_be_bytes());
    hasher.update(at.timestamp_micros().to_be_bytes());
    hasher.update(content.to_string().as_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("{prefix}_{}", &digest[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(content: serde_json::Value) -> TrajectoryEntry {
        TrajectoryEntry {
            id: String::new(),
            timestamp: Utc::now(),
            entry_type: TrajectoryType::Observation,
            content,
            coherence: 0.5,
            agent_id: String::new(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn merge_converges_and_is_idempotent() {
        let mut a = CrdtDocument::new("agent-a");
        let mut b = CrdtDocument::new("agent-b");
        a.append_trajectory(obs(serde_json::json!({"n": 1})));
        b.append_trajectory(obs(serde_json::json!({"n": 2})));

        let b_state = b.encode_state();
        let b_doc = CrdtDocument::decode_state(&b_state).unwrap();
        a.merge(&b_doc);
        assert_eq!(a.trajectory.len(), 2);

        // Re-merge the same state: no duplication, no divergence.
        let hash_after_first = a.hash.clone();
        a.merge(&b_doc);
        assert_eq!(a.trajectory.len(), 2);
        a.merge(&b_doc);
        assert_eq!(a.trajectory.len(), 2);
        assert_eq!(a.hash, hash_after_first);
    }

    #[test]
    fn pattern_counts_merge_as_g_counter() {
        let mut a = CrdtDocument::new("agent-a");
        let mut b = CrdtDocument::new("agent-b");
        a.pattern_counts.insert("p".into(), 2);
        b.pattern_counts.insert("p".into(), 5);
        a.merge(&b);
        assert_eq!(a.pattern_counts.get("p"), Some(&5));
        b.merge(&a);
        assert_eq!(b.pattern_counts.get("p"), Some(&5));
    }

    #[test]
    fn context_last_write_wins_by_version() {
        let mut a = CrdtDocument::new("agent-a");
        a.set_context("k", serde_json::json!(1));
        a.set_context("k", serde_json::json!(2));
        let mut b = CrdtDocument::new("agent-b");
        b.set_context("k", serde_json::json!(99));
        b.merge(&a);
        assert_eq!(b.get_context("k"), Some(&serde_json::json!(2)));
    }
}
