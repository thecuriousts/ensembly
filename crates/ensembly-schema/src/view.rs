//! Domain-agnostic read contract: snapshot, change feed, facts with text + stable id.
//! Views are read-only; they never append to the log.

use crate::record::{AggregateId, EventEnvelope, TextForm, Version};
use crate::replay::{replay, MaterializedAggregate, ReplayError};
use crate::{Checked, UnitOfWork};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    pub stable_id: AggregateId,
    pub text: TextForm,
    pub version: Version,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub at: Version,
    pub facts: Vec<Fact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeEvent<P> {
    pub stable_id: AggregateId,
    pub text: TextForm,
    pub version: Version,
    pub envelope: EventEnvelope<P>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feed<P> {
    pub since: Version,
    pub events: Vec<ChangeEvent<P>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewError {
    ReadOnly,
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewError::ReadOnly => write!(f, "views cannot write"),
        }
    }
}

/// Read-only view over an in-memory event list (built from log parse or fixtures).
pub struct RecordView<P> {
    events: Vec<EventEnvelope<P>>,
}

impl<P> RecordView<P>
where
    P: Checked + Serialize + Clone + for<'de> Deserialize<'de>,
{
    pub fn from_events(events: Vec<EventEnvelope<P>>) -> Self {
        Self { events }
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

pub trait ViewContract<P> {
    fn snapshot_at(&self, at: Version) -> Result<Snapshot, ReplayError>;
    fn change_feed(&self, since: Version) -> Feed<P>;
    fn text_of(&self, id: &AggregateId) -> Option<TextForm>;
    fn stable_id(&self, id: &AggregateId) -> AggregateId;
}

impl<P> ViewContract<P> for RecordView<P>
where
    P: Checked + Serialize + Clone + for<'de> Deserialize<'de>,
{
    fn snapshot_at(&self, at: Version) -> Result<Snapshot, ReplayError> {
        let filtered: Vec<_> = self
            .events
            .iter()
            .filter(|e| e.version.0 <= at.0)
            .cloned()
            .collect();
        let rows = replay(&filtered)?;
        Ok(Snapshot {
            at,
            facts: facts_from_rows(&rows),
        })
    }

    fn change_feed(&self, since: Version) -> Feed<P> {
        let mut events: Vec<ChangeEvent<P>> = self
            .events
            .iter()
            .filter(|e| e.version.0 > since.0)
            .map(|e| change_event_from_envelope(e))
            .collect();
        events.sort_by(|a, b| {
            a.version
                .0
                .cmp(&b.version.0)
                .then_with(|| a.stable_id.0.cmp(&b.stable_id.0))
        });
        Feed { since, events }
    }

    fn text_of(&self, id: &AggregateId) -> Option<TextForm> {
        self.events
            .iter()
            .filter(|e| &e.aggregate == id)
            .max_by_key(|e| e.version.0)
            .map(fact_text_from_envelope)
    }

    fn stable_id(&self, id: &AggregateId) -> AggregateId {
        id.clone()
    }
}

/// Views must not mutate the log; append is refused.
pub fn view_append<P>(_view: &RecordView<P>, _unit: UnitOfWork<P>) -> Result<(), ViewError> {
    Err(ViewError::ReadOnly)
}

fn facts_from_rows<P>(rows: &[MaterializedAggregate<P>]) -> Vec<Fact> {
    let mut facts: Vec<Fact> = rows
        .iter()
        .map(|r| Fact {
            stable_id: r.aggregate.clone(),
            text: fact_text_from_envelope(&r.event),
            version: r.version,
        })
        .collect();
    facts.sort_by(|a, b| a.stable_id.0.cmp(&b.stable_id.0));
    facts
}

fn fact_text_from_envelope<P>(e: &EventEnvelope<P>) -> TextForm {
    TextForm(format!("{}@v{}", e.aggregate.0, e.version.0))
}

fn change_event_from_envelope<P: Clone>(e: &EventEnvelope<P>) -> ChangeEvent<P> {
    ChangeEvent {
        stable_id: e.aggregate.clone(),
        text: fact_text_from_envelope(e),
        version: e.version,
        envelope: e.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commit::{Commit, CommitFault};
    use crate::replay::parse_event_log;
    use std::path::PathBuf;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ScenarioPayload {
        writer: String,
        note: String,
    }

    impl Checked for ScenarioPayload {
        fn check(&self) -> Result<(), String> {
            crate::validate_writer_identity(&self.writer)?;
            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct DebtFactPayload {
        payer: String,
        amount_sek: i64,
        writer: String,
    }

    impl Checked for DebtFactPayload {
        fn check(&self) -> Result<(), String> {
            if !self.payer.ends_with(".example.test") {
                return Err("payer must be under example.test".into());
            }
            crate::validate_writer_identity(&self.writer)?;
            Ok(())
        }
    }

    fn load_scenario_a() -> RecordView<ScenarioPayload> {
        let events: Vec<EventEnvelope<ScenarioPayload>> =
            serde_json::from_str(include_str!("../fixtures/scenarios/a-happy-events.json"))
                .expect("scenario a");
        RecordView::from_events(events)
    }

    fn load_scenario_f() -> RecordView<DebtFactPayload> {
        let raw = include_str!("../fixtures/scenarios/f-debt-shaped-events.json");
        let events: Vec<EventEnvelope<DebtFactPayload>> =
            serde_json::from_str(raw).expect("scenario f");
        RecordView::from_events(events)
    }

    #[test]
    fn snapshot_at_round_trip() {
        let view = load_scenario_a();
        let snap = view.snapshot_at(Version(2)).unwrap();
        let json = serde_json::to_string(&snap).unwrap();
        let back: Snapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap, back);
        assert_eq!(snap.facts.len(), 2);
    }

    #[test]
    fn change_feed_ordered_by_version() {
        let view = load_scenario_a();
        let feed = view.change_feed(Version(0));
        assert_eq!(feed.events.len(), 3);
        assert!(feed.events.windows(2).all(|w| {
            w[0].version.0 < w[1].version.0
                || (w[0].version.0 == w[1].version.0 && w[0].stable_id.0 <= w[1].stable_id.0)
        }));
    }

    #[test]
    fn every_fact_has_text_and_stable_id() {
        let view = load_scenario_a();
        let snap = view.snapshot_at(Version(99)).unwrap();
        for f in &snap.facts {
            assert!(!f.stable_id.0.is_empty());
            assert!(!f.text.0.is_empty());
        }
        let feed = view.change_feed(Version(0));
        for e in &feed.events {
            assert!(!e.stable_id.0.is_empty());
            assert!(!e.text.0.is_empty());
        }
    }

    #[test]
    fn views_cannot_write() {
        let view = load_scenario_a();
        let events: Vec<EventEnvelope<ScenarioPayload>> =
            serde_json::from_str(include_str!("../fixtures/scenarios/a-happy-events.json"))
                .unwrap();
        let unit = UnitOfWork { events };
        assert_eq!(view_append(&view, unit), Err(ViewError::ReadOnly));
    }

    #[test]
    fn scenario_f_debt_shaped_view_reads() {
        let view = load_scenario_f();
        let snap = view.snapshot_at(Version(1)).unwrap();
        assert!(!snap.facts.is_empty());
        assert!(snap.facts.iter().all(|f| f.text.0.contains("@v")));
    }

    #[test]
    fn snapshot_matches_replay_on_committed_log() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("ensembly-view-test-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log: PathBuf = dir.join("events.jsonl");
        let commit = Commit::new(&log);
        let events: Vec<EventEnvelope<ScenarioPayload>> =
            serde_json::from_str(include_str!("../fixtures/scenarios/a-happy-events.json"))
                .unwrap();
        commit
            .run(
                UnitOfWork {
                    events: events.clone(),
                },
                &CommitFault::default(),
            )
            .unwrap();
        let text = std::fs::read_to_string(&log).unwrap();
        let parsed = parse_event_log::<ScenarioPayload>(&text).unwrap();
        let view = RecordView::from_events(parsed);
        let snap = view.snapshot_at(Version(2)).unwrap();
        assert_eq!(snap.facts.len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }
}
