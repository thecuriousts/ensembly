//! The published writer for pulse and decision stores on the operator machine.
//!
//! Application-ledger writes live in the optional opportunity-finder pack
//! (`--features opportunity-finder`); see [`crate::writer_ledger`] when enabled.
//!
//! For other crates, [`KernelWriter`] is the only type that can write a decision
//! event, or admit, supersede, or resolve a pulse write. Raw write functions are
//! crate-private.
//!
//! # Compile-fail checks
//!
//! ```compile_fail
//! let _ = ensembly_kernel::pulse_supersede::admit;
//! ```
//!
//! Raw application-ledger append is refused outside the opportunity-finder pack writer.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ensembly_schema::{
    require_configured_writer, DecisionEvent, DecisionEventType, LabelRow, ModelDecision,
    MODEL_VERSION, TRIAGE_QUESTION,
};
use thiserror::Error;

use crate::decision::{self, workflow, Signal};
use crate::pulse_supersede::{
    AdmitReceipt, AdmitRequest, ResolveRequest, SupersedeError, SupersedeFault, SupersedeReceipt,
    SupersedeRequest,
};
use crate::write_guard;

/// Result of a decision-log append: a new event, or a retry of the same step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionWriteReceipt {
    pub rid: String,
    pub event_type: DecisionEventType,
    pub appended: bool,
}

#[derive(Debug, Error)]
pub enum WriterError {
    #[error("{0}")]
    Refused(String),
    #[error("idempotency conflict on key {key}: {detail}")]
    Conflict { key: String, detail: String },
    #[error("ledger: {0}")]
    Ledger(String),
    #[error("pulse: {0}")]
    Pulse(#[from] SupersedeError),
    #[error("store: {0}")]
    Store(String),
}

/// The published writer for pulse stores and decision logs.
pub struct KernelWriter {
    #[cfg(feature = "opportunity-finder")]
    pub(crate) ledger: Option<PathBuf>,
    pub(crate) pulse_db: Option<PathBuf>,
    pub(crate) decisions: Option<PathBuf>,
}

impl KernelWriter {
    pub fn open_pulse(path: impl AsRef<Path>) -> Result<Self, WriterError> {
        Self {
            #[cfg(feature = "opportunity-finder")]
            ledger: None,
            pulse_db: None,
            decisions: None,
        }
        .with_pulse(path)
    }

    pub fn open_decisions(path: impl AsRef<Path>) -> Result<Self, WriterError> {
        Ok(Self {
            #[cfg(feature = "opportunity-finder")]
            ledger: None,
            pulse_db: None,
            decisions: Some(path.as_ref().to_path_buf()),
        })
    }

    pub(crate) fn with_pulse(mut self, path: impl AsRef<Path>) -> Result<Self, WriterError> {
        let path = path.as_ref().to_path_buf();
        if !path.is_file() {
            return Err(WriterError::Store("pulse db missing".into()));
        }
        let conn = rusqlite::Connection::open(&path).map_err(store)?;
        write_guard::grant_session(&conn).map_err(store)?;
        write_guard::install_triggers(&conn).map_err(store)?;
        self.pulse_db = Some(path);
        Ok(self)
    }

    pub fn admit(
        &self,
        req: &AdmitRequest,
        fault: &SupersedeFault,
    ) -> Result<AdmitReceipt, WriterError> {
        let path = self.pulse_path()?;
        require_writer(&req.writer)?;
        let receipt = crate::pulse_supersede::admit(path, req, fault)?;
        self.refresh_pulse_guard()?;
        Ok(receipt)
    }

    pub fn supersede(
        &self,
        req: &SupersedeRequest,
        fault: &SupersedeFault,
    ) -> Result<SupersedeReceipt, WriterError> {
        let path = self.pulse_path()?;
        require_writer(&req.writer)?;
        let receipt = crate::pulse_supersede::supersede(path, req, fault)?;
        self.refresh_pulse_guard()?;
        Ok(receipt)
    }

    pub fn resolve_conflict(
        &self,
        req: &ResolveRequest,
        fault: &SupersedeFault,
    ) -> Result<SupersedeReceipt, WriterError> {
        let path = self.pulse_path()?;
        require_writer(&req.writer)?;
        let receipt = crate::pulse_supersede::resolve_conflict(path, req, fault)?;
        self.refresh_pulse_guard()?;
        Ok(receipt)
    }

    /// Seal an empty or missing decision event log.
    pub fn init_decisions(&self) -> Result<(), WriterError> {
        let path = self.decisions_path()?;
        let bytes = read_sealed_bytes(path)?;
        if !bytes.is_empty() {
            return Err(WriterError::Refused(
                "decision log already has rows; init is only for a new empty log".into(),
            ));
        }
        if seal_path(path).exists() {
            return Err(WriterError::Refused(
                "decision log is already sealed".into(),
            ));
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .map_err(|err| WriterError::Refused(format!("create {}: {err}", dir.display())))?;
        }
        if !path.exists() {
            fs::write(path, b"")
                .map_err(|err| WriterError::Refused(format!("create {}: {err}", path.display())))?;
        }
        write_seal(path, b"")?;
        lock_sealed(path)
    }

    /// Record a proposal (or a gate skip) and wait. Idempotent on (rid, step).
    pub fn propose_decision(
        &self,
        row: &LabelRow,
        score: &decision::Score,
        writer: &str,
        event_time: &str,
        deadline: Option<&str>,
    ) -> Result<DecisionWriteReceipt, WriterError> {
        require_writer(writer)?;
        let snapshot = row.snapshot_hash();
        let rid = ensembly_schema::proposal_rid(&row.link_key, MODEL_VERSION, &snapshot);
        let (event_type, decision) = if score.gate.verdict == ensembly_schema::GateVerdict::Fail {
            (DecisionEventType::Skipped, Some(ModelDecision::Skip))
        } else {
            (DecisionEventType::Proposed, Some(score.decision))
        };
        let ev = DecisionEvent {
            event_type,
            rid: rid.clone(),
            link_key: ensembly_schema::link_key(&row.link_key).to_string(),
            event_time: event_time.into(),
            writer: writer.into(),
            model_version: Some(MODEL_VERSION.into()),
            snapshot_hash: snapshot,
            decision,
            p_apply: Some(score.p_apply),
            reason: Some(score.reason.clone()),
            deadline: deadline.map(str::to_string),
            provenance: "model-proposal".into(),
            triage_question: TRIAGE_QUESTION.into(),
        };
        self.append_decision_event(&ev)
    }

    pub fn confirm_decision(
        &self,
        rid: &str,
        writer: &str,
        event_time: &str,
    ) -> Result<DecisionWriteReceipt, WriterError> {
        self.signal_decision(rid, writer, event_time, Signal::Confirm, None)
    }

    pub fn override_decision(
        &self,
        rid: &str,
        writer: &str,
        event_time: &str,
        label: ModelDecision,
    ) -> Result<DecisionWriteReceipt, WriterError> {
        self.signal_decision(
            rid,
            writer,
            event_time,
            Signal::Override { label },
            Some(label),
        )
    }

    pub fn expire_decision(
        &self,
        rid: &str,
        writer: &str,
        event_time: &str,
    ) -> Result<DecisionWriteReceipt, WriterError> {
        self.signal_decision(rid, writer, event_time, Signal::Expire, None)
    }

    fn signal_decision(
        &self,
        rid: &str,
        writer: &str,
        event_time: &str,
        signal: Signal,
        override_label: Option<ModelDecision>,
    ) -> Result<DecisionWriteReceipt, WriterError> {
        require_writer(writer)?;
        let path = self.decisions_path()?;
        let before = read_sealed_bytes(path)?;
        check_seal(path, &before)?;
        let text = std::str::from_utf8(&before).map_err(|e| WriterError::Refused(e.to_string()))?;
        let log =
            decision::store::parse_log(text).map_err(|e| WriterError::Refused(e.to_string()))?;
        let owned: Vec<DecisionEvent> = decision::store::events_for(&log, rid)
            .into_iter()
            .cloned()
            .collect();
        let first = owned
            .first()
            .ok_or_else(|| WriterError::Refused("unknown rid".into()))?;
        let event_type = match &signal {
            Signal::Confirm => DecisionEventType::Confirmed,
            Signal::Override { .. } => DecisionEventType::Overridden,
            Signal::Expire => DecisionEventType::Expired,
        };
        if let Some(old) = owned.iter().find(|e| e.event_type == event_type) {
            let expected = if event_type == DecisionEventType::Expired {
                None
            } else {
                override_label.or(first.decision)
            };
            if old.writer == writer && old.decision == expected {
                return Ok(DecisionWriteReceipt {
                    rid: rid.into(),
                    event_type,
                    appended: false,
                });
            }
            return Err(WriterError::Refused(format!(
                "idempotency conflict on rid {rid} step {}: a different payload is already stored",
                event_type.as_str()
            )));
        }
        let current = workflow::phase(&owned).map_err(|e| WriterError::Refused(e.to_string()))?;
        workflow::apply(current, signal.clone())
            .map_err(|e| WriterError::Refused(e.to_string()))?;
        let event_type = match signal {
            Signal::Confirm => DecisionEventType::Confirmed,
            Signal::Override { .. } => DecisionEventType::Overridden,
            Signal::Expire => DecisionEventType::Expired,
        };
        let ev = DecisionEvent {
            event_type,
            rid: rid.into(),
            link_key: first.link_key.clone(),
            event_time: event_time.into(),
            writer: writer.into(),
            model_version: first.model_version.clone(),
            snapshot_hash: first.snapshot_hash.clone(),
            decision: override_label.or(first.decision),
            p_apply: first.p_apply,
            reason: match signal {
                Signal::Expire => Some("expired: silence took no action".into()),
                Signal::Override { .. } => Some("operator override".into()),
                Signal::Confirm => Some("operator confirm".into()),
            },
            deadline: first.deadline.clone(),
            provenance: "operator-signal".into(),
            triage_question: TRIAGE_QUESTION.into(),
        };
        let ev = if event_type == DecisionEventType::Expired {
            DecisionEvent {
                decision: None,
                p_apply: first.p_apply,
                ..ev
            }
        } else {
            ev
        };
        self.append_decision_event(&ev)
    }

    fn append_decision_event(
        &self,
        ev: &DecisionEvent,
    ) -> Result<DecisionWriteReceipt, WriterError> {
        let path = self.decisions_path()?;
        let before = read_sealed_bytes(path)?;
        check_seal(path, &before)?;
        let text = std::str::from_utf8(&before).map_err(|e| WriterError::Refused(e.to_string()))?;
        let (after, appended) = decision::store::append_event_text(text, ev)
            .map_err(|e| WriterError::Refused(e.to_string()))?;
        if !appended {
            return Ok(DecisionWriteReceipt {
                rid: ev.rid.clone(),
                event_type: ev.event_type,
                appended: false,
            });
        }
        unlock_sealed(path)?;
        let write = fs::write(path, after.as_bytes())
            .map_err(|err| WriterError::Refused(format!("write {}: {err}", path.display())));
        let after_bytes = after.into_bytes();
        let sealed = write.and_then(|_| {
            if !sealed_grew_without_rewrite(&before, &after_bytes) {
                return Err(WriterError::Refused(
                    "decision log history was rewritten; the writer will not keep a mutated prefix"
                        .into(),
                ));
            }
            write_seal(path, &after_bytes)
        });
        let lock = lock_sealed(path);
        sealed?;
        lock?;
        Ok(DecisionWriteReceipt {
            rid: ev.rid.clone(),
            event_type: ev.event_type,
            appended: true,
        })
    }

    fn decisions_path(&self) -> Result<&Path, WriterError> {
        self.decisions
            .as_deref()
            .ok_or_else(|| WriterError::Refused("decision log is not open".into()))
    }

    fn refresh_pulse_guard(&self) -> Result<(), WriterError> {
        let path = self.pulse_path()?;
        let conn = rusqlite::Connection::open(path).map_err(store)?;
        write_guard::grant_session(&conn).map_err(store)?;
        write_guard::install_triggers(&conn).map_err(store)?;
        Ok(())
    }

    fn pulse_path(&self) -> Result<&Path, WriterError> {
        self.pulse_db
            .as_deref()
            .ok_or_else(|| WriterError::Refused("pulse db is not open".into()))
    }
}

fn require_writer(writer: &str) -> Result<(), WriterError> {
    require_configured_writer(writer).map_err(WriterError::Refused)?;
    Ok(())
}

pub(crate) fn read_sealed_bytes(path: &Path) -> Result<Vec<u8>, WriterError> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(WriterError::Ledger(format!(
            "read {}: {err}",
            path.display()
        ))),
    }
}

pub(crate) fn sealed_grew_without_rewrite(before: &[u8], after: &[u8]) -> bool {
    if before.is_empty() {
        return !after.is_empty();
    }
    if after.starts_with(before) {
        return after.len() > before.len();
    }
    let mut with_nl = before.to_vec();
    if !with_nl.ends_with(b"\n") {
        with_nl.push(b'\n');
    }
    after.starts_with(&with_nl) && after.len() > with_nl.len()
}

pub(crate) fn unlock_sealed(path: &Path) -> Result<(), WriterError> {
    if path.exists() {
        set_sealed_mode(path, 0o644)?;
    }
    let seal = seal_path(path);
    if seal.exists() {
        set_sealed_mode(&seal, 0o644)?;
    }
    Ok(())
}

pub(crate) fn lock_sealed(path: &Path) -> Result<(), WriterError> {
    if path.exists() {
        set_sealed_mode(path, 0o444)?;
    }
    let seal = seal_path(path);
    if seal.exists() {
        set_sealed_mode(&seal, 0o444)?;
    }
    Ok(())
}

fn set_sealed_mode(path: &Path, mode: u32) -> Result<(), WriterError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|err| WriterError::Ledger(format!("permissions {}: {err}", path.display())))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

pub(crate) fn seal_path(ledger: &Path) -> PathBuf {
    let mut name = ledger
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("ledger.jsonl"))
        .to_os_string();
    name.push(".writer-hash");
    ledger.with_file_name(name)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

pub(crate) fn check_seal(path: &Path, bytes: &[u8]) -> Result<(), WriterError> {
    let seal = seal_path(path);
    match fs::read_to_string(&seal) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(WriterError::Ledger(
            "seal is missing; the writer will not adopt current bytes".into(),
        )),
        Ok(expected) => {
            if expected.trim() == sha256_hex(bytes) {
                Ok(())
            } else {
                Err(WriterError::Ledger(
                    "history was rewritten; the writer will not append".into(),
                ))
            }
        }
        Err(err) => Err(WriterError::Ledger(format!(
            "read {}: {err}",
            seal.display()
        ))),
    }
}

pub(crate) fn write_seal(path: &Path, bytes: &[u8]) -> Result<(), WriterError> {
    let seal = seal_path(path);
    if seal.exists() {
        set_sealed_mode(&seal, 0o644)?;
    }
    fs::write(
        &seal,
        format!(
            "{}
",
            sha256_hex(bytes)
        ),
    )
    .map_err(|err| WriterError::Ledger(format!("write {}: {err}", seal.display())))?;
    set_sealed_mode(&seal, 0o444)
}

pub(crate) fn store(err: impl ToString) -> WriterError {
    WriterError::Store(err.to_string())
}
