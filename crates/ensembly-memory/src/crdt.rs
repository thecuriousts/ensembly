//! CRDT document — trajectory / skills / goals / context with entry-level,
//! idempotent, commutative merge. This is the durable learning substrate:
//! concurrent writers (CLI invocations today, peers later) converge without
//! coordination. The kernel runtime remains the control SoT; this document
//! only *remembers*.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};

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

/// A context value kept after a later write replaced it.
///
/// The link is supersession: who replaced this value, and when that write
/// happened. It is not a similarity score. Raw values stay in the archive.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SupersededContext {
    pub value: serde_json::Value,
    pub updated_at: DateTime<Utc>,
    pub agent_id: String,
    pub version: u64,
    pub superseded_by_version: u64,
    pub superseded_by_agent: String,
    /// `true` when a later `set_context` on a replica replaced this value.
    /// `false` when merge archived a concurrent live value under the winner.
    /// Merge unions the explicit links and recomputes the rest, so replica
    /// order cannot change the successor.
    #[serde(default)]
    pub explicit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub updated_at: DateTime<Utc>,
    pub agent_id: String,
    pub version: u64,
    /// Priors displaced by an explicit later write, or by a deterministic
    /// merge when two live values meet. Empty on a first write.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub superseded: Vec<SupersededContext>,
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

        let context_keys: BTreeSet<String> = self
            .context
            .keys()
            .chain(other.context.keys())
            .cloned()
            .collect();
        for key in context_keys {
            let merged = match (self.context.get(&key), other.context.get(&key)) {
                (Some(left), Some(right)) => merge_context_entries(left, right),
                (Some(left), None) => left.clone(),
                (None, Some(right)) => right.clone(),
                (None, None) => continue,
            };
            match self.context.get(&key) {
                Some(existing) if existing == &merged => {}
                _ => {
                    self.context.insert(key, merged);
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

    /// Patch `summary` on the newest Reflection trajectory entry (by timestamp).
    /// Used after optional InferenceProvider enrich so durable memory matches
    /// what the CLI printed. Returns false when no Reflection entry exists.
    pub fn patch_latest_reflection_summary(&mut self, summary: &str) -> bool {
        let Some(id) = self
            .trajectory
            .values()
            .filter(|e| e.entry_type == TrajectoryType::Reflection)
            .max_by_key(|e| e.timestamp)
            .map(|e| e.id.clone())
        else {
            return false;
        };
        let Some(entry) = self.trajectory.get_mut(&id) else {
            return false;
        };
        match entry.content.as_object_mut() {
            Some(obj) => {
                obj.insert("summary".into(), serde_json::json!(summary));
                self.increment_version();
                true
            }
            None => false,
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
        let now = Utc::now();
        let agent = self.agent_id.clone();
        let (version, superseded) = if let Some(prev) = self.context.get(key) {
            let version = prev.version.saturating_add(1);
            let mut superseded = prev.superseded.clone();
            superseded.push(SupersededContext {
                value: prev.value.clone(),
                updated_at: prev.updated_at,
                agent_id: prev.agent_id.clone(),
                version: prev.version,
                superseded_by_version: version,
                superseded_by_agent: agent.clone(),
                explicit: true,
            });
            sort_superseded(&mut superseded);
            (version, superseded)
        } else {
            (1, Vec::new())
        };
        self.context.insert(
            key.to_string(),
            ContextEntry {
                key: key.to_string(),
                value,
                updated_at: now,
                agent_id: agent,
                version,
                superseded,
            },
        );
        self.increment_version();
    }

    pub fn get_context(&self, key: &str) -> Option<&serde_json::Value> {
        self.context.get(key).map(|e| &e.value)
    }

    /// Archive of context values replaced for `key`. The live value is
    /// [`Self::get_context`]. `None` when the key was never written.
    pub fn context_archive(&self, key: &str) -> Option<&[SupersededContext]> {
        self.context.get(key).map(|e| e.superseded.as_slice())
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

#[derive(Clone)]
struct ContextObs {
    value: serde_json::Value,
    updated_at: DateTime<Utc>,
    agent_id: String,
    version: u64,
}

fn obs_rank(obs: &ContextObs) -> (u64, i64, &str, String) {
    (
        obs.version,
        obs.updated_at.timestamp_micros(),
        obs.agent_id.as_str(),
        obs.value.to_string(),
    )
}

fn obs_id(obs: &ContextObs) -> String {
    format!("{}\u{1f}{}\u{1f}{}", obs.agent_id, obs.version, obs.value)
}

fn sort_superseded(items: &mut [SupersededContext]) {
    items.sort_by(|a, b| {
        a.version
            .cmp(&b.version)
            .then(a.agent_id.cmp(&b.agent_id))
            .then(a.value.to_string().cmp(&b.value.to_string()))
            .then(a.superseded_by_version.cmp(&b.superseded_by_version))
            .then(a.superseded_by_agent.cmp(&b.superseded_by_agent))
            .then(a.explicit.cmp(&b.explicit))
    });
}

fn push_obs(into: &mut BTreeMap<String, ContextObs>, obs: ContextObs) {
    let id = obs_id(&obs);
    match into.get(&id) {
        Some(existing) if obs_rank(existing) >= obs_rank(&obs) => {}
        _ => {
            into.insert(id, obs);
        }
    }
}

fn successor_id(obs: &BTreeMap<String, ContextObs>, agent: &str, version: u64) -> Option<String> {
    obs.iter()
        .filter(|(_, item)| item.agent_id == agent && item.version == version)
        .max_by(|(_, a), (_, b)| obs_rank(a).cmp(&obs_rank(b)))
        .map(|(id, _)| id.clone())
}

/// Union both replicas' values and explicit supersession links, then pick one
/// live value. A concurrent live value is kept in the archive with a link to
/// the winner. The same inputs always produce the same entry.
fn merge_context_entries(left: &ContextEntry, right: &ContextEntry) -> ContextEntry {
    let mut observed: BTreeMap<String, ContextObs> = BTreeMap::new();
    for entry in [left, right] {
        push_obs(
            &mut observed,
            ContextObs {
                value: entry.value.clone(),
                updated_at: entry.updated_at,
                agent_id: entry.agent_id.clone(),
                version: entry.version,
            },
        );
        for prior in &entry.superseded {
            push_obs(
                &mut observed,
                ContextObs {
                    value: prior.value.clone(),
                    updated_at: prior.updated_at,
                    agent_id: prior.agent_id.clone(),
                    version: prior.version,
                },
            );
        }
    }

    // Explicit links only. Links created by an earlier merge are recomputed
    // below so they cannot depend on which replica arrived first.
    let mut explicit: BTreeMap<String, String> = BTreeMap::new();
    for entry in [left, right] {
        for prior in &entry.superseded {
            if !prior.explicit {
                continue;
            }
            let from = ContextObs {
                value: prior.value.clone(),
                updated_at: prior.updated_at,
                agent_id: prior.agent_id.clone(),
                version: prior.version,
            };
            let from_id = obs_id(&from);
            let Some(to_id) = successor_id(
                &observed,
                &prior.superseded_by_agent,
                prior.superseded_by_version,
            ) else {
                continue;
            };
            if to_id == from_id {
                continue;
            }
            match explicit.get(&from_id) {
                None => {
                    explicit.insert(from_id, to_id);
                }
                Some(existing) => {
                    let keep_new = observed
                        .get(&to_id)
                        .zip(observed.get(existing))
                        .is_some_and(|(next, prev)| obs_rank(next) > obs_rank(prev));
                    if keep_new {
                        explicit.insert(from_id, to_id);
                    }
                }
            }
        }
    }

    let heads: Vec<String> = observed
        .keys()
        .filter(|id| !explicit.contains_key(*id))
        .cloned()
        .collect();
    let live_id = if heads.is_empty() {
        observed
            .iter()
            .max_by(|(_, a), (_, b)| obs_rank(a).cmp(&obs_rank(b)))
            .map(|(id, _)| id.clone())
            .expect("context merge has at least one value")
    } else {
        heads
            .iter()
            .max_by(|a, b| obs_rank(&observed[*a]).cmp(&obs_rank(&observed[*b])))
            .cloned()
            .expect("context merge has a live value")
    };
    let mut links = explicit.clone();
    for id in observed.keys() {
        if id != &live_id && !links.contains_key(id) {
            links.insert(id.clone(), live_id.clone());
        }
    }
    // A cycle of explicit links leaves every node with a successor. Point the
    // chosen live value at nobody and point the node that targeted it aside.
    if links.contains_key(&live_id) {
        links.remove(&live_id);
    }

    let live = observed.get(&live_id).expect("live context value").clone();
    let mut superseded = Vec::with_capacity(links.len());
    for (from_id, to_id) in &links {
        let Some(from) = observed.get(from_id) else {
            continue;
        };
        let Some(to) = observed.get(to_id) else {
            continue;
        };
        superseded.push(SupersededContext {
            value: from.value.clone(),
            updated_at: from.updated_at,
            agent_id: from.agent_id.clone(),
            version: from.version,
            superseded_by_version: to.version,
            superseded_by_agent: to.agent_id.clone(),
            explicit: explicit.contains_key(from_id),
        });
    }
    sort_superseded(&mut superseded);
    ContextEntry {
        key: left.key.clone(),
        value: live.value,
        updated_at: live.updated_at,
        agent_id: live.agent_id,
        version: live.version,
        superseded,
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
    fn context_live_value_keeps_the_superseded_prior() {
        let mut a = CrdtDocument::new("laptop-1");
        a.set_context("pref", serde_json::json!("tea"));
        a.set_context("pref", serde_json::json!("coffee"));
        let archive = a.context_archive("pref").expect("archive");
        assert_eq!(archive.len(), 1);
        assert_eq!(archive[0].value, serde_json::json!("tea"));
        assert_eq!(archive[0].superseded_by_version, 2);
        assert_eq!(archive[0].superseded_by_agent, "laptop-1");
        assert!(archive[0].explicit, "a later write is supersession");
        assert_eq!(a.get_context("pref"), Some(&serde_json::json!("coffee")));

        let mut b = CrdtDocument::new("laptop-2");
        b.set_context("pref", serde_json::json!("juice"));
        b.merge(&a);
        assert_eq!(b.get_context("pref"), Some(&serde_json::json!("coffee")));
        let kept: Vec<_> = b
            .context_archive("pref")
            .unwrap()
            .iter()
            .map(|s| s.value.clone())
            .collect();
        assert!(kept.contains(&serde_json::json!("tea")));
        assert!(kept.contains(&serde_json::json!("juice")));
    }

    /// Supersession links survive every merge order. Live value and archive
    /// are a function of the replicas, not of the order they arrive.
    #[test]
    fn context_supersession_merge_is_order_independent() {
        let a = pref_doc("laptop-1", &["tea", "coffee"]);
        let b = pref_doc("laptop-2", &["juice"]);
        let c = pref_doc("mac-mini", &["water"]);

        let mut seen = Vec::new();
        for order in [
            [&a, &b, &c],
            [&a, &c, &b],
            [&b, &a, &c],
            [&b, &c, &a],
            [&c, &a, &b],
            [&c, &b, &a],
        ] {
            let mut merged = CrdtDocument::new("box");
            for doc in order {
                merged.merge(doc);
            }
            seen.push(context_projection(&merged, "pref"));
        }
        for item in &seen[1..] {
            assert_eq!(item, &seen[0]);
        }
        assert_eq!(seen[0].0, serde_json::json!("coffee"));
        let archived = &seen[0].1;
        assert!(archived.iter().any(|s| s.0 == serde_json::json!("tea")));
        assert!(archived.iter().any(|s| s.0 == serde_json::json!("juice")));
        assert!(archived.iter().any(|s| s.0 == serde_json::json!("water")));

        let mut ab = CrdtDocument::new("box");
        ab.merge(&a);
        ab.merge(&b);
        let mut left = ab.clone();
        left.merge(&c);
        let mut bc = CrdtDocument::new("box");
        bc.merge(&b);
        bc.merge(&c);
        let mut right = CrdtDocument::new("box");
        right.merge(&a);
        right.merge(&bc);
        assert_eq!(
            context_projection(&left, "pref"),
            context_projection(&right, "pref")
        );

        let hash = left.hash.clone();
        let again = CrdtDocument::decode_state(&left.encode_state()).unwrap();
        left.merge(&again);
        assert_eq!(left.hash, hash, "re-merge must be a no-op");
        assert_eq!(left.context_archive("pref").unwrap().len(), 3);
    }

    #[test]
    fn context_without_superseded_field_still_decodes() {
        let mut doc = CrdtDocument::new("laptop-1");
        doc.set_context("pref", serde_json::json!("tea"));
        let mut value: serde_json::Value = serde_json::from_slice(&doc.encode_state()).unwrap();
        value["context"]["pref"]
            .as_object_mut()
            .unwrap()
            .remove("superseded");
        let decoded = CrdtDocument::decode_state(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(decoded.get_context("pref"), Some(&serde_json::json!("tea")));
        assert!(decoded.context_archive("pref").unwrap().is_empty());
    }

    fn pref_doc(agent: &str, values: &[&str]) -> CrdtDocument {
        let mut doc = CrdtDocument::new(agent);
        for value in values {
            doc.set_context("pref", serde_json::json!(value));
        }
        doc
    }

    fn context_projection(
        doc: &CrdtDocument,
        key: &str,
    ) -> (
        serde_json::Value,
        Vec<(serde_json::Value, u64, String, bool)>,
    ) {
        let live = doc.get_context(key).cloned().unwrap();
        let mut archive: Vec<_> = doc
            .context_archive(key)
            .unwrap()
            .iter()
            .map(|s| {
                (
                    s.value.clone(),
                    s.superseded_by_version,
                    s.superseded_by_agent.clone(),
                    s.explicit,
                )
            })
            .collect();
        archive.sort_by(|a, b| {
            a.0.to_string()
                .cmp(&b.0.to_string())
                .then(a.1.cmp(&b.1))
                .then(a.2.cmp(&b.2))
                .then(a.3.cmp(&b.3))
        });
        (live, archive)
    }
}
