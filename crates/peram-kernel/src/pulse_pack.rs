//! Portable pulse + memory sync packs — export/replica path for laptop ↔ bot.
//!
//! Law: **one writer** on `peram-ops.sqlite` (canonical kernel host). Packs carry
//! episodic memory (CRDT) and optional read-only archive slices — never dual-live
//! ops mutation. Import merges into `peram-memory.json` via CRDT `merge`; archive
//! rows land in a sidecar JSONL (`pulse-archive.jsonl`), not the ops ledger.

use chrono::{DateTime, Utc};
use peram_memory::{CrdtDocument, TrajectoryEntry, TrajectoryType};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::store::OpsStore;

pub const PULSE_PACK_FORMAT: &str = "peram-pulse-pack-v1";
pub const DEFAULT_ARCHIVE_SIDECAR: &str = "data/local/pulse-archive.jsonl";

#[derive(Debug, Error)]
pub enum PulsePackError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("memory: {0}")]
    Memory(#[from] peram_memory::MemoryError),
    #[error("store: {0}")]
    Store(#[from] crate::store::StoreError),
    #[error("pack: {0}")]
    Pack(String),
}

/// Portable memory trace — maps 1:1 to CRDT trajectory entry id as `nat_key`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryTrace {
    /// Stable natural key (trajectory entry id).
    pub nat_key: String,
    pub ts: DateTime<Utc>,
    pub kind: String,
    pub content: serde_json::Value,
    pub coherence: f32,
    pub agent_id: String,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// When this trace was admitted to the exporting host (LWW tie-breaker).
    pub admitted_at: DateTime<Utc>,
}

/// Read-only archive slice — audit / pulse event, keyed for idempotent import.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArchiveEvent {
    pub nat_key: String,
    pub ts: DateTime<Utc>,
    pub kind: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulsePack {
    pub format: String,
    pub exported_at: DateTime<Utc>,
    pub source_host: String,
    /// SHA-256 hex of canonical payload (traces + archive + memory hash).
    pub pack_hash: String,
    /// Content hash of `memory_crdt` at export (survives JSON roundtrip).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_hash: Option<String>,
    pub memory_traces: Vec<MemoryTrace>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub archive_events: Vec<ArchiveEvent>,
    /// Full CRDT document for merge-import (optional when traces-only pack).
    pub memory_crdt: Option<CrdtDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulseExportOpts {
    pub memory_path: PathBuf,
    pub ops_db: Option<PathBuf>,
    pub archive_limit: usize,
    pub source_host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulseImportOpts {
    pub memory_path: PathBuf,
    pub archive_sidecar: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulsePackStatus {
    pub format: String,
    pub pack_hash: String,
    pub exported_at: DateTime<Utc>,
    pub source_host: String,
    pub memory_trace_count: usize,
    pub archive_event_count: usize,
    pub last_seen: Option<DateTime<Utc>>,
    pub memory_trajectory_count: Option<usize>,
    pub memory_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulseImportReport {
    pub ok: bool,
    pub dry_run: bool,
    pub pack_hash: String,
    pub traces_merged: usize,
    pub traces_added: usize,
    pub archive_appended: usize,
    pub archive_skipped: usize,
    pub memory_trajectory_before: usize,
    pub memory_trajectory_after: usize,
    pub message: String,
}

impl PulsePack {
    pub fn from_memory(
        doc: &CrdtDocument,
        archive: Vec<ArchiveEvent>,
        source_host: impl Into<String>,
    ) -> Self {
        let memory_traces = traces_from_document(doc);
        let mem_hash = doc.hash.clone();
        let pack_hash = compute_pack_hash(&memory_traces, &archive, Some(&mem_hash));
        Self {
            format: PULSE_PACK_FORMAT.into(),
            exported_at: Utc::now(),
            source_host: source_host.into(),
            pack_hash,
            memory_hash: Some(mem_hash),
            memory_traces,
            archive_events: archive,
            memory_crdt: Some(doc.clone()),
        }
    }

    pub fn validate_hash(&self) -> Result<(), PulsePackError> {
        let mem_hash = self
            .memory_hash
            .as_deref()
            .or_else(|| self.memory_crdt.as_ref().map(|d| d.hash.as_str()))
            .filter(|h| !h.is_empty());
        let expected = compute_pack_hash(&self.memory_traces, &self.archive_events, mem_hash);
        if expected != self.pack_hash {
            return Err(PulsePackError::Pack(format!(
                "pack_hash mismatch: expected {expected}, got {}",
                self.pack_hash
            )));
        }
        if self.format != PULSE_PACK_FORMAT {
            return Err(PulsePackError::Pack(format!(
                "unexpected format {}",
                self.format
            )));
        }
        Ok(())
    }

    pub fn status(&self) -> PulsePackStatus {
        let last_seen = self
            .memory_traces
            .iter()
            .map(|t| t.ts)
            .chain(self.archive_events.iter().map(|e| e.ts))
            .max();
        PulsePackStatus {
            format: self.format.clone(),
            pack_hash: self.pack_hash.clone(),
            exported_at: self.exported_at,
            source_host: self.source_host.clone(),
            memory_trace_count: self.memory_traces.len(),
            archive_event_count: self.archive_events.len(),
            last_seen,
            memory_trajectory_count: self.memory_crdt.as_ref().map(|d| d.trajectory.len()),
            memory_hash: self.memory_hash.clone().or_else(|| {
                self.memory_crdt.as_ref().map(|d| d.hash.clone()).filter(|h| !h.is_empty())
            }),
        }
    }
}

pub fn traces_from_document(doc: &CrdtDocument) -> Vec<MemoryTrace> {
    doc.trajectory
        .values()
        .map(|e| trace_from_entry(e))
        .collect()
}

pub fn trace_from_entry(entry: &TrajectoryEntry) -> MemoryTrace {
    MemoryTrace {
        nat_key: entry.id.clone(),
        ts: entry.timestamp,
        kind: trajectory_kind_str(&entry.entry_type),
        content: entry.content.clone(),
        coherence: entry.coherence,
        agent_id: entry.agent_id.clone(),
        metadata: entry.metadata.clone(),
        admitted_at: entry.timestamp,
    }
}

fn trajectory_kind_str(t: &TrajectoryType) -> String {
    match t {
        TrajectoryType::Observation => "observation",
        TrajectoryType::Action => "action",
        TrajectoryType::Reflection => "reflection",
        TrajectoryType::SkillSynthesis => "skill_synthesis",
        TrajectoryType::GoalUpdate => "goal_update",
        TrajectoryType::ContextUpdate => "context_update",
    }
    .into()
}

fn parse_trajectory_kind(s: &str) -> TrajectoryType {
    match s {
        "action" => TrajectoryType::Action,
        "reflection" => TrajectoryType::Reflection,
        "skill_synthesis" => TrajectoryType::SkillSynthesis,
        "goal_update" => TrajectoryType::GoalUpdate,
        "context_update" => TrajectoryType::ContextUpdate,
        _ => TrajectoryType::Observation,
    }
}

pub fn entry_from_trace(trace: &MemoryTrace) -> TrajectoryEntry {
    let ts = if trace.admitted_at > trace.ts {
        trace.admitted_at
    } else {
        trace.ts
    };
    TrajectoryEntry {
        id: trace.nat_key.clone(),
        timestamp: ts,
        entry_type: parse_trajectory_kind(&trace.kind),
        content: trace.content.clone(),
        coherence: trace.coherence,
        agent_id: trace.agent_id.clone(),
        metadata: trace.metadata.clone(),
    }
}

/// Merge traces into a document using nat_key LWW (newer `admitted_at` / `ts` wins).
pub fn merge_traces_into(doc: &mut CrdtDocument, traces: &[MemoryTrace]) -> (usize, usize) {
    let before = doc.trajectory.len();
    let mut patch = CrdtDocument::new(&doc.agent_id);
    for trace in traces {
        let entry = entry_from_trace(trace);
        match patch.trajectory.get(&trace.nat_key) {
            Some(existing) if entry.timestamp <= existing.timestamp => {}
            _ => {
                patch.trajectory.insert(trace.nat_key.clone(), entry);
            }
        }
    }
    doc.merge(&patch);
    let after = doc.trajectory.len();
    (before, after - before)
}

pub fn export_pulse_pack(opts: &PulseExportOpts) -> Result<PulsePack, PulsePackError> {
    let persistence = peram_memory::FilePersistence::new(&opts.memory_path);
    let doc = match persistence.load()? {
        Some(d) => d,
        None => CrdtDocument::new("peram-swarm"),
    };

    let archive = if let Some(db_path) = &opts.ops_db {
        export_archive_from_ops(db_path, opts.archive_limit)?
    } else {
        vec![]
    };

    Ok(PulsePack::from_memory(&doc, archive, &opts.source_host))
}

fn export_archive_from_ops(
    db_path: &Path,
    limit: usize,
) -> Result<Vec<ArchiveEvent>, PulsePackError> {
    if limit == 0 {
        return Ok(vec![]);
    }
    let store = OpsStore::open(db_path)?;
    let tail = store.audit_tail(limit)?;
    let mut out = vec![];
    for (ts, kind, payload) in tail {
        let nat_key = format!("audit:{}:{}", ts, kind);
        let payload_val = serde_json::from_str(&payload).unwrap_or_else(|_| {
            serde_json::json!({ "raw": payload })
        });
        let ts_parsed = DateTime::parse_from_rfc3339(&ts)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        out.push(ArchiveEvent {
            nat_key,
            ts: ts_parsed,
            kind,
            payload: payload_val,
        });
    }
    Ok(out)
}

pub fn write_pulse_pack(path: impl AsRef<Path>, pack: &PulsePack) -> Result<(), PulsePackError> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(pack)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn read_pulse_pack(path: impl AsRef<Path>) -> Result<PulsePack, PulsePackError> {
    let raw = fs::read_to_string(path)?;
    let pack: PulsePack = serde_json::from_str(&raw)?;
    pack.validate_hash()?;
    Ok(pack)
}

pub fn import_pulse_pack(
    pack: &PulsePack,
    opts: &PulseImportOpts,
) -> Result<PulseImportReport, PulsePackError> {
    pack.validate_hash()?;

    let persistence = peram_memory::FilePersistence::new(&opts.memory_path);
    let mut doc = match persistence.load()? {
        Some(d) => d,
        None => CrdtDocument::new("peram-swarm"),
    };
    let before = doc.trajectory.len();

    if let Some(peer) = &pack.memory_crdt {
        doc.merge(peer);
    }
    let (merged_from, added_from_traces) = merge_traces_into(&mut doc, &pack.memory_traces);

    let (archive_appended, archive_skipped) = if opts.dry_run {
        count_archive_merge(&opts.archive_sidecar, &pack.archive_events)?
    } else {
        merge_archive_sidecar(&opts.archive_sidecar, &pack.archive_events)?
    };

    let after = doc.trajectory.len();

    if !opts.dry_run {
        persistence.save(&doc)?;
    }

    Ok(PulseImportReport {
        ok: true,
        dry_run: opts.dry_run,
        pack_hash: pack.pack_hash.clone(),
        traces_merged: merged_from,
        traces_added: added_from_traces,
        archive_appended,
        archive_skipped,
        memory_trajectory_before: before,
        memory_trajectory_after: after,
        message: if opts.dry_run {
            "import dry-run OK — local stores not modified".into()
        } else {
            "import OK — memory CRDT merged, archive sidecar updated".into()
        },
    })
}

pub fn local_pulse_status(
    memory_path: &Path,
    archive_sidecar: &Path,
) -> Result<PulsePackStatus, PulsePackError> {
    let persistence = peram_memory::FilePersistence::new(memory_path);
    let doc = persistence.load()?;
    let trace_count = doc.as_ref().map(|d| d.trajectory.len()).unwrap_or(0);
    let mem_hash = doc.as_ref().map(|d| d.hash.clone());
    let last_mem = doc
        .as_ref()
        .and_then(|d| d.trajectory.values().map(|e| e.timestamp).max());
    let archive_count = count_archive_lines(archive_sidecar)?;
    let last_archive = last_archive_ts(archive_sidecar)?;
    let last_seen = match (last_mem, last_archive) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };
    Ok(PulsePackStatus {
        format: "local".into(),
        pack_hash: mem_hash.clone().unwrap_or_else(|| "-".into()),
        exported_at: Utc::now(),
        source_host: hostname(),
        memory_trace_count: trace_count,
        archive_event_count: archive_count,
        last_seen,
        memory_trajectory_count: doc.as_ref().map(|d| d.trajectory.len()),
        memory_hash: mem_hash,
    })
}

fn compute_pack_hash(
    traces: &[MemoryTrace],
    archive: &[ArchiveEvent],
    memory_hash: Option<&str>,
) -> String {
    let canonical = serde_json::json!({
        "traces": traces,
        "archive": archive,
        "memory_hash": memory_hash,
    });
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    let digest = Sha256::digest(&bytes);
    hex::encode(digest)
}

fn merge_archive_sidecar(
    path: &Path,
    events: &[ArchiveEvent],
) -> Result<(usize, usize), PulsePackError> {
    if events.is_empty() {
        return Ok((0, 0));
    }
    let existing = load_archive_index(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let mut appended = 0usize;
    let mut skipped = 0usize;
    for event in events {
        if existing.contains_key(&event.nat_key) {
            let prev_ts = existing.get(&event.nat_key).copied().unwrap_or(event.ts);
            if event.ts <= prev_ts {
                skipped += 1;
                continue;
            }
        }
        let line = serde_json::to_string(event)?;
        writeln!(file, "{line}")?;
        appended += 1;
    }
    Ok((appended, skipped))
}

fn count_archive_merge(
    path: &Path,
    events: &[ArchiveEvent],
) -> Result<(usize, usize), PulsePackError> {
    let existing = load_archive_index(path)?;
    let mut appended = 0usize;
    let mut skipped = 0usize;
    for event in events {
        if existing.contains_key(&event.nat_key) {
            let prev_ts = existing.get(&event.nat_key).copied().unwrap_or(event.ts);
            if event.ts <= prev_ts {
                skipped += 1;
            } else {
                appended += 1;
            }
        } else {
            appended += 1;
        }
    }
    Ok((appended, skipped))
}

fn load_archive_index(path: &Path) -> Result<HashMap<String, DateTime<Utc>>, PulsePackError> {
    let mut index = HashMap::new();
    if !path.exists() {
        return Ok(index);
    }
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(event) = serde_json::from_str::<ArchiveEvent>(&line) {
            index
                .entry(event.nat_key.clone())
                .and_modify(|ts| {
                    if event.ts > *ts {
                        *ts = event.ts;
                    }
                })
                .or_insert(event.ts);
        }
    }
    Ok(index)
}

fn count_archive_lines(path: &Path) -> Result<usize, PulsePackError> {
    if !path.exists() {
        return Ok(0);
    }
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    Ok(reader.lines().filter(|l| l.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)).count())
}

fn last_archive_ts(path: &Path) -> Result<Option<DateTime<Utc>>, PulsePackError> {
    if !path.exists() {
        return Ok(None);
    }
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut max_ts: Option<DateTime<Utc>> = None;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(event) = serde_json::from_str::<ArchiveEvent>(&line) {
            max_ts = Some(max_ts.map(|m| m.max(event.ts)).unwrap_or(event.ts));
        }
    }
    Ok(max_ts)
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use peram_memory::EpisodicMemory;

    fn sample_trace(key: &str, ts: DateTime<Utc>, from: &str) -> MemoryTrace {
        MemoryTrace {
            nat_key: key.into(),
            ts,
            kind: "observation".into(),
            content: serde_json::json!({ "from": from }),
            coherence: 0.5,
            agent_id: "test".into(),
            metadata: HashMap::new(),
            admitted_at: ts,
        }
    }

    #[test]
    fn export_import_roundtrip_preserves_trajectory() {
        let dir = tempfile::tempdir().unwrap();
        let mem_path = dir.path().join("mem.json");
        let pack_path = dir.path().join("pulse.pack.json");
        let archive_path = dir.path().join("archive.jsonl");

        let mut mem = EpisodicMemory::open(&mem_path, "bot").unwrap();
        mem.append(
            peram_memory::TrajectoryType::Observation,
            serde_json::json!({"note": "from-bot"}),
            0.7,
        );
        mem.save().unwrap();

        let pack = export_pulse_pack(&PulseExportOpts {
            memory_path: mem_path.clone(),
            ops_db: None,
            archive_limit: 0,
            source_host: "grok-bot".into(),
        })
        .unwrap();
        write_pulse_pack(&pack_path, &pack).unwrap();

        let read = read_pulse_pack(&pack_path).unwrap();
        assert_eq!(read.memory_traces.len(), 1);

        let dest_mem = dir.path().join("laptop-mem.json");
        let report = import_pulse_pack(
            &read,
            &PulseImportOpts {
                memory_path: dest_mem.clone(),
                archive_sidecar: archive_path,
                dry_run: false,
            },
        )
        .unwrap();
        assert!(report.ok);
        assert_eq!(report.memory_trajectory_after, 1);

        let laptop = peram_memory::FilePersistence::new(&dest_mem);
        let doc = laptop.load().unwrap().unwrap();
        assert_eq!(doc.trajectory.len(), 1);
    }

    #[test]
    fn concurrent_merge_does_not_lose_entries() {
        let dir = tempfile::tempdir().unwrap();
        let bot_mem = dir.path().join("bot.json");
        let laptop_mem = dir.path().join("laptop.json");
        let archive = dir.path().join("archive.jsonl");

        let mut bot = EpisodicMemory::open(&bot_mem, "bot").unwrap();
        bot.append(
            peram_memory::TrajectoryType::Action,
            serde_json::json!({"from": "bot"}),
            0.6,
        );
        bot.save().unwrap();

        let mut laptop = EpisodicMemory::open(&laptop_mem, "laptop").unwrap();
        laptop.append(
            peram_memory::TrajectoryType::Observation,
            serde_json::json!({"from": "laptop"}),
            0.5,
        );
        laptop.save().unwrap();

        let bot_pack = export_pulse_pack(&PulseExportOpts {
            memory_path: bot_mem.clone(),
            ops_db: None,
            archive_limit: 0,
            source_host: "bot".into(),
        })
        .unwrap();
        let laptop_pack = export_pulse_pack(&PulseExportOpts {
            memory_path: laptop_mem.clone(),
            ops_db: None,
            archive_limit: 0,
            source_host: "laptop".into(),
        })
        .unwrap();

        // Laptop imports bot pack first, then bot imports laptop pack — both converge.
        import_pulse_pack(
            &bot_pack,
            &PulseImportOpts {
                memory_path: laptop_mem.clone(),
                archive_sidecar: archive.clone(),
                dry_run: false,
            },
        )
        .unwrap();
        import_pulse_pack(
            &laptop_pack,
            &PulseImportOpts {
                memory_path: bot_mem.clone(),
                archive_sidecar: archive.clone(),
                dry_run: false,
            },
        )
        .unwrap();

        let bot_doc = peram_memory::FilePersistence::new(&bot_mem)
            .load()
            .unwrap()
            .unwrap();
        let laptop_doc = peram_memory::FilePersistence::new(&laptop_mem)
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(bot_doc.trajectory.len(), 2);
        assert_eq!(laptop_doc.trajectory.len(), 2);
    }

    #[test]
    fn nat_key_lww_keeps_newer_trace() {
        let dir = tempfile::tempdir().unwrap();
        let mem_path = dir.path().join("mem.json");
        let archive = dir.path().join("archive.jsonl");

        let older = Utc.with_ymd_and_hms(2026, 9, 4, 9, 0, 0).unwrap();
        let newer = Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap();

        let mut doc = CrdtDocument::new("host");
        let mut patch = CrdtDocument::new("host");
        patch
            .trajectory
            .insert("key-1".into(), entry_from_trace(&sample_trace("key-1", older, "old")));
        doc.merge(&patch);

        let pack = PulsePack::from_memory(&doc, vec![], "peer");
        let mut peer_doc = CrdtDocument::new("peer");
        let mut peer_patch = CrdtDocument::new("peer");
        peer_patch
            .trajectory
            .insert("key-1".into(), entry_from_trace(&sample_trace("key-1", newer, "new")));
        peer_doc.merge(&peer_patch);
        let mut pack_with_update = pack;
        pack_with_update.memory_traces = vec![sample_trace("key-1", newer, "new")];
        pack_with_update.memory_crdt = Some(peer_doc);
        pack_with_update.memory_hash = pack_with_update.memory_crdt.as_ref().map(|d| d.hash.clone());
        pack_with_update.pack_hash = compute_pack_hash(
            &pack_with_update.memory_traces,
            &pack_with_update.archive_events,
            pack_with_update.memory_hash.as_deref(),
        );

        import_pulse_pack(
            &pack_with_update,
            &PulseImportOpts {
                memory_path: mem_path.clone(),
                archive_sidecar: archive,
                dry_run: false,
            },
        )
        .unwrap();

        let loaded = peram_memory::FilePersistence::new(&mem_path)
            .load()
            .unwrap()
            .unwrap();
        let entry = loaded.trajectory.get("key-1").unwrap();
        assert_eq!(entry.content["from"], "new");
    }

    #[test]
    fn pack_hash_detects_tamper() {
        let doc = CrdtDocument::new("x");
        let pack = PulsePack::from_memory(&doc, vec![], "host");
        let mut tampered = pack.clone();
        tampered.pack_hash = "00".repeat(32);
        assert!(tampered.validate_hash().is_err());
    }
}
