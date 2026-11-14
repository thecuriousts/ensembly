//! Domain-agnostic event-log replay, materialized aggregates, and restore drill.

use crate::record::{AggregateId, EventEnvelope, IdempotencyKey, Version};
use crate::{decode_line, encode_line, Checked, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedAggregate<P> {
    pub aggregate: AggregateId,
    pub version: Version,
    pub event: EventEnvelope<P>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    Line(usize, String),
    IdempotencyConflict {
        aggregate: AggregateId,
        key: IdempotencyKey,
    },
    Io(String),
    Restore(String),
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Line(n, m) => write!(f, "line {n}: {m}"),
            ReplayError::IdempotencyConflict { aggregate, key } => {
                write!(
                    f,
                    "idempotency conflict on aggregate {:?} key {:?}",
                    aggregate.0, key.0
                )
            }
            ReplayError::Io(m) => write!(f, "io: {m}"),
            ReplayError::Restore(m) => write!(f, "restore: {m}"),
        }
    }
}

/// Parse a JSONL event log (blank lines skipped).
pub fn parse_event_log<P>(text: &str) -> Result<Vec<EventEnvelope<P>>, ReplayError>
where
    P: Checked + for<'de> Deserialize<'de>,
{
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event = decode_line::<EventEnvelope<P>>(line)
            .map_err(|e| ReplayError::Line(i + 1, e))?
            .value;
        event.check().map_err(|e| ReplayError::Line(i + 1, e))?;
        out.push(event);
    }
    Ok(out)
}

/// Materialize current aggregate rows from ordered events (idempotent on same key + payload).
pub fn replay<P>(events: &[EventEnvelope<P>]) -> Result<Vec<MaterializedAggregate<P>>, ReplayError>
where
    P: Checked + Serialize + Clone,
{
    let mut seen: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut latest: BTreeMap<String, MaterializedAggregate<P>> = BTreeMap::new();

    for event in events {
        let line = encode_line(SCHEMA_VERSION, event).map_err(ReplayError::Io)?;
        let idem = (event.aggregate.0.clone(), event.key.0.clone());
        if let Some(prev) = seen.get(&idem) {
            if prev == &line {
                continue;
            }
            return Err(ReplayError::IdempotencyConflict {
                aggregate: event.aggregate.clone(),
                key: event.key.clone(),
            });
        }
        seen.insert(idem, line);

        let agg_key = event.aggregate.0.clone();
        let replace = latest
            .get(&agg_key)
            .is_none_or(|row| event.version.0 >= row.version.0);
        if replace {
            latest.insert(
                agg_key,
                MaterializedAggregate {
                    aggregate: event.aggregate.clone(),
                    version: event.version,
                    event: event.clone(),
                },
            );
        }
    }

    Ok(current_rows(latest.into_values().collect()))
}

/// Stable sort order for materialized rows (by aggregate id, then version).
pub fn current_rows<P>(mut rows: Vec<MaterializedAggregate<P>>) -> Vec<MaterializedAggregate<P>> {
    rows.sort_by(|a, b| {
        a.aggregate
            .0
            .cmp(&b.aggregate.0)
            .then_with(|| a.version.0.cmp(&b.version.0))
    });
    rows
}

/// Read a log file and materialize rows.
pub fn replay_log_file<P>(path: &Path) -> Result<Vec<MaterializedAggregate<P>>, ReplayError>
where
    P: Checked + Serialize + Clone + for<'de> Deserialize<'de>,
{
    let text = std::fs::read_to_string(path).map_err(|e| ReplayError::Io(e.to_string()))?;
    let events = parse_event_log(&text)?;
    replay(&events)
}

/// Copy backup bytes to a restored path; returns restored body for checks.
pub fn restore_log(source: &Path, restored: &Path) -> Result<Vec<u8>, ReplayError> {
    let bytes = std::fs::read(source).map_err(|e| ReplayError::Io(e.to_string()))?;
    if let Some(dir) = restored.parent() {
        std::fs::create_dir_all(dir).map_err(|e| ReplayError::Io(e.to_string()))?;
    }
    std::fs::write(restored, &bytes).map_err(|e| ReplayError::Io(e.to_string()))?;
    Ok(bytes)
}

/// Restore drill on a temp copy; the source log's modification time must not change.
pub fn restore_drill(source: &Path) -> Result<(), ReplayError> {
    let meta = std::fs::metadata(source).map_err(|e| ReplayError::Io(e.to_string()))?;
    let mtime_before = meta
        .modified()
        .map_err(|e| ReplayError::Restore(e.to_string()))?;

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "ensembly-record-restore-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).map_err(|e| ReplayError::Io(e.to_string()))?;
    let backup = dir.join("backup.jsonl");
    let restored = dir.join("restored.jsonl");

    let bytes = restore_log(source, &backup)?;
    let back = restore_log(&backup, &restored)?;
    if back != bytes {
        return Err(ReplayError::Restore(
            "restored bytes differ from backup".into(),
        ));
    }

    let mtime_after = std::fs::metadata(source)
        .map_err(|e| ReplayError::Io(e.to_string()))?
        .modified()
        .map_err(|e| ReplayError::Restore(e.to_string()))?;
    if mtime_after != mtime_before {
        return Err(ReplayError::Restore("source mtime changed".into()));
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commit::{Commit, CommitFault, UnitOfWork};
    use crate::record::{EventId, Rfc3339Time, Source};
    use std::time::Duration;

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

    fn t(s: &str) -> Rfc3339Time {
        Rfc3339Time::new(s).unwrap()
    }

    fn event(n: u64, writer: &str, aggregate: &str, key: &str) -> EventEnvelope<ScenarioPayload> {
        EventEnvelope {
            id: EventId(ulid::Ulid::from_parts(
                1_700_000_000 + (n % 10_000_000),
                n as u128,
            )),
            aggregate: AggregateId(aggregate.into()),
            key: IdempotencyKey(key.into()),
            version: Version(n),
            event_time: t("2026-10-26T18:59:00+01:00"),
            record_time: t("2026-10-26T19:00:00+01:00"),
            source: Source::Owner,
            prev_hash: None,
            schema_v: SCHEMA_VERSION,
            payload: ScenarioPayload {
                writer: writer.into(),
                note: "scenario".into(),
            },
        }
    }

    fn temp_log() -> (std::path::PathBuf, std::path::PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "ensembly-record-replay-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        (dir.clone(), dir.join("events.jsonl"))
    }

    #[test]
    fn replay_log_equals_materialized_rows() {
        let (dir, log) = temp_log();
        let commit = Commit::new(&log);
        let unit = UnitOfWork {
            events: vec![
                event(1, "laptop-1", "agg://example.test/acme-north/1", "a-1"),
                event(2, "laptop-1", "agg://example.test/acme-north/1", "a-2"),
                event(1, "laptop-2", "agg://example.test/riverbench/1", "b-1"),
            ],
        };
        commit.run(unit, &CommitFault::default()).unwrap();
        let text = std::fs::read_to_string(&log).unwrap();
        let parsed = parse_event_log::<ScenarioPayload>(&text).unwrap();
        let from_replay = replay(&parsed).unwrap();
        let from_file = replay_log_file::<ScenarioPayload>(&log).unwrap();
        assert_eq!(from_replay, from_file);
        assert_eq!(from_replay.len(), 2);
        let acme = from_replay
            .iter()
            .find(|r| r.aggregate.0.contains("acme-north"))
            .unwrap();
        assert_eq!(acme.version.0, 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn replay_twice_same_as_once_over_1000_keys() {
        let mut events = Vec::with_capacity(2000);
        for i in 0..1000 {
            let n = (i as u64) + 1;
            let agg = format!("agg://example.test/cobalt/{n}");
            let key = format!("key-{n}");
            let ev = event(n, "mac-mini", &agg, &key);
            events.push(ev.clone());
            events.push(ev);
        }
        let once = replay(&events).unwrap();
        assert_eq!(once.len(), 1000);
        let twice = replay(&events).unwrap();
        assert_eq!(once, twice);
        let mut doubled_log = events.clone();
        doubled_log.extend(events);
        assert_eq!(replay(&doubled_log).unwrap(), once);
    }

    #[test]
    fn restore_drill_leaves_source_mtime_unchanged() {
        let (dir, log) = temp_log();
        let commit = Commit::new(&log);
        let unit = UnitOfWork {
            events: vec![event(
                1,
                "laptop-1",
                "agg://example.test/glasspine/1",
                "d-restore",
            )],
        };
        commit.run(unit, &CommitFault::default()).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        restore_drill(&log).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn scenario_a_fixture_replays() {
        let fixture = include_str!("../fixtures/scenarios/a-happy-events.json");
        let events: Vec<EventEnvelope<ScenarioPayload>> =
            serde_json::from_str(fixture).expect("scenario a fixture");
        let rows = replay(&events).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows
            .iter()
            .all(|r| matches!(r.event.payload.writer.as_str(), "laptop-1" | "mac-mini")));
    }
}
