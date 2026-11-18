//! Shared Commit and View entry for the kernel CLI and the MCP server.

use crate::commit::{Commit, CommitError, CommitFault, CommitPhase, CommitResult};
use crate::hlc::{local_append, physical_time_us, HlcState};
use crate::idempotency::same_idempotent_content;
use crate::identity::{
    configured_node, configured_writer, require_configured_writer, validate_writer_identity,
};
use crate::log_index::{
    ensure_snapshot_at_max, extend_index_lines, load_events_since_version, load_events_up_to_version,
    load_or_rebuild, lookup_aggregate_key_entry, read_line_at, rebuild_index, ring_doorbell,
    sidecar_header_state, LogIndexError,
};
use crate::record::{EventEnvelope, EventTimePrec, Rfc3339Time, Version};
use crate::replay::{parse_event_log, replay, ReplayError};
use crate::view::{Feed, RecordView, Snapshot, ViewContract};
use crate::{encode_line, Checked, UnitOfWork, SCHEMA_VERSION};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonPayload(pub serde_json::Value);

impl Checked for JsonPayload {
    fn check(&self) -> Result<(), String> {
        let Some(obj) = self.0.as_object() else {
            return Err("writer is required".into());
        };
        match obj.get("writer") {
            Some(serde_json::Value::String(writer)) => {
                validate_writer_identity(writer)?;
            }
            Some(serde_json::Value::Number(_))
            | Some(serde_json::Value::Object(_))
            | Some(serde_json::Value::Array(_))
            | Some(serde_json::Value::Bool(_))
            | Some(serde_json::Value::Null)
            | None => return Err("writer is required".into()),
        }
        if value_has_private_path(&self.0) {
            return Err("payload contains a private path".into());
        }
        Ok(())
    }
}

fn value_has_private_path(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(text) => text_has_private_path(text),
        serde_json::Value::Array(items) => items.iter().any(value_has_private_path),
        serde_json::Value::Object(map) => map.values().any(value_has_private_path),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            false
        }
    }
}

fn text_has_private_path(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("/home/")
        || lower.contains("/users/")
        || lower.contains("private/")
        || lower.contains("~/")
}

/// Current time on this machine as RFC 3339 with a numeric local offset.
pub fn record_now_local() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Wall clock inside the commit lock: UTC `Z` with exactly six fraction digits.
pub fn record_now_micros_utc() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

#[derive(Debug)]
pub enum RecordSurfaceError {
    Json(String),
    Commit(CommitError),
    Replay(ReplayError),
    Index(LogIndexError),
    Idempotency { key: String },
    MissingLog(String),
}

/// How a unit's keys relate to what the log already holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyCheck {
    /// No key is in the log yet.
    New,
    /// Every event is already in the log, equal on every field except `record_time`.
    Duplicate,
}

impl std::fmt::Display for RecordSurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordSurfaceError::Json(m) => write!(f, "{m}"),
            RecordSurfaceError::Commit(e) => write!(f, "{e}"),
            RecordSurfaceError::Replay(e) => write!(f, "{e}"),
            RecordSurfaceError::Index(e) => write!(f, "{e}"),
            RecordSurfaceError::Idempotency { key } => {
                write!(f, "idempotency conflict on key {key}: already logged with different content, or the unit is only partly logged")
            }
            RecordSurfaceError::MissingLog(m) => write!(f, "missing log: {m}"),
        }
    }
}

impl std::error::Error for RecordSurfaceError {}

fn stamp_unit_v2(
    _log: &Path,
    unit: &mut UnitOfWork<JsonPayload>,
    hlc_state: &mut HlcState,
    seq: &mut u64,
) -> Result<crate::hlc::HlcWire, RecordSurfaceError> {
    let node = configured_node().map_err(RecordSurfaceError::Json)?;
    let writer = configured_writer().map_err(RecordSurfaceError::Json)?;
    let pt = physical_time_us();
    let mut last_hlc = crate::hlc::HlcWire { l: 0, c: 0 };
    for event in &mut unit.events {
        let pw = payload_writer(event).map_err(RecordSurfaceError::Json)?;
        validate_writer_identity(&pw).map_err(RecordSurfaceError::Json)?;
        *seq += 1;
        let hlc = local_append(hlc_state, pt).map_err(RecordSurfaceError::Json)?;
        event.schema_v = SCHEMA_VERSION;
        event.record_time =
            Rfc3339Time::new(record_now_micros_utc()).map_err(RecordSurfaceError::Json)?;
        event.event_time = Rfc3339Time::new(
            to_event_time_utc_z(event.event_time.as_str()).map_err(RecordSurfaceError::Json)?,
        )
        .map_err(RecordSurfaceError::Json)?;
        event.event_time_prec = Some(infer_event_time_prec(event.event_time.as_str()));
        if matches!(event.event_time_prec, Some(EventTimePrec::Unknown)) {
            event.event_time_prec = Some(EventTimePrec::S);
        }
        event.node = Some(node.clone());
        event.seq = Some(*seq);
        event.hlc = Some(hlc);
        last_hlc = hlc;
        event.writer = Some(writer.clone());
        event.depth.get_or_insert(0);
    }
    Ok(last_hlc)
}

fn payload_writer(event: &EventEnvelope<JsonPayload>) -> Result<String, String> {
    match event.payload.0.get("writer") {
        Some(serde_json::Value::String(writer)) => validate_writer_identity(writer),
        _ => Err("writer is required".into()),
    }
}

fn to_event_time_utc_z(raw: &str) -> Result<String, String> {
    let dt = chrono::DateTime::parse_from_rfc3339(raw).map_err(|e| format!("event_time: {e}"))?;
    Ok(dt.with_timezone(&Utc).to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true))
}

fn infer_event_time_prec(t: &str) -> EventTimePrec {
    match t.split('.').nth(1).map(|s| s.trim_end_matches('Z').len()) {
        Some(6) => EventTimePrec::Us,
        Some(3) => EventTimePrec::Ms,
        Some(0) => EventTimePrec::S,
        None => EventTimePrec::S,
        Some(_) => EventTimePrec::Unknown,
    }
}

pub fn commit_json(
    log: &Path,
    unit_json: &str,
    fault: &CommitFault,
) -> Result<CommitResult, RecordSurfaceError> {
    let _lock = ExclusiveLogLock::acquire(log)?;
    let mut unit = parse_unit(unit_json)?;
    let (mut hlc_state, mut last_seq) =
        sidecar_header_state::<JsonPayload>(log).map_err(RecordSurfaceError::Index)?;
    let last_hlc_wire = stamp_unit_v2(log, &mut unit, &mut hlc_state, &mut last_seq)?;
    if check_known_keys(log, &unit)? == KeyCheck::Duplicate {
        return Ok(CommitResult::duplicate());
    }
    #[cfg(test)]
    test_hold();
    let result = Commit::new(log)
        .run(unit.clone(), fault)
        .map_err(RecordSurfaceError::Commit)?;
    if result.phase == CommitPhase::Done && !result.duplicate {
        let lines = encode_unit_lines(&unit.events)?;
        extend_index_lines(log, &lines, &unit.events, last_hlc_wire, last_seq)
            .map_err(RecordSurfaceError::Index)?;
        ring_doorbell(log);
    }
    Ok(result)
}

pub fn recover_json(log: &Path, fault: &CommitFault) -> Result<CommitResult, RecordSurfaceError> {
    let _lock = ExclusiveLogLock::acquire(log)?;
    let result = Commit::new(log)
        .recover::<JsonPayload>(fault)
        .map_err(RecordSurfaceError::Commit)?;
    if result.phase == CommitPhase::Done {
        let _ = rebuild_index::<JsonPayload>(log);
    }
    Ok(result)
}

struct ExclusiveLogLock {
    file: File,
}

impl ExclusiveLogLock {
    fn acquire(log: &Path) -> Result<Self, RecordSurfaceError> {
        if let Some(dir) = log.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| RecordSurfaceError::Commit(CommitError::Io(e.to_string())))?;
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(log_lock_path(log))
            .map_err(|e| RecordSurfaceError::Commit(CommitError::Io(e.to_string())))?;
        lock_exclusive(&file)?;
        Ok(Self { file })
    }
}

impl Drop for ExclusiveLogLock {
    fn drop(&mut self) {
        unlock_exclusive(&self.file);
    }
}

fn log_lock_path(log: &Path) -> PathBuf {
    let mut name = log
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from("events.jsonl"));
    name.push(".lock");
    match log.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(name),
        Some(_) | None => PathBuf::from(name),
    }
}

#[cfg(unix)]
fn lock_exclusive(file: &File) -> Result<(), RecordSurfaceError> {
    // The fd stays open until the guard drops, which is what flock requires.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
    if rc == 0 {
        Ok(())
    } else {
        Err(RecordSurfaceError::Commit(CommitError::Io(
            std::io::Error::last_os_error().to_string(),
        )))
    }
}

#[cfg(not(unix))]
fn lock_exclusive(_file: &File) -> Result<(), RecordSurfaceError> {
    Err(RecordSurfaceError::Commit(CommitError::Io(
        "record log lock requires flock".into(),
    )))
}

#[cfg(unix)]
fn unlock_exclusive(file: &File) {
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
}

#[cfg(not(unix))]
fn unlock_exclusive(_file: &File) {}

pub fn snapshot_json(log: &Path, at: Option<u64>) -> Result<Snapshot, RecordSurfaceError> {
    if !log.is_file() {
        return Err(RecordSurfaceError::MissingLog(log.display().to_string()));
    }
    let mut idx = load_or_rebuild::<JsonPayload>(log).map_err(RecordSurfaceError::Index)?;
    let at_v = at.unwrap_or(idx.max_version);
    if at_v == idx.max_version && idx.event_count > 0 {
        ensure_snapshot_at_max::<JsonPayload>(log, &mut idx).map_err(RecordSurfaceError::Index)?;
        if let Some(ref snap) = idx.snapshot_at_max {
            if snap.at.0 == at_v {
                return Ok(snap.clone());
            }
        }
    }
    let events =
        load_events_up_to_version::<JsonPayload>(log, at_v).map_err(RecordSurfaceError::Index)?;
    RecordView::from_events(events)
        .snapshot_at(Version(at_v))
        .map_err(RecordSurfaceError::Replay)
}

pub fn feed_json(log: &Path, since: u64) -> Result<Feed<JsonPayload>, RecordSurfaceError> {
    if !log.is_file() {
        return Err(RecordSurfaceError::MissingLog(log.display().to_string()));
    }
    let events = load_events_since_version::<JsonPayload>(log, since)
        .map_err(RecordSurfaceError::Index)?;
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

fn encode_unit_lines(
    events: &[EventEnvelope<JsonPayload>],
) -> Result<Vec<String>, RecordSurfaceError> {
    events
        .iter()
        .map(|event| {
            let line = encode_line(SCHEMA_VERSION, event).map_err(RecordSurfaceError::Json)?;
            Ok(format!("{line}\n"))
        })
        .collect()
}

#[cfg(test)]
fn test_hold() {
    let Ok(raw) = std::env::var("RECORD_COMMIT_HOLD_MS") else {
        return;
    };
    let Ok(ms) = raw.parse::<u64>() else {
        return;
    };
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

/// Sort a unit against the log by (aggregate, key).
///
/// - no key logged: [`KeyCheck::New`]
/// - every event logged and equal except `record_time`: [`KeyCheck::Duplicate`]
///   (the MCP server stamps a fresh `record_time` on each call, so a retry differs there)
/// - anything else, including a key repeated inside the unit: an idempotency conflict
fn check_known_keys(
    log: &Path,
    unit: &UnitOfWork<JsonPayload>,
) -> Result<KeyCheck, RecordSurfaceError> {
    let idx = if log.is_file() {
        Some(load_or_rebuild::<JsonPayload>(log).map_err(RecordSurfaceError::Index)?)
    } else {
        None
    };
    let mut in_unit: BTreeSet<(String, String)> = BTreeSet::new();
    let mut known = 0usize;
    for event in &unit.events {
        let id = (event.aggregate.0.clone(), event.key.0.clone());
        if !in_unit.insert(id.clone()) {
            return Err(RecordSurfaceError::Idempotency { key: id.1 });
        }
        if let Some(ref index) = idx {
            if let Some(entry) =
                lookup_aggregate_key_entry(index, &event.aggregate.0, &event.key.0)
            {
                let line = read_line_at(log, entry.offset, entry.len)
                    .map_err(RecordSurfaceError::Index)?;
                let prior = crate::decode_line::<EventEnvelope<JsonPayload>>(&line)
                    .map_err(|e| RecordSurfaceError::Json(e))?
                    .value;
                if !same_idempotent_content(&prior, event) {
                    return Err(RecordSurfaceError::Idempotency { key: id.1 });
                }
                known += 1;
            }
        }
    }
    match known {
        0 => Ok(KeyCheck::New),
        n if n == unit.events.len() => Ok(KeyCheck::Duplicate),
        _ => {
            let key = unit
                .events
                .iter()
                .find(|e| {
                    idx.as_ref().is_some_and(|index| {
                        lookup_aggregate_key_entry(index, &e.aggregate.0, &e.key.0).is_some()
                    })
                })
                .map(|e| e.key.0.clone())
                .unwrap_or_default();
            Err(RecordSurfaceError::Idempotency { key })
        }
    }
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
    fn scenario_a_commit_snapshot_equals_replay_and_dedupes_same_key() {
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
        let again = commit_json(&log, SCENARIO_A, &CommitFault::default()).unwrap();
        assert_eq!(again, CommitResult::duplicate());
        assert_eq!(nonempty_lines(&log), 3);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn same_key_different_payload_conflicts() {
        let (dir, log) = temp_log();
        let first = one_event(
            "k-1",
            "01JCV1Q8Z00000000000000000",
            r#"{"writer":"laptop-1","note":"a"}"#,
        );
        let edited = one_event(
            "k-1",
            "01JCV1Q8Z00000000000000000",
            r#"{"writer":"laptop-1","note":"b"}"#,
        );
        commit_json(&log, &first, &CommitFault::default()).unwrap();
        let err = commit_json(&log, &edited, &CommitFault::default()).unwrap_err();
        assert!(err.to_string().contains("idempotency conflict"), "{err}");
        assert_eq!(nonempty_lines(&log), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn partial_duplicate_unit_conflicts() {
        let (dir, log) = temp_log();
        let first = one_event(
            "k-1",
            "01JCV1Q8Z00000000000000000",
            r#"{"writer":"laptop-1"}"#,
        );
        commit_json(&log, &first, &CommitFault::default()).unwrap();
        let mut events: Vec<serde_json::Value> = serde_json::from_str(&first).unwrap();
        let mut second: serde_json::Value = events[0].clone();
        second["id"] = "01JCV1Q8Z10000000000000000".into();
        second["key"] = "k-2".into();
        second["version"] = 2.into();
        events.push(second);
        let unit = serde_json::to_string(&events).unwrap();
        let err = commit_json(&log, &unit, &CommitFault::default()).unwrap_err();
        assert!(err.to_string().contains("idempotency conflict"), "{err}");
        assert_eq!(nonempty_lines(&log), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn duplicate_ignores_record_time() {
        let (dir, log) = temp_log();
        let first = one_event(
            "k-1",
            "01JCV1Q8Z00000000000000000",
            r#"{"writer":"laptop-1"}"#,
        );
        commit_json(&log, &first, &CommitFault::default()).unwrap();
        let retry = first.replace("2026-10-26T19:00:00+01:00", "2026-10-27T08:00:00+01:00");
        assert_ne!(first, retry);
        let res = commit_json(&log, &retry, &CommitFault::default()).unwrap();
        assert!(res.duplicate);
        assert_eq!(nonempty_lines(&log), 1);
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
            "payload": {"writer": "not/safe", "note": "no"}
        }]"#;
        let err = commit_json(&log, unit, &CommitFault::default()).unwrap_err();
        assert!(err.to_string().contains("not a safe identity"), "{err}");
    }

    fn one_event(key: &str, id: &str, payload: &str) -> String {
        format!(
            r#"[{{
                "id": "{id}",
                "aggregate": "agg://example.test/acme-north/staff/1",
                "key": "{key}",
                "version": 1,
                "event_time": "2026-10-26T18:59:00+01:00",
                "record_time": "2026-10-26T19:00:00+01:00",
                "source": "owner",
                "schema_v": 1,
                "payload": {payload}
            }}]"#
        )
    }

    #[test]
    fn writer_must_be_a_public_host_alias_string() {
        let (_dir, log) = temp_log();
        for payload in [
            r#"{"note":"missing"}"#,
            r#"{"writer":7}"#,
            r#"{"writer":{"h":"laptop-1"}}"#,
            r#""text""#,
        ] {
            let err = commit_json(
                &log,
                &one_event("writer-shape", "01JCV1Q8Z00000000000000000", payload),
                &CommitFault::default(),
            )
            .unwrap_err();
            assert!(
                err.to_string().contains("writer is required"),
                "{payload} {err}"
            );
        }
        let leaked = format!(r#"{{"writer":"laptop-1","note":"/{}/example"}}"#, "home");
        let err = commit_json(
            &log,
            &one_event("writer-leak", "01JCV1Q8Z00000000000000000", &leaked),
            &CommitFault::default(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("private path"), "{err}");
    }

    #[test]
    fn concurrent_commits_both_land() {
        if std::env::var_os("RECORD_COMMIT_WORKER").is_some() {
            let log = PathBuf::from(std::env::var("RECORD_COMMIT_LOG").unwrap());
            let unit = std::env::var("RECORD_COMMIT_UNIT").unwrap();
            commit_json(&log, &unit, &CommitFault::default()).unwrap();
            return;
        }
        let (dir, log) = temp_log();
        let exe = std::env::current_exe().unwrap();
        let mut children = Vec::new();
        for (key, id) in [
            ("key-a", "01JCV1Q8ZA000000000000000A"),
            ("key-b", "01JCV1Q8ZB000000000000000B"),
        ] {
            let unit = one_event(key, id, r#"{"writer":"laptop-1","note":"concurrent"}"#);
            children.push(
                std::process::Command::new(&exe)
                    .arg("surface::tests::concurrent_commits_both_land")
                    .arg("--exact")
                    .env("RECORD_COMMIT_WORKER", "1")
                    .env("RECORD_COMMIT_HOLD_MS", "400")
                    .env("RECORD_COMMIT_LOG", &log)
                    .env("RECORD_COMMIT_UNIT", unit)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .unwrap(),
            );
        }
        for child in children {
            let out = child.wait_with_output().unwrap();
            assert!(
                out.status.success(),
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("key-a"), "{text}");
        assert!(text.contains("key-b"), "{text}");
        assert_eq!(nonempty_lines(&log), 2);
        let _ = std::fs::remove_dir_all(dir);
    }
}
