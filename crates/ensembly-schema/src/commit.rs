//! Domain-agnostic commit boundary: validate → append → fsync → verify.

use crate::record::EventEnvelope;
use crate::{encode_line, Checked, Decoded, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommitPhase {
    Begin,
    Validate,
    Append,
    Fsync,
    Verify,
    Done,
    Abort,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitOfWork<P> {
    pub events: Vec<EventEnvelope<P>>,
}

#[derive(Debug, Clone, Default)]
pub struct CommitFault {
    /// Stop immediately after entering this phase (simulates crash before phase work finishes).
    pub crash_entering: Option<CommitPhase>,
    /// During Append, write this many bytes of the next line then abort (partial line).
    pub append_abort_after_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitResult {
    pub phase: CommitPhase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitError {
    Phase(CommitPhase, String),
    Io(String),
}

impl std::fmt::Display for CommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitError::Phase(p, m) => write!(f, "{p:?}: {m}"),
            CommitError::Io(m) => write!(f, "io: {m}"),
        }
    }
}

impl CommitPhase {
    fn as_str(self) -> &'static str {
        match self {
            CommitPhase::Begin => "begin",
            CommitPhase::Validate => "validate",
            CommitPhase::Append => "append",
            CommitPhase::Fsync => "fsync",
            CommitPhase::Verify => "verify",
            CommitPhase::Done => "done",
            CommitPhase::Abort => "abort",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "begin" => Some(CommitPhase::Begin),
            "validate" => Some(CommitPhase::Validate),
            "append" => Some(CommitPhase::Append),
            "fsync" => Some(CommitPhase::Fsync),
            "verify" => Some(CommitPhase::Verify),
            "done" => Some(CommitPhase::Done),
            "abort" => Some(CommitPhase::Abort),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    phase: String,
    unit_lines: Vec<String>,
}

pub struct Commit {
    log_path: PathBuf,
}

impl Commit {
    pub fn new(log_path: impl Into<PathBuf>) -> Self {
        Self {
            log_path: log_path.into(),
        }
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    pub fn run<P>(
        &self,
        unit: UnitOfWork<P>,
        fault: &CommitFault,
    ) -> Result<CommitResult, CommitError>
    where
        P: Checked + Serialize + for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        self.run_from(CommitPhase::Begin, unit, fault)
    }

    /// Resume after crash when checkpoint says we landed on Verify (post-fsync).
    pub fn recover<P>(&self, fault: &CommitFault) -> Result<CommitResult, CommitError>
    where
        P: Checked + Serialize + for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let ck = load_checkpoint(&self.checkpoint_path())?;
        let phase = CommitPhase::from_str(&ck.phase).ok_or_else(|| {
            CommitError::Phase(
                CommitPhase::Abort,
                format!("unknown checkpoint phase {}", ck.phase),
            )
        })?;
        if phase != CommitPhase::Verify {
            self.cleanup_staging()?;
            clear_checkpoint(&self.checkpoint_path())?;
            return Ok(CommitResult {
                phase: CommitPhase::Abort,
            });
        }
        let unit = unit_from_lines::<P>(&ck.unit_lines)?;
        self.run_from(CommitPhase::Verify, unit, fault)
    }

    fn run_from<P>(
        &self,
        start: CommitPhase,
        unit: UnitOfWork<P>,
        fault: &CommitFault,
    ) -> Result<CommitResult, CommitError>
    where
        P: Checked + Serialize + for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let mut phase = start;
        loop {
            if fault.crash_entering == Some(phase) {
                return self.abort_cleanup(phase);
            }
            phase = match phase {
                CommitPhase::Begin => {
                    write_checkpoint(
                        &self.checkpoint_path(),
                        CommitPhase::Begin,
                        &encode_unit(&unit.events)?,
                    )?;
                    CommitPhase::Validate
                }
                CommitPhase::Validate => {
                    validate_unit(&unit)?;
                    CommitPhase::Append
                }
                CommitPhase::Append => {
                    self.append_staging(&unit, fault)?;
                    CommitPhase::Fsync
                }
                CommitPhase::Fsync => {
                    let lines = encode_unit(&unit.events)?;
                    self.fsync_unit(&lines)?;
                    write_checkpoint(&self.checkpoint_path(), CommitPhase::Verify, &lines)?;
                    if fault.crash_entering == Some(CommitPhase::Verify) {
                        return Ok(CommitResult {
                            phase: CommitPhase::Verify,
                        });
                    }
                    CommitPhase::Verify
                }
                CommitPhase::Verify => {
                    let lines = encode_unit(&unit.events)?;
                    verify_log_tail::<P>(self.log_path(), &lines)?;
                    clear_checkpoint(&self.checkpoint_path())?;
                    return Ok(CommitResult {
                        phase: CommitPhase::Done,
                    });
                }
                CommitPhase::Done | CommitPhase::Abort => {
                    return Ok(CommitResult { phase });
                }
            };
        }
    }

    fn checkpoint_path(&self) -> PathBuf {
        let dir = self.log_path.parent().unwrap_or_else(|| Path::new("."));
        let name = self
            .log_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "events.jsonl".into());
        dir.join(format!(".{name}.commit-state.json"))
    }

    fn staging_path(&self) -> PathBuf {
        let dir = self.log_path.parent().unwrap_or_else(|| Path::new("."));
        let name = self
            .log_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "events.jsonl".into());
        dir.join(format!(".{}.staging-{}", name, std::process::id()))
    }

    fn append_staging<P>(
        &self,
        unit: &UnitOfWork<P>,
        fault: &CommitFault,
    ) -> Result<(), CommitError>
    where
        P: Checked + Serialize,
    {
        self.cleanup_staging()?;
        let staging = self.staging_path();
        if let Some(dir) = staging.parent() {
            std::fs::create_dir_all(dir).map_err(|e| CommitError::Io(e.to_string()))?;
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|e| CommitError::Io(e.to_string()))?;
        for event in &unit.events {
            let line = encode_line(SCHEMA_VERSION, event)
                .map_err(|e| CommitError::Phase(CommitPhase::Append, e))?;
            let bytes = format!("{line}\n");
            if let Some(n) = fault.append_abort_after_bytes {
                let take = n.min(bytes.len());
                file.write_all(&bytes.as_bytes()[..take])
                    .map_err(|e| CommitError::Io(e.to_string()))?;
                let _ = file.sync_all();
                let _ = std::fs::remove_file(&staging);
                clear_checkpoint(&self.checkpoint_path()).ok();
                return Err(CommitError::Phase(
                    CommitPhase::Append,
                    "simulated crash mid-line".into(),
                ));
            }
            file.write_all(bytes.as_bytes())
                .map_err(|e| CommitError::Io(e.to_string()))?;
        }
        file.sync_all()
            .map_err(|e| CommitError::Io(e.to_string()))?;
        Ok(())
    }

    fn fsync_unit(&self, _lines: &[String]) -> Result<(), CommitError> {
        let staging = self.staging_path();
        let staging_body = std::fs::read_to_string(&staging)
            .map_err(|e| CommitError::Phase(CommitPhase::Fsync, e.to_string()))?;
        let mut combined = read_log(&self.log_path)?;
        if !combined.is_empty() && !combined.ends_with('\n') {
            return Err(CommitError::Phase(
                CommitPhase::Fsync,
                "log ends with partial line".into(),
            ));
        }
        combined.push_str(&staging_body);
        write_atomic(&self.log_path, combined.as_bytes()).map_err(CommitError::Io)?;
        self.cleanup_staging()?;
        Ok(())
    }

    fn cleanup_staging(&self) -> Result<(), CommitError> {
        let staging = self.staging_path();
        match std::fs::remove_file(&staging) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CommitError::Io(e.to_string())),
        }
    }

    fn abort_cleanup(&self, _phase: CommitPhase) -> Result<CommitResult, CommitError> {
        let _ = self.cleanup_staging();
        let _ = clear_checkpoint(&self.checkpoint_path());
        Ok(CommitResult {
            phase: CommitPhase::Abort,
        })
    }
}

fn encode_unit<P: Serialize>(events: &[EventEnvelope<P>]) -> Result<Vec<String>, CommitError> {
    events
        .iter()
        .map(|e| {
            encode_line(SCHEMA_VERSION, e).map_err(|e| CommitError::Phase(CommitPhase::Validate, e))
        })
        .collect()
}

fn validate_unit<P: Checked>(unit: &UnitOfWork<P>) -> Result<(), CommitError> {
    if unit.events.is_empty() {
        return Err(CommitError::Phase(
            CommitPhase::Validate,
            "unit must contain at least one event".into(),
        ));
    }
    for event in &unit.events {
        event
            .check()
            .map_err(|e| CommitError::Phase(CommitPhase::Validate, e))?;
    }
    Ok(())
}

fn unit_from_lines<P>(lines: &[String]) -> Result<UnitOfWork<P>, CommitError>
where
    P: Checked + for<'de> serde::Deserialize<'de>,
{
    use crate::decode_line;
    let events = lines
        .iter()
        .map(|line| {
            decode_line::<EventEnvelope<P>>(line)
                .map(|Decoded { value, .. }| value)
                .map_err(|e| CommitError::Phase(CommitPhase::Verify, e))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(UnitOfWork { events })
}

fn verify_log_tail<P>(path: &Path, expected: &[String]) -> Result<(), CommitError>
where
    P: Checked + for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    use crate::decode_line;
    let body = read_log(path)?;
    let mut lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() < expected.len() {
        return Err(CommitError::Phase(
            CommitPhase::Verify,
            "log shorter than unit".into(),
        ));
    }
    let tail: Vec<String> = lines
        .drain(lines.len() - expected.len()..)
        .map(|s| s.to_string())
        .collect();
    if tail != expected {
        return Err(CommitError::Phase(
            CommitPhase::Verify,
            "tail mismatch".into(),
        ));
    }
    for line in &tail {
        decode_line::<EventEnvelope<P>>(line)
            .map_err(|e| CommitError::Phase(CommitPhase::Verify, e))?;
    }
    Ok(())
}

fn read_log(path: &Path) -> Result<String, CommitError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(CommitError::Io(e.to_string())),
    }
}

fn write_checkpoint(path: &Path, phase: CommitPhase, lines: &[String]) -> Result<(), CommitError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| CommitError::Io(e.to_string()))?;
    }
    let ck = Checkpoint {
        phase: phase.as_str().into(),
        unit_lines: lines.to_vec(),
    };
    let bytes = serde_json::to_string(&ck).map_err(|e| CommitError::Io(e.to_string()))?;
    write_atomic(path, bytes.as_bytes()).map_err(CommitError::Io)
}

fn load_checkpoint(path: &Path) -> Result<Checkpoint, CommitError> {
    let body = std::fs::read_to_string(path).map_err(|e| CommitError::Io(e.to_string()))?;
    serde_json::from_str(&body).map_err(|e| CommitError::Io(e.to_string()))
}

fn clear_checkpoint(path: &Path) -> Result<(), CommitError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CommitError::Io(e.to_string())),
    }
}

/// Replace `path` atomically (temp in same dir, sync, rename). Mirrors kernel ledger pattern.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| format!("no file name in {}", path.display()))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = dir.join(format!(
        ".{}.tmp-{}-{nanos}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| -> Result<(), String> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{AggregateId, EventId, IdempotencyKey, Source, StockholmTime, Version};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ScenarioPayload {
        writer: String,
        note: String,
    }

    impl Checked for ScenarioPayload {
        fn check(&self) -> Result<(), String> {
            if !matches!(self.writer.as_str(), "laptop-1" | "laptop-2" | "mac-mini") {
                return Err("writer must be a public host alias".into());
            }
            Ok(())
        }
    }

    fn t(s: &str) -> StockholmTime {
        StockholmTime::new(s).unwrap()
    }

    fn event(n: u64, writer: &str, key: &str) -> EventEnvelope<ScenarioPayload> {
        let id = match n {
            1 => "01JCV1Q8Z00000000000000000",
            2 => "01JCV1Q8Z10000000000000000",
            10 => "01JCV1Q8Z20000000000000000",
            11 => "01JCV1Q8Z30000000000000000",
            _ => "01JCV1Q8Z40000000000000000",
        };
        EventEnvelope {
            id: EventId(id.parse().unwrap()),
            aggregate: AggregateId(format!("agg://example.test/acme-north/{n}")),
            key: IdempotencyKey(key.into()),
            version: Version(n),
            event_time: t("2026-10-26T18:59:00+01:00"),
            record_time: t("2026-10-26T19:00:00+01:00"),
            source: Source::Owner,
            prev_hash: None,
            schema_v: SCHEMA_VERSION,
            payload: ScenarioPayload {
                writer: writer.into(),
                note: "scenario-d".into(),
            },
        }
    }

    fn temp_log() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "ensembly-record-commit-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("events.jsonl")
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
    fn crash_mid_append_leaves_no_partial_line() {
        let log = temp_log();
        let dir = log.parent().unwrap();
        let commit = Commit::new(&log);
        let unit = UnitOfWork {
            events: vec![event(1, "laptop-1", "unit-d-1")],
        };
        let fault = CommitFault {
            append_abort_after_bytes: Some(10),
            ..Default::default()
        };
        assert!(matches!(
            commit.run(unit, &fault),
            Err(CommitError::Phase(CommitPhase::Append, _))
        ));
        assert!(!log.exists());
        assert!(!dir_has_partial_jsonl_line(dir));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fsync_then_crash_recovers_at_verify() {
        let log = temp_log();
        let dir = log.parent().unwrap();
        let commit = Commit::new(&log);
        let unit = UnitOfWork {
            events: vec![
                event(1, "laptop-1", "unit-d-a"),
                event(2, "laptop-2", "unit-d-b"),
            ],
        };
        let fault = CommitFault {
            crash_entering: Some(CommitPhase::Verify),
            ..Default::default()
        };
        let mid = commit.run(unit, &fault).unwrap();
        assert_eq!(mid.phase, CommitPhase::Verify);
        assert!(log.exists());
        let done = commit
            .recover::<ScenarioPayload>(&CommitFault::default())
            .unwrap();
        assert_eq!(done.phase, CommitPhase::Done);
        let body = std::fs::read_to_string(&log).unwrap();
        assert_eq!(body.lines().filter(|l| !l.trim().is_empty()).count(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn all_or_none_multi_event_unit() {
        let log = temp_log();
        let dir = log.parent().unwrap();
        let commit = Commit::new(&log);
        let unit = UnitOfWork {
            events: vec![
                event(10, "mac-mini", "unit-none-a"),
                event(11, "mac-mini", "unit-none-b"),
            ],
        };
        let fault = CommitFault {
            crash_entering: Some(CommitPhase::Validate),
            ..Default::default()
        };
        let aborted = commit.run(unit.clone(), &fault).unwrap();
        assert_eq!(aborted.phase, CommitPhase::Abort);
        assert!(!log.exists());

        let done = commit.run(unit, &CommitFault::default()).unwrap();
        assert_eq!(done.phase, CommitPhase::Done);
        assert_eq!(
            std::fs::read_to_string(&log)
                .unwrap()
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count(),
            2
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn scenario_d_fixture_unit_shape() {
        let fixture = include_str!("../fixtures/scenarios/d-crash-unit.json");
        let unit: UnitOfWork<ScenarioPayload> =
            serde_json::from_str(fixture).expect("scenario d fixture");
        assert_eq!(unit.events.len(), 2);
        validate_unit(&unit).unwrap();
    }
}
