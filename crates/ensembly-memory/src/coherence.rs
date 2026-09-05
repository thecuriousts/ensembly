//! Coherence engine — on-demand reflection over the durable trajectory.
//! Unlike a daemon loop, the kernel drives this explicitly (`runtime reflect`)
//! so every effect is inspectable; engine state (pattern counts, coherence
//! history) lives inside the CrdtDocument and therefore merges.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::crdt::{CrdtDocument, Goal, Skill, TrajectoryEntry, TrajectoryType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceConfig {
    pub coherence_threshold: f32,
    pub max_trajectory_window_hours: i64,
    pub skill_synthesis_threshold: u64,
    /// Reflection is skipped (loudly, by the caller) below this many entries.
    pub min_entries_for_reflection: usize,
}

impl Default for CoherenceConfig {
    fn default() -> Self {
        Self {
            coherence_threshold: 0.7,
            max_trajectory_window_hours: 24,
            skill_synthesis_threshold: 3,
            min_entries_for_reflection: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceRecord {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub coherence: f32,
    pub trajectory_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reflection {
    pub timestamp: DateTime<Utc>,
    pub trajectory_length: usize,
    pub coherence: f32,
    pub active_goals: usize,
    pub known_skills: usize,
    pub new_patterns: usize,
    pub new_skills: Vec<String>,
    pub goal_proposals: Vec<GoalProposal>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPattern {
    pub signature: String,
    pub sequence: Vec<serde_json::Value>,
    pub count: u64,
    pub coherence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalProposal {
    pub description: String,
    pub priority: f32,
    pub goal_type: String,
    pub skill_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceReport {
    pub current: f32,
    pub average: f32,
    pub trend: f32,
    pub history: Vec<CoherenceRecord>,
    pub skills_count: usize,
    pub trajectory_length: usize,
}

/// Jaccard word-set similarity over the JSON string forms — deterministic,
/// model-free. An LLM judge may augment this later behind a trait; the
/// deterministic path stays the fallback and the test oracle.
pub fn semantic_similarity(a: &serde_json::Value, b: &serde_json::Value) -> f32 {
    let words = |v: &serde_json::Value| -> HashSet<String> {
        v.to_string()
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2)
            .map(str::to_string)
            .collect()
    };
    let a_words = words(a);
    let b_words = words(b);
    if a_words.is_empty() && b_words.is_empty() {
        return 1.0;
    }
    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }
    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();
    intersection as f32 / union as f32
}

pub fn compute_coherence(doc: &CrdtDocument, observation: &serde_json::Value) -> f32 {
    let goals = doc.get_active_goals();
    let recent = doc.get_recent_trajectory(1);

    let mut coherence = 0.5_f32;
    if !goals.is_empty() {
        let alignment = goals
            .iter()
            .map(|g| semantic_similarity(observation, &serde_json::json!(g.description)))
            .sum::<f32>()
            / goals.len() as f32;
        coherence = 0.3 + 0.7 * alignment;
    }
    if !recent.is_empty() {
        let temporal = recent
            .iter()
            .map(|e| semantic_similarity(observation, &e.content))
            .sum::<f32>()
            / recent.len() as f32;
        coherence = 0.5 * coherence + 0.5 * temporal;
    }
    coherence.clamp(0.0, 1.0)
}

pub fn trajectory_coherence(trajectory: &[&TrajectoryEntry]) -> f32 {
    if trajectory.len() < 2 {
        return 1.0;
    }
    let sum: f32 = trajectory
        .windows(2)
        .map(|w| semantic_similarity(&w[0].content, &w[1].content))
        .sum();
    sum / (trajectory.len() - 1) as f32
}

/// Sliding n-gram (n=3) over action signatures in the reflection window.
/// Counts accumulate in the document (durable), so a pattern seen once per
/// CLI run still synthesizes after `skill_synthesis_threshold` runs.
pub fn detect_patterns(doc: &mut CrdtDocument, config: &CoherenceConfig) -> Vec<SkillPattern> {
    // Clone the action entries so the borrow ends before counting begins.
    let actions: Vec<TrajectoryEntry> = doc
        .get_recent_trajectory(config.max_trajectory_window_hours)
        .into_iter()
        .filter(|e| e.entry_type == TrajectoryType::Action)
        .cloned()
        .collect();

    let mut patterns = Vec::new();
    for window in actions.windows(3) {
        let sig = window
            .iter()
            .map(|e| {
                e.content
                    .get("action")
                    .and_then(|a| a.get("intent").or_else(|| a.get("description")))
                    .unwrap_or(&e.content)
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(" -> ");

        let count = doc.pattern_counts.entry(sig.clone()).or_insert(0);
        *count += 1;

        if *count >= config.skill_synthesis_threshold {
            let refs: Vec<&TrajectoryEntry> = window.iter().collect();
            patterns.push(SkillPattern {
                signature: sig,
                sequence: window.iter().map(|e| e.content.clone()).collect(),
                count: *count,
                coherence: trajectory_coherence(&refs),
            });
        }
    }
    patterns
}

pub fn synthesize_skill(doc: &mut CrdtDocument, pattern: &SkillPattern) -> Option<String> {
    let skill_name = format!(
        "skill_{}",
        pattern
            .signature
            .replace(|c: char| !c.is_alphanumeric(), "_")
            .chars()
            .take(50)
            .collect::<String>()
    );

    if let Some(existing) = doc.skills.get(&skill_name) {
        if existing.version >= pattern.count {
            return None;
        }
    }

    let trigger = pattern
        .sequence
        .first()
        .and_then(|s| s.get("observation").or_else(|| s.get("context")))
        .cloned()
        .unwrap_or(serde_json::json!({"trigger": "unknown"}));
    let preconditions: Vec<_> = pattern
        .sequence
        .iter()
        .filter_map(|s| s.get("context").or_else(|| s.get("observation")).cloned())
        .collect();
    let effects: Vec<_> = pattern
        .sequence
        .iter()
        .filter_map(|s| s.get("result").cloned())
        .collect();

    let skill = Skill {
        name: skill_name.clone(),
        pattern: pattern.signature.clone(),
        sequence: pattern.sequence.clone(),
        trigger,
        preconditions,
        effects,
        confidence: pattern.coherence,
        usage_count: pattern.count,
        last_used: Utc::now(),
        version: pattern.count,
        agent_id: doc.agent_id.clone(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    doc.add_skill(skill);
    Some(skill_name)
}

pub fn propose_goals(doc: &CrdtDocument) -> Vec<GoalProposal> {
    let mut proposals = Vec::new();
    let active = doc.get_active_goals();

    if active.len() < 3 {
        proposals.push(GoalProposal {
            description: "Explore and develop new capabilities based on recent patterns".into(),
            priority: 0.7,
            goal_type: "exploration".into(),
            skill_name: None,
        });
    }
    for skill in doc.skills.values() {
        if skill.confidence > 0.8 && skill.usage_count > 5 {
            proposals.push(GoalProposal {
                description: format!("Refine and generalize skill: {}", skill.name),
                priority: 0.8,
                goal_type: "skill_refinement".into(),
                skill_name: Some(skill.name.clone()),
            });
        }
    }
    proposals
}

/// One explicit reflection pass. Returns None when the window is too small —
/// the caller decides how to say that loudly. On success, appends a
/// Reflection trajectory entry and records coherence history in the doc.
pub fn reflect(doc: &mut CrdtDocument, config: &CoherenceConfig) -> Option<Reflection> {
    // Scope the read-only window borrow: everything needed from `recent` is
    // computed here, before any mutation of the document.
    let (recent_len, coherence) = {
        let recent = doc.get_recent_trajectory(config.max_trajectory_window_hours);
        if recent.len() < config.min_entries_for_reflection {
            return None;
        }
        (recent.len(), trajectory_coherence(&recent))
    };

    let record = CoherenceRecord {
        id: crate::crdt::entry_id(
            "c",
            &doc.agent_id,
            doc.vector_clock.get(&doc.agent_id).copied().unwrap_or(0),
            &serde_json::json!({"coherence": coherence, "n": recent_len}),
            Utc::now(),
        ),
        timestamp: Utc::now(),
        coherence,
        trajectory_length: recent_len,
    };
    doc.coherence_history.push(record);
    if doc.coherence_history.len() > 100 {
        let drain = doc.coherence_history.len() - 100;
        doc.coherence_history.drain(0..drain);
    }

    let patterns = detect_patterns(doc, config);
    let mut new_skills = Vec::new();
    for pattern in &patterns {
        if let Some(name) = synthesize_skill(doc, pattern) {
            new_skills.push(name);
        }
    }

    let proposals = propose_goals(doc);
    let reflection = Reflection {
        timestamp: Utc::now(),
        trajectory_length: recent_len,
        coherence,
        active_goals: doc.get_active_goals().len(),
        known_skills: doc.skills.len(),
        new_patterns: patterns.len(),
        new_skills,
        goal_proposals: proposals,
        summary: format!(
            "Coherence {:.1}% over {} entries. {} skills known.",
            coherence * 100.0,
            recent_len,
            doc.skills.len()
        ),
    };

    let content = serde_json::to_value(&reflection).unwrap_or_default();
    doc.append_trajectory(TrajectoryEntry {
        id: String::new(),
        timestamp: Utc::now(),
        entry_type: TrajectoryType::Reflection,
        content,
        coherence,
        agent_id: String::new(),
        metadata: HashMap::new(),
    });
    Some(reflection)
}

pub fn coherence_report(doc: &CrdtDocument) -> CoherenceReport {
    let history: Vec<CoherenceRecord> = doc.coherence_history.iter().rev().take(10).cloned().collect();
    let average = if history.is_empty() {
        0.5
    } else {
        history.iter().map(|r| r.coherence).sum::<f32>() / history.len() as f32
    };
    let trend = if history.len() > 1 {
        history[0].coherence - history[history.len() - 1].coherence
    } else {
        0.0
    };
    CoherenceReport {
        current: history.first().map(|r| r.coherence).unwrap_or(0.5),
        average,
        trend,
        history: history.into_iter().rev().collect(),
        skills_count: doc.skills.len(),
        trajectory_length: doc.get_recent_trajectory(24).len(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn goal(
    description: &str,
    priority: f32,
    goal_type: &str,
    skill_name: Option<String>,
) -> Goal {
    Goal {
        id: String::new(),
        description: description.into(),
        priority,
        goal_type: goal_type.into(),
        skill_name,
        status: crate::crdt::GoalStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        agent_id: String::new(),
        progress: 0.0,
        metadata: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crdt::TrajectoryEntry;

    fn push_actions(doc: &mut CrdtDocument, intents: &[&str]) {
        for intent in intents {
            doc.append_trajectory(TrajectoryEntry {
                id: String::new(),
                timestamp: Utc::now(),
                entry_type: TrajectoryType::Action,
                content: serde_json::json!({
                    "action": {"intent": intent},
                    "result": {"success": true}
                }),
                coherence: 0.8,
                agent_id: String::new(),
                metadata: HashMap::new(),
            });
        }
    }

    #[test]
    fn reflection_requires_minimum_window() {
        let mut doc = CrdtDocument::new("t");
        push_actions(&mut doc, &["a", "b"]);
        assert!(reflect(&mut doc, &CoherenceConfig::default()).is_none());
    }

    #[test]
    fn skill_synthesizes_across_durable_reflects() {
        let config = CoherenceConfig {
            skill_synthesis_threshold: 2,
            min_entries_for_reflection: 3,
            ..Default::default()
        };
        let mut doc = CrdtDocument::new("t");
        // First reflect: pattern seen once, below threshold — no skill yet.
        push_actions(&mut doc, &["x", "y", "z"]);
        let r1 = reflect(&mut doc, &config).expect("enough entries");
        assert_eq!(r1.known_skills, 0);
        // Second reflect over the same window: counts accumulate in the doc.
        let r2 = reflect(&mut doc, &config).expect("enough entries");
        assert!(
            !r2.new_skills.is_empty(),
            "pattern crossing threshold synthesizes a skill"
        );
        assert!(!doc.skills.is_empty());
        // Reflection entries themselves were appended.
        assert!(doc
            .trajectory
            .values()
            .any(|e| e.entry_type == TrajectoryType::Reflection));
        assert!(!doc.coherence_history.is_empty());
    }

    #[test]
    fn report_tracks_history() {
        let config = CoherenceConfig::default();
        let mut doc = CrdtDocument::new("t");
        push_actions(&mut doc, &["a", "b", "c", "d", "e"]);
        reflect(&mut doc, &config);
        let report = coherence_report(&doc);
        assert!(report.current > 0.0);
        assert_eq!(report.history.len(), 1);
    }

    #[test]
    fn similarity_is_symmetric_and_bounded() {
        let a = serde_json::json!({"task": "triage inbox messages"});
        let b = serde_json::json!({"task": "triage inbox"});
        let s1 = semantic_similarity(&a, &b);
        let s2 = semantic_similarity(&b, &a);
        assert!((s1 - s2).abs() < f32::EPSILON);
        assert!((0.0..=1.0).contains(&s1));
        assert!(s1 > 0.5);
    }
}
