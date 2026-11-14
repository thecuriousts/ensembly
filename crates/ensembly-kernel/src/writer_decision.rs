//! Decision-log writer (opportunity-finder pack).

use std::fs;
use std::path::Path;

use ensembly_schema::{
    DecisionEvent, DecisionEventType, LabelRow, ModelDecision, MODEL_VERSION, TRIAGE_QUESTION,
};

use super::writer::{
    check_seal, lock_sealed, read_sealed_bytes, require_writer, sealed_grew_without_rewrite,
    unlock_sealed, write_seal, KernelWriter, WriterError,
};
use crate::decision::{self, workflow, Signal};

/// Result of a decision-log append: a new event, or a retry of the same step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionWriteReceipt {
    pub rid: String,
    pub event_type: DecisionEventType,
    pub appended: bool,
}

impl KernelWriter {
    pub fn open_decisions(path: impl AsRef<Path>) -> Result<Self, WriterError> {
        Ok(Self {
            ledger: None,
            pulse_db: None,
            decisions: Some(path.as_ref().to_path_buf()),
        })
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
        if super::writer::seal_path(path).exists() {
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

    pub(crate) fn decisions_path(&self) -> Result<&Path, WriterError> {
        self.decisions
            .as_deref()
            .ok_or_else(|| WriterError::Refused("decision log is not open".into()))
    }
}
