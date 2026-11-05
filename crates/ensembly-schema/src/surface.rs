//! Shared Commit and View entry for the kernel CLI and the MCP server.

use crate::commit::{Commit, CommitError, CommitFault, CommitResult};
use crate::record::{EventEnvelope, Version};
use crate::replay::{parse_event_log, ReplayError};
use crate::view::{Feed, RecordView, Snapshot, ViewContract};
use crate::{Checked, UnitOfWork};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonPayload(pub serde_json::Value);

impl Checked for JsonPayload {
    fn check(&self) -> Result<(), String> {
        let Some(writer) = self.0.get("writer").and_then(|v| v.as_str()) else {
            return Ok(());
        };
        if matches!(writer, "laptop-1" | "laptop-2" | "mac-mini") {
            Ok(())
        } else {
            Err("writer must be a public host alias".into())
        }
    }
}

#[derive(Debug)]
pub enum RecordSurfaceError {
    Json(String),
    Commit(CommitError),
    Replay(ReplayError),
    Idempotency { key: String },
    MissingLog(String),
}

impl std::fmt::Display for RecordSurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordSurfaceError::Json(m) => write!(f, "{m}"),
            RecordSurfaceError::Commit(e) => write!(f, "{e}"),
            RecordSurfaceError::Replay(e) => write!(f, "{e}"),
            RecordSurfaceError::Idempotency { key } => {
                write!(f, "idempotency key rejected: {key}")
            }
            RecordSurfaceError::MissingLog(m) => write!(f, "missing log: {m}"),
        }
    }
}

impl std::error::Error for RecordSurfaceError {}

pub fn commit_json(
    log: &Path,
    unit_json: &str,
    fault: &CommitFault,
) -> Result<CommitResult, RecordSurfaceError> {
    let unit = parse_unit(unit_json)?;
    reject_known_keys(log, &unit)?;
    Commit::new(log)
        .run(unit, fault)
        .map_err(RecordSurfaceError::Commit)
}

pub fn recover_json(log: &Path, fault: &CommitFault) -> Result<CommitResult, RecordSurfaceError> {
    Commit::new(log)
        .recover::<JsonPayload>(fault)
        .map_err(RecordSurfaceError::Commit)
}

pub fn snapshot_json(log: &Path, at: Option<u64>) -> Result<Snapshot, RecordSurfaceError> {
    let events = load_events(log)?;
    let at = at.unwrap_or_else(|| events.iter().map(|e| e.version.0).max().unwrap_or(0));
    RecordView::from_events(events)
        .snapshot_at(Version(at))
        .map_err(RecordSurfaceError::Replay)
}

pub fn feed_json(log: &Path, since: u64) -> Result<Feed<JsonPayload>, RecordSurfaceError> {
    let events = load_events(log)?;
    Ok(RecordView::from_events(events).change_feed(Version(since)))
}

fn parse_unit(json: &str) -> Result<UnitOfWork<JsonPayload>, RecordSurfaceError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| RecordSurfaceError::Json(e.to_string()))?;
    match value {
        serde_json::Value::Array(items) => {
            let events = serde_json::from_value(serde_json::Value::Array(items))
                .map_err(|e| RecordSurfaceError::Json(e.to_string()))?;
            Ok(UnitOfWork { events })
        }
        serde_json::Value::Object(map) if map.contains_key("events") => {
            serde_json::from_value(serde_json::Value::Object(map))
                .map_err(|e| RecordSurfaceError::Json(e.to_string()))
        }
        serde_json::Value::Object(_)
        | serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => Err(RecordSurfaceError::Json(
            "unit must be an event array or an object with events".into(),
        )),
    }
}

fn load_events(log: &Path) -> Result<Vec<EventEnvelope<JsonPayload>>, RecordSurfaceError> {
    let text = std::fs::read_to_string(log).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            RecordSurfaceError::MissingLog(log.display().to_string())
        } else {
            RecordSurfaceError::Replay(ReplayError::Io(e.to_string()))
        }
    })?;
    parse_event_log(&text).map_err(RecordSurfaceError::Replay)
}

fn reject_known_keys(log: &Path, unit: &UnitOfWork<JsonPayload>) -> Result<(), RecordSurfaceError> {
    let existing = if log.is_file() {
        load_events(log)?
    } else {
        Vec::new()
    };
    let mut seen: BTreeSet<(String, String)> = existing
        .into_iter()
        .map(|event| (event.aggregate.0, event.key.0))
        .collect();
    for event in &unit.events {
        let key = event.key.0.clone();
        if !seen.insert((event.aggregate.0.clone(), key.clone())) {
            return Err(RecordSurfaceError::Idempotency { key });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commit::{CommitFault, CommitPhase};
    use crate::record::Version;
    use crate::replay::{parse_event_log, replay};
    use crate::view::{view_append, RecordView, ViewContract, ViewError};
    use crate::UnitOfWork;
    use std::path::PathBuf;

    const SCENARIO_A: &str = include_str!("../fixtures/scenarios/a-happy-events.json");
    const SCENARIO_D: &str = include_str!("../fixtures/scenarios/d-crash-unit.json");

    fn temp_log() -> (PathBuf, PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("record-surface-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("events.jsonl");
        (dir, log)
    }

    fn nonempty_lines(path: &Path) -> usize {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count()
    }

    fn dir_has_partial_jsonl_line(dir: &Path) -> bool {
        for ent in std::fs::read_dir(dir).unwrap() {
            let path = ent.unwrap().path();
            if !path.is_file() {
                continue;
            }
            let Ok(body) = std::fs::read_to_string(&path) else {
                continue;
            };
            if body.is_empty() {
                continue;
            }
            if body.contains('\n') {
                for line in body.split('\n') {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if serde_json::from_str::<serde_json::Value>(line).is_err() {
                        return true;
                    }
                }
            } else if serde_json::from_str::<serde_json::Value>(&body).is_err() {
                return true;
            }
        }
        false
    }

    #[test]
    fn scenario_a_commit_snapshot_equals_replay_and_rejects_same_key() {
        let (dir, log) = temp_log();
        commit_json(&log, SCENARIO_A, &CommitFault::default()).unwrap();
        let snap = snapshot_json(&log, None).unwrap();
        let text = std::fs::read_to_string(&log).unwrap();
        let events = parse_event_log::<JsonPayload>(&text).unwrap();
        let rows = replay(&events).unwrap();
        let via = RecordView::from_events(events)
            .snapshot_at(snap.at)
            .unwrap();
        assert_eq!(snap, via);
        assert_eq!(snap.facts.len(), rows.len());
        for (fact, row) in snap.facts.iter().zip(rows.iter()) {
            assert_eq!(fact.stable_id, row.aggregate);
            assert_eq!(fact.version, row.version);
        }
        let err = commit_json(&log, SCENARIO_A, &CommitFault::default()).unwrap_err();
        assert!(err.to_string().contains("idempotency"), "{err}");
        assert_eq!(nonempty_lines(&log), 3);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn scenario_d_crash_mid_append_leaves_no_partial_line_and_recover_finishes() {
        let (dir, log) = temp_log();
        let abort = CommitFault {
            append_abort_after_bytes: Some(10),
            ..Default::default()
        };
        assert!(commit_json(&log, SCENARIO_D, &abort).is_err());
        assert!(!log.exists());
        assert!(!dir_has_partial_jsonl_line(&dir));

        let (dir_v, log_v) = temp_log();
        let fault = CommitFault {
            crash_entering: Some(CommitPhase::Verify),
            ..Default::default()
        };
        let mid = commit_json(&log_v, SCENARIO_D, &fault).unwrap();
        assert_eq!(mid.phase, CommitPhase::Verify);
        let done = recover_json(&log_v, &CommitFault::default()).unwrap();
        assert_eq!(done.phase, CommitPhase::Done);
        assert_eq!(nonempty_lines(&log_v), 2);
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(dir_v);
    }

    #[test]
    fn feed_and_snapshot_at_match_view_contract_and_view_append_refuses() {
        let (dir, log) = temp_log();
        commit_json(&log, SCENARIO_A, &CommitFault::default()).unwrap();
        let before = std::fs::read(&log).unwrap();
        let at_one = snapshot_json(&log, Some(1)).unwrap();
        assert_eq!(at_one.at, Version(1));
        assert!(at_one.facts.iter().all(|f| f.version.0 <= 1));
        let feed = feed_json(&log, 1).unwrap();
        let events =
            parse_event_log::<JsonPayload>(&std::fs::read_to_string(&log).unwrap()).unwrap();
        let via = RecordView::from_events(events.clone()).change_feed(Version(1));
        assert_eq!(feed, via);
        assert!(feed.events.iter().all(|e| e.version.0 > 1));
        let view = RecordView::from_events(events.clone());
        let unit = UnitOfWork { events };
        assert_eq!(view_append(&view, unit), Err(ViewError::ReadOnly));
        assert_eq!(std::fs::read(&log).unwrap(), before);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn writer_outside_public_host_alias_is_rejected() {
        let (_dir, log) = temp_log();
        let unit = r#"[{
            "id": "01JCV1Q8Z00000000000000000",
            "aggregate": "agg://example.test/acme-north/staff/1",
            "key": "bad-writer",
            "version": 1,
            "event_time": "2026-10-26T18:59:00+01:00",
            "record_time": "2026-10-26T19:00:00+01:00",
            "source": "owner",
            "schema_v": 1,
            "payload": {"writer": "office-box", "note": "no"}
        }]"#;
        let err = commit_json(&log, unit, &CommitFault::default()).unwrap_err();
        assert!(err.to_string().contains("public host alias"), "{err}");
    }
}
