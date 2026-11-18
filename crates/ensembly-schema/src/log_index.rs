//! Rebuildable on-disk sidecar for the record event log.
//!
//! Derived data only: safe to delete; rebuilt from the log when missing or stale.
//! `keys_by_idempotency` is reserved for global ingest dedupe (empty in slice 2).

use crate::hlc::{HlcState, HlcWire};
use crate::record::{EventEnvelope, Version};
use crate::replay::{parse_event_log, replay, ReplayError};
use crate::view::{facts_from_rows, Snapshot};
use crate::{decode_line, Checked};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const INDEX_SCHEMA_V: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeysHeader {
    pub log_len: u64,
    pub last_line_sha256: String,
    pub last_seq: u64,
    pub last_hlc: HlcWire,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogIndexSidecar {
    pub schema_v: u32,
    pub header: KeysHeader,
    pub event_count: u64,
    pub max_version: u64,
    /// `(aggregate \\0 idempotency_key)` → line location (today's uniqueness scope).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keys_by_aggregate: BTreeMap<String, KeyEntry>,
    /// Global idempotency key → line location (slice-2+: empty; ready for bus ingest dedupe).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keys_by_idempotency: BTreeMap<String, KeyEntry>,
    pub events: Vec<EventEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_at_max: Option<Snapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEntry {
    pub offset: u64,
    pub len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEntry {
    pub offset: u64,
    pub len: u32,
    pub version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

#[derive(Debug)]
pub enum LogIndexError {
    Io(String),
    Replay(ReplayError),
    Tail(String),
}

impl std::fmt::Display for LogIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogIndexError::Io(m) => write!(f, "io: {m}"),
            LogIndexError::Replay(e) => write!(f, "{e}"),
            LogIndexError::Tail(m) => write!(f, "tail: {m}"),
        }
    }
}

impl std::error::Error for LogIndexError {}

pub fn sidecar_path(log: &Path) -> PathBuf {
    let mut name = log
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from("events.jsonl"));
    name.push(".keys");
    match log.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(name),
        Some(_) | None => PathBuf::from(name),
    }
}

pub fn torn_fragment_path(log: &Path) -> PathBuf {
    let mut name = log
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from("events.jsonl"));
    name.push(".torn-fragment");
    match log.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(name),
        Some(_) | None => PathBuf::from(name),
    }
}

fn map_aggregate_key(aggregate: &str, key: &str) -> String {
    format!("{aggregate}\0{key}")
}

pub fn load_or_rebuild<P>(log: &Path) -> Result<LogIndexSidecar, LogIndexError>
where
    P: Checked + Serialize + for<'de> Deserialize<'de> + Clone,
{
    let idx_path = sidecar_path(log);
    let (log_len, tail_sha256) = log_metadata(log)?;
    if let Ok(idx) = read_sidecar(&idx_path) {
        if idx_matches_log(&idx, log_len, &tail_sha256) {
            return Ok(idx);
        }
    }
    rebuild_index::<P>(log)
}

fn log_metadata(log: &Path) -> Result<(u64, String), LogIndexError> {
    let len = match log.metadata() {
        Ok(m) => m.len(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((0, sha256_hex(b""))),
        Err(e) => return Err(LogIndexError::Io(e.to_string())),
    };
    if len == 0 {
        return Ok((0, sha256_hex(b"")));
    }
    let last = read_last_complete_line_bytes(log)?;
    Ok((len, sha256_hex(&last)))
}

fn idx_matches_log(idx: &LogIndexSidecar, len: u64, tail_sha256: &str) -> bool {
    idx.schema_v == INDEX_SCHEMA_V
        && idx.header.log_len == len
        && idx.header.last_line_sha256 == tail_sha256
}

pub fn rebuild_index<P>(log: &Path) -> Result<LogIndexSidecar, LogIndexError>
where
    P: Checked + Serialize + for<'de> Deserialize<'de> + Clone,
{
    let bytes = match fs::read(log) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(LogIndexError::Io(e.to_string())),
    };
    ensure_tail_intact(log, Some(&bytes))?;
    let mut idx = scan_bytes::<P>(&bytes)?;
    if !bytes.is_empty() {
        idx.snapshot_at_max = Some(snapshot_for_events::<P>(
            load_events_from_bytes::<P>(&bytes)?,
            idx.max_version,
        )?);
    }
    write_sidecar(log, &idx)?;
    Ok(idx)
}

fn load_events_from_bytes<P>(bytes: &[u8]) -> Result<Vec<EventEnvelope<P>>, LogIndexError>
where
    P: Checked + for<'de> Deserialize<'de> + Clone,
{
    let text = std::str::from_utf8(bytes).map_err(|e| LogIndexError::Io(e.to_string()))?;
    parse_event_log(text).map_err(LogIndexError::Replay)
}

fn snapshot_for_events<P>(
    events: Vec<EventEnvelope<P>>,
    max_version: u64,
) -> Result<Snapshot, LogIndexError>
where
    P: Checked + Serialize + Clone + for<'de> Deserialize<'de>,
{
    let filtered: Vec<_> = events
        .into_iter()
        .filter(|e| e.version.0 <= max_version)
        .collect();
    let rows = replay(&filtered).map_err(LogIndexError::Replay)?;
    Ok(Snapshot {
        at: Version(max_version),
        facts: facts_from_rows(&rows),
    })
}

fn scan_bytes<P>(bytes: &[u8]) -> Result<LogIndexSidecar, LogIndexError>
where
    P: Checked + for<'de> Deserialize<'de>,
{
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let mut keys_by_aggregate = BTreeMap::new();
    let mut events = Vec::new();
    let mut max_version = 0u64;
    let mut offset = 0u64;
    let mut tail_sha256 = sha256_hex(b"");
    let mut event_count = 0u64;
    let mut last_seq = 0u64;
    let mut last_hlc = HlcWire { l: 0, c: 0 };
    let mut line_index = 0u64;
    for line in bytes.split(|&b| b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let line_str = std::str::from_utf8(line).map_err(|e| LogIndexError::Io(e.to_string()))?;
        let decoded =
            decode_line::<EventEnvelope<P>>(line_str).map_err(|e| LogIndexError::Tail(e))?;
        let event = decoded.value;
        let line_len = u32::try_from(line.len().saturating_add(1))
            .map_err(|_| LogIndexError::Tail("line too long".into()))?;
        let entry = KeyEntry {
            offset,
            len: line_len,
        };
        keys_by_aggregate.insert(
            map_aggregate_key(&event.aggregate.0, &event.key.0),
            entry.clone(),
        );
        events.push(EventEntry {
            offset,
            len: line_len,
            version: event.version.0,
            idempotency_key: Some(event.key.0.clone()),
        });
        max_version = max_version.max(event.version.0);
        event_count += 1;
        line_index += 1;
        last_seq = event.seq.unwrap_or(line_index);
        if let Some(hlc) = event.hlc {
            last_hlc = hlc;
        }
        tail_sha256 = sha256_hex(line);
        offset = offset.saturating_add(u64::from(line_len));
    }
    Ok(LogIndexSidecar {
        schema_v: INDEX_SCHEMA_V,
        header: KeysHeader {
            log_len: len,
            last_line_sha256: tail_sha256,
            last_seq,
            last_hlc,
        },
        event_count,
        max_version,
        keys_by_aggregate,
        keys_by_idempotency: BTreeMap::new(),
        events,
        snapshot_at_max: None,
    })
}

pub fn sidecar_header_state<P>(log: &Path) -> Result<(HlcState, u64), LogIndexError>
where
    P: Checked + Serialize + for<'de> Deserialize<'de> + Clone,
{
    if !log.is_file() {
        return Ok((HlcState::default(), 0));
    }
    let idx = load_or_rebuild::<P>(log)?;
    Ok((
        HlcState {
            l: idx.header.last_hlc.l,
            c: idx.header.last_hlc.c,
        },
        idx.header.last_seq,
    ))
}

pub fn extend_index_lines<P>(
    log: &Path,
    new_lines: &[String],
    new_events: &[EventEnvelope<P>],
    last_hlc: HlcWire,
    last_seq: u64,
) -> Result<(), LogIndexError>
where
    P: Checked + Serialize + for<'de> Deserialize<'de> + Clone,
{
    let mut idx = load_or_rebuild::<P>(log)?;
    let mut offset = idx.header.log_len;
    for (event, line) in new_events.iter().zip(new_lines) {
        let nbytes = line.as_bytes();
        let line_len =
            u32::try_from(nbytes.len()).map_err(|_| LogIndexError::Tail("line too long".into()))?;
        let entry = KeyEntry {
            offset,
            len: line_len,
        };
        idx.keys_by_aggregate
            .insert(map_aggregate_key(&event.aggregate.0, &event.key.0), entry);
        idx.events.push(EventEntry {
            offset,
            len: line_len,
            version: event.version.0,
            idempotency_key: Some(event.key.0.clone()),
        });
        idx.max_version = idx.max_version.max(event.version.0);
        idx.event_count += 1;
        let line_body = nbytes.strip_suffix(b"\n").unwrap_or(nbytes);
        idx.header.last_line_sha256 = sha256_hex(line_body);
        offset = offset.saturating_add(u64::from(line_len));
    }
    idx.header.log_len = offset;
    idx.header.last_hlc = last_hlc;
    idx.header.last_seq = last_seq;
    idx.snapshot_at_max = None;
    write_sidecar(log, &idx)
}

#[cfg(unix)]
pub fn ring_doorbell(log: &Path) {
    use std::os::unix::net::UnixDatagram;
    let sock = format!("{}.doorbell", sidecar_path(log).display());
    if let Ok(sender) = UnixDatagram::unbound() {
        let _ = sender.send_to(&[0u8], sock);
    }
}

#[cfg(not(unix))]
pub fn ring_doorbell(_log: &Path) {}

pub fn ensure_tail_intact(log: &Path, bytes: Option<&[u8]>) -> Result<(), LogIndexError> {
    let data = match bytes {
        Some(b) => b.to_vec(),
        None => match fs::read(log) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(LogIndexError::Io(e.to_string())),
        },
    };
    if data.is_empty() {
        return Ok(());
    }
    if data.last() != Some(&b'\n') {
        write_torn_diagnostic(log, &data)?;
        return Err(LogIndexError::Tail(format!(
            "log ends with a torn line; appended writes refused (diagnostic: {})",
            torn_fragment_path(log).display()
        )));
    }
    let text = std::str::from_utf8(&data).map_err(|e| LogIndexError::Io(e.to_string()))?;
    if let Some(last) = text.lines().filter(|l| !l.trim().is_empty()).last() {
        serde_json::from_str::<serde_json::Value>(last)
            .map_err(|e| LogIndexError::Tail(format!("last line is not valid json: {e}")))?;
    }
    Ok(())
}

fn write_torn_diagnostic(log: &Path, data: &[u8]) -> Result<(), LogIndexError> {
    let diag = torn_fragment_path(log);
    if let Some(dir) = diag.parent() {
        fs::create_dir_all(dir).map_err(|e| LogIndexError::Io(e.to_string()))?;
    }
    let mut start = data.len();
    while start > 0 && data[start - 1] != b'\n' {
        start -= 1;
    }
    let fragment = &data[start..];
    fs::write(&diag, fragment).map_err(|e| LogIndexError::Io(e.to_string()))?;
    Ok(())
}

fn read_last_complete_line_bytes(log: &Path) -> Result<Vec<u8>, LogIndexError> {
    let data = fs::read(log).map_err(|e| LogIndexError::Io(e.to_string()))?;
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if data.last() != Some(&b'\n') {
        return Err(LogIndexError::Tail("log ends without newline".into()));
    }
    let mut end = data.len().saturating_sub(1);
    while end > 0 && data[end - 1] != b'\n' {
        end -= 1;
    }
    Ok(data[end..data.len().saturating_sub(1)].to_vec())
}

pub fn read_line_at(log: &Path, offset: u64, len: u32) -> Result<String, LogIndexError> {
    let mut file = File::open(log).map_err(|e| LogIndexError::Io(e.to_string()))?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| LogIndexError::Io(e.to_string()))?;
    let mut buf = vec![0u8; len as usize];
    file.read_exact(&mut buf)
        .map_err(|e| LogIndexError::Io(e.to_string()))?;
    let line = buf.strip_suffix(b"\n").unwrap_or(&buf);
    String::from_utf8(line.to_vec()).map_err(|e| LogIndexError::Io(e.to_string()))
}

pub fn load_events_indexed<P>(log: &Path) -> Result<Vec<EventEnvelope<P>>, LogIndexError>
where
    P: Checked + Serialize + for<'de> Deserialize<'de> + Clone,
{
    let idx = load_or_rebuild::<P>(log)?;
    load_events_from_index::<P>(log, &idx)
}

pub fn load_events_up_to_version<P>(
    log: &Path,
    at: u64,
) -> Result<Vec<EventEnvelope<P>>, LogIndexError>
where
    P: Checked + Serialize + for<'de> Deserialize<'de> + Clone,
{
    let idx = load_or_rebuild::<P>(log)?;
    let mut events = Vec::new();
    for entry in idx.events.iter().filter(|e| e.version <= at) {
        let line = read_line_at(log, entry.offset, entry.len)?;
        let decoded = decode_line::<EventEnvelope<P>>(&line).map_err(|e| LogIndexError::Tail(e))?;
        events.push(decoded.value);
    }
    Ok(events)
}

pub fn load_events_since_version<P>(
    log: &Path,
    since: u64,
) -> Result<Vec<EventEnvelope<P>>, LogIndexError>
where
    P: Checked + Serialize + for<'de> Deserialize<'de> + Clone,
{
    let idx = load_or_rebuild::<P>(log)?;
    let mut events = Vec::new();
    for entry in idx.events.iter().filter(|e| e.version > since) {
        let line = read_line_at(log, entry.offset, entry.len)?;
        let decoded = decode_line::<EventEnvelope<P>>(&line).map_err(|e| LogIndexError::Tail(e))?;
        events.push(decoded.value);
    }
    Ok(events)
}

fn load_events_from_index<P>(
    log: &Path,
    idx: &LogIndexSidecar,
) -> Result<Vec<EventEnvelope<P>>, LogIndexError>
where
    P: Checked + for<'de> Deserialize<'de>,
{
    let mut events = Vec::with_capacity(idx.events.len());
    for entry in &idx.events {
        let line = read_line_at(log, entry.offset, entry.len)?;
        let decoded = decode_line::<EventEnvelope<P>>(&line).map_err(|e| LogIndexError::Tail(e))?;
        events.push(decoded.value);
    }
    Ok(events)
}

pub fn lookup_aggregate_key_entry<'a>(
    idx: &'a LogIndexSidecar,
    aggregate: &str,
    key: &str,
) -> Option<&'a KeyEntry> {
    idx.keys_by_aggregate
        .get(&map_aggregate_key(aggregate, key))
}

pub fn ensure_snapshot_at_max<P>(log: &Path, idx: &mut LogIndexSidecar) -> Result<(), LogIndexError>
where
    P: Checked + Serialize + for<'de> Deserialize<'de> + Clone,
{
    if idx.snapshot_at_max.is_some() || idx.event_count == 0 {
        return Ok(());
    }
    let events = load_events_from_index::<P>(log, idx)?;
    idx.snapshot_at_max = Some(snapshot_for_events(events, idx.max_version)?);
    write_sidecar(log, idx)
}

fn read_sidecar(path: &Path) -> Result<LogIndexSidecar, LogIndexError> {
    let body = fs::read_to_string(path).map_err(|e| LogIndexError::Io(e.to_string()))?;
    serde_json::from_str(&body).map_err(|e| LogIndexError::Io(e.to_string()))
}

fn write_sidecar(log: &Path, idx: &LogIndexSidecar) -> Result<(), LogIndexError> {
    let path = sidecar_path(log);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| LogIndexError::Io(e.to_string()))?;
    }
    let body = serde_json::to_string(idx).map_err(|e| LogIndexError::Io(e.to_string()))?;
    let tmp = path.with_extension("keys.tmp");
    fs::write(&tmp, body.as_bytes()).map_err(|e| LogIndexError::Io(e.to_string()))?;
    fs::rename(&tmp, &path).map_err(|e| LogIndexError::Io(e.to_string()))?;
    Ok(())
}

pub fn append_bytes_to_log(log: &Path, bytes: &[u8]) -> Result<(), LogIndexError> {
    if let Some(dir) = log.parent() {
        fs::create_dir_all(dir).map_err(|e| LogIndexError::Io(e.to_string()))?;
    }
    ensure_tail_intact(log, None)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(|e| LogIndexError::Io(e.to_string()))?;
    file.write_all(bytes)
        .map_err(|e| LogIndexError::Io(e.to_string()))?;
    file.sync_all()
        .map_err(|e| LogIndexError::Io(e.to_string()))?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commit::{Commit, CommitFault, UnitOfWork};
    use crate::record::{
        AggregateId, EventId, IdempotencyKey, Rfc3339Time, Source, Version as RecordVersion,
    };
    use crate::replay::{parse_event_log, replay};
    use crate::surface::JsonPayload;
    use crate::view::{RecordView, ViewContract};
    use crate::{encode_line, SCHEMA_VERSION, SCHEMA_VERSION_LEGACY};

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct NotePayload {
        writer: String,
        note: String,
    }

    impl crate::Checked for NotePayload {
        fn check(&self) -> Result<(), String> {
            crate::validate_writer_identity(&self.writer).map(|_| ())
        }
    }

    fn temp_log() -> (PathBuf, PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("log-index-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        (dir.clone(), dir.join("events.jsonl"))
    }

    fn fixture_event_id(n: u64) -> EventId {
        let raw = format!("01JCV1Q8Z{:016X}", n);
        let padded = format!("{raw:0<26}");
        EventId(padded[..26].parse().expect("fixture ulid"))
    }

    fn event(n: u64, key: &str) -> EventEnvelope<NotePayload> {
        EventEnvelope::with_payload(
            fixture_event_id(n),
            AggregateId(format!("agg://example.test/index/{n}")),
            IdempotencyKey(key.into()),
            RecordVersion(n),
            Rfc3339Time::new("2026-10-26T18:59:00+01:00").unwrap(),
            Rfc3339Time::new("2026-10-26T19:00:00+01:00").unwrap(),
            Source::Owner,
            SCHEMA_VERSION_LEGACY,
            NotePayload {
                writer: "laptop-1".into(),
                note: "idx".into(),
            },
        )
    }

    #[test]
    fn sidecar_rebuild_matches_incremental() {
        let (_dir, log) = temp_log();
        let unit = UnitOfWork {
            events: vec![event(1, "k1"), event(2, "k2")],
        };
        Commit::new(&log)
            .run(unit, &CommitFault::default())
            .unwrap();
        let lines = vec![format!(
            "{}\n",
            encode_line(SCHEMA_VERSION, &event(3, "k3")).unwrap()
        )];
        append_bytes_to_log(&log, lines[0].as_bytes()).unwrap();
        let ev = event(3, "k3");
        extend_index_lines(
            &log,
            &lines,
            &[ev.clone()],
            ev.hlc.unwrap_or(HlcWire { l: 0, c: 0 }),
            ev.seq.unwrap_or(3),
        )
        .unwrap();
        let incremental = load_or_rebuild::<NotePayload>(&log).unwrap();
        fs::remove_file(sidecar_path(&log)).unwrap();
        let rebuilt = rebuild_index::<NotePayload>(&log).unwrap();
        assert_eq!(incremental.header.log_len, rebuilt.header.log_len);
        assert_eq!(incremental.event_count, rebuilt.event_count);
        assert_eq!(rebuilt.event_count, 3);
    }

    #[test]
    fn torn_tail_refuses_append_and_writes_fragment() {
        let (_dir, log) = temp_log();
        fs::write(&log, b"{\"partial\":true").unwrap();
        let err = append_bytes_to_log(&log, b"{}\n").unwrap_err();
        assert!(err.to_string().contains("torn"));
        assert!(torn_fragment_path(&log).is_file());
    }

    #[test]
    fn cached_snapshot_at_max_matches_full_replay() {
        let (_dir, log) = temp_log();
        let unit = UnitOfWork {
            events: vec![event(1, "a"), event(2, "b")],
        };
        Commit::new(&log)
            .run(unit, &CommitFault::default())
            .unwrap();
        let mut idx = rebuild_index::<NotePayload>(&log).unwrap();
        ensure_snapshot_at_max::<NotePayload>(&log, &mut idx).unwrap();
        let cached = idx.snapshot_at_max.clone().unwrap();
        let text = fs::read_to_string(&log).unwrap();
        let events = parse_event_log::<NotePayload>(&text).unwrap();
        let via = RecordView::from_events(events)
            .snapshot_at(cached.at)
            .unwrap();
        assert_eq!(cached, via);
        let rows = replay(&load_events_indexed::<NotePayload>(&log).unwrap()).unwrap();
        assert_eq!(cached.facts.len(), rows.len());
    }
}
