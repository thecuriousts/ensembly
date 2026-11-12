//! Map application-ledger rows onto record events.
//!
//! The ledger stays the source of truth. This module only builds events.
//! `Confirmed` has no ledger source. `Rejected` and `NextStage` carry evidence
//! source `legacy-unknown` because the record type requires evidence and the
//! ledger has none. A silent row uses its submit time, or `observed_at` when
//! that time was never recorded.

use crate::record::{
    AggregateId, EventEnvelope, EventId, Evidence, IdempotencyKey, Source, Status as RecordStatus,
    StockholmTime, SubmitTime as RecordSubmitTime, TextForm, Version,
};
use crate::replay::replay;
use crate::{link_key, LedgerRow, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedStage {
    pub token: String,
    pub evidence_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppEventPayload {
    pub writer: String,
    pub company: String,
    pub role: String,
    pub status: RecordStatus,
}

impl crate::Checked for AppEventPayload {
    fn check(&self) -> Result<(), String> {
        check_writer(&self.writer)?;
        if self.company.trim().is_empty() || self.role.trim().is_empty() {
            return Err("company and role must not be blank".into());
        }
        if payload_has_private_path(self) {
            return Err("payload contains a private path".into());
        }
        self.status.check()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompareMismatch {
    pub link_key: String,
    pub ledger: String,
    pub record: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompareReport {
    pub matches: Vec<String>,
    pub mismatches: Vec<CompareMismatch>,
}

pub fn map_ledger_status_name(name: &str) -> Result<MappedStage, String> {
    match status_from_name(name) {
        Some(status) => Ok(map_ledger_status(status)),
        None => Err(unknown_status(name)),
    }
}

pub fn map_ledger_status(status: crate::ledger::Status) -> MappedStage {
    map_open_status(status).unwrap_or_else(|| map_closed_status(status))
}

pub fn import_plan(
    existing: &[EventEnvelope<AppEventPayload>],
    rows: &[LedgerRow],
    writer: &str,
    observed_at: &str,
) -> Result<Vec<EventEnvelope<AppEventPayload>>, String> {
    check_writer(writer)?;
    let at = StockholmTime::new(observed_at)?;
    let mut planned = Vec::new();
    let mut staged = existing.to_vec();
    for row in current_rows(rows) {
        let event = build_event(row, writer, &at, &staged)?;
        match seen(&staged, &event) {
            Seen::Same => {}
            Seen::Conflict => return Err(conflict(&event.key.0)),
            Seen::New => {
                staged.push(event.clone());
                planned.push(event);
            }
        }
    }
    Ok(planned)
}

pub fn compare_ledger(
    rows: &[LedgerRow],
    events: &[EventEnvelope<AppEventPayload>],
) -> Result<CompareReport, String> {
    let ledger = ledger_tokens(rows);
    let record = record_tokens(events)?;
    let mut keys = BTreeSet::new();
    keys.extend(ledger.keys().cloned());
    keys.extend(record.keys().cloned());
    let mut matches = Vec::new();
    let mut mismatches = Vec::new();
    for key in keys {
        classify(&ledger, &record, &key, &mut matches, &mut mismatches);
    }
    Ok(CompareReport {
        matches,
        mismatches,
    })
}

fn stage(token: &str, evidence: Option<&str>) -> MappedStage {
    MappedStage {
        token: token.to_string(),
        evidence_source: evidence.map(str::to_string),
    }
}

fn unknown_status(name: &str) -> String {
    format!(
        "unknown ledger status {name:?}; known: prepared, submitted, interview, rejected, withdrawn, silent-closed"
    )
}

fn status_from_name(name: &str) -> Option<crate::ledger::Status> {
    open_name(name).or_else(|| closed_name(name))
}

fn open_name(name: &str) -> Option<crate::ledger::Status> {
    match name {
        "prepared" => Some(crate::ledger::Status::Prepared),
        "submitted" => Some(crate::ledger::Status::Submitted),
        "withdrawn" => Some(crate::ledger::Status::Withdrawn),
        _ => None,
    }
}

fn closed_name(name: &str) -> Option<crate::ledger::Status> {
    match name {
        "interview" => Some(crate::ledger::Status::Interview),
        "rejected" => Some(crate::ledger::Status::Rejected),
        "silent-closed" => Some(crate::ledger::Status::SilentClosed),
        _ => None,
    }
}

fn map_open_status(status: crate::ledger::Status) -> Option<MappedStage> {
    match status {
        crate::ledger::Status::Prepared => Some(stage("intended", None)),
        crate::ledger::Status::Submitted => Some(stage("submitted", None)),
        crate::ledger::Status::Withdrawn => Some(stage("withdrawn", None)),
        crate::ledger::Status::Interview
        | crate::ledger::Status::Rejected
        | crate::ledger::Status::SilentClosed => None,
    }
}

fn map_closed_status(status: crate::ledger::Status) -> MappedStage {
    match status {
        crate::ledger::Status::Interview => stage("next-stage:1", Some("legacy-unknown")),
        crate::ledger::Status::Rejected => stage("rejected", Some("legacy-unknown")),
        crate::ledger::Status::SilentClosed => stage("silent", None),
        crate::ledger::Status::Prepared
        | crate::ledger::Status::Submitted
        | crate::ledger::Status::Withdrawn => {
            map_open_status(status).unwrap_or_else(|| stage("intended", None))
        }
    }
}

fn check_writer(writer: &str) -> Result<(), String> {
    if matches!(writer, "laptop-1" | "mac-mini") {
        Ok(())
    } else {
        Err("writer must be laptop-1 or mac-mini".into())
    }
}

fn private_path(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("/home/") || lower.contains("/users/") || lower.contains("~/")
}

fn payload_has_private_path(payload: &AppEventPayload) -> bool {
    private_path(&payload.company) || private_path(&payload.role) || private_path(&payload.writer)
}

fn current_rows(rows: &[LedgerRow]) -> Vec<&LedgerRow> {
    let mut latest: BTreeMap<&str, &LedgerRow> = BTreeMap::new();
    for row in rows {
        latest.insert(link_key(&row.link), row);
    }
    let mut out: Vec<&LedgerRow> = latest.into_values().collect();
    out.sort_by(|left, right| link_key(&left.link).cmp(link_key(&right.link)));
    out
}

enum Seen {
    New,
    Same,
    Conflict,
}

fn seen(
    existing: &[EventEnvelope<AppEventPayload>],
    event: &EventEnvelope<AppEventPayload>,
) -> Seen {
    match existing
        .iter()
        .find(|row| row.aggregate == event.aggregate && row.key == event.key)
    {
        None => Seen::New,
        Some(row) if row.payload == event.payload => Seen::Same,
        Some(_) => Seen::Conflict,
    }
}

fn conflict(key: &str) -> String {
    format!("idempotency conflict on key {key}")
}

fn build_event(
    row: &LedgerRow,
    writer: &str,
    at: &StockholmTime,
    existing: &[EventEnvelope<AppEventPayload>],
) -> Result<EventEnvelope<AppEventPayload>, String> {
    require_link(&row.link)?;
    let status = status_for(row, at)?;
    let key = link_key(&row.link);
    Ok(EventEnvelope {
        id: EventId(ulid::Ulid::new()),
        aggregate: AggregateId(key.to_string()),
        key: IdempotencyKey(format!("{key}\t{}", stage_token(&status))),
        version: Version(next_version(existing, key)),
        event_time: when(row, at)?,
        record_time: at.clone(),
        source: Source::Document,
        prev_hash: None,
        schema_v: SCHEMA_VERSION,
        payload: AppEventPayload {
            writer: writer.to_string(),
            company: row.company.clone(),
            role: row.role.clone(),
            status,
        },
    })
}

fn require_link(link: &str) -> Result<(), String> {
    if link.starts_with("https://") && link.len() > "https://".len() {
        Ok(())
    } else {
        Err("link must be an https URL".into())
    }
}

fn next_version(existing: &[EventEnvelope<AppEventPayload>], aggregate: &str) -> u64 {
    existing
        .iter()
        .filter(|event| event.aggregate.0 == aggregate)
        .map(|event| event.version.0)
        .max()
        .unwrap_or(0)
        + 1
}

fn status_for(row: &LedgerRow, at: &StockholmTime) -> Result<RecordStatus, String> {
    if let Some(status) = status_open(row) {
        return status;
    }
    status_closed(row, at)
}

fn status_open(row: &LedgerRow) -> Option<Result<RecordStatus, String>> {
    match row.status {
        crate::ledger::Status::Prepared => Some(Ok(RecordStatus::Intended)),
        crate::ledger::Status::Withdrawn => Some(Ok(RecordStatus::Withdrawn)),
        crate::ledger::Status::Submitted => Some(submitted_status(row)),
        crate::ledger::Status::Interview
        | crate::ledger::Status::Rejected
        | crate::ledger::Status::SilentClosed => None,
    }
}

fn status_closed(row: &LedgerRow, at: &StockholmTime) -> Result<RecordStatus, String> {
    match row.status {
        crate::ledger::Status::Interview => Ok(RecordStatus::NextStage {
            n: 1,
            evidence: evidence(row, at)?,
        }),
        crate::ledger::Status::Rejected => Ok(RecordStatus::Rejected {
            evidence: evidence(row, at)?,
        }),
        crate::ledger::Status::SilentClosed => Ok(RecordStatus::Silent {
            since: when(row, at)?,
        }),
        crate::ledger::Status::Prepared
        | crate::ledger::Status::Submitted
        | crate::ledger::Status::Withdrawn => {
            status_open(row).unwrap_or(Ok(RecordStatus::Intended))
        }
    }
}

fn submitted_status(row: &LedgerRow) -> Result<RecordStatus, String> {
    Ok(RecordStatus::Submitted {
        submit: submit_of(row)?,
    })
}

fn submit_of(row: &LedgerRow) -> Result<RecordSubmitTime, String> {
    match row.submit_time() {
        Some(crate::ledger::SubmitTime::Known(time)) => Ok(RecordSubmitTime::Known {
            at: StockholmTime::new(time)?,
        }),
        Some(crate::ledger::SubmitTime::Unknown) | None => Ok(RecordSubmitTime::Unknown),
    }
}

fn evidence(row: &LedgerRow, at: &StockholmTime) -> Result<Evidence, String> {
    Ok(Evidence {
        source: Source::LegacyUnknown,
        at: when(row, at)?,
        text: TextForm("legacy-unknown".into()),
    })
}

fn when(row: &LedgerRow, at: &StockholmTime) -> Result<StockholmTime, String> {
    match row.submit_time() {
        Some(crate::ledger::SubmitTime::Known(time)) => StockholmTime::new(time),
        Some(crate::ledger::SubmitTime::Unknown) | None => Ok(at.clone()),
    }
}

pub fn stage_token(status: &RecordStatus) -> String {
    early_token(status).unwrap_or_else(|| late_token(status))
}

fn early_token(status: &RecordStatus) -> Option<String> {
    match status {
        RecordStatus::Intended => Some("intended".to_string()),
        RecordStatus::Submitted { .. } => Some("submitted".to_string()),
        RecordStatus::Withdrawn => Some("withdrawn".to_string()),
        RecordStatus::Silent { .. } => Some("silent".to_string()),
        RecordStatus::NextStage { .. }
        | RecordStatus::Rejected { .. }
        | RecordStatus::Confirmed { .. }
        | RecordStatus::Expired { .. } => None,
    }
}

fn late_token(status: &RecordStatus) -> String {
    match status {
        RecordStatus::NextStage { n, .. } => format!("next-stage:{n}"),
        RecordStatus::Rejected { .. } => "rejected".to_string(),
        RecordStatus::Confirmed { .. } => "confirmed".to_string(),
        RecordStatus::Expired { .. } => "expired".to_string(),
        RecordStatus::Intended
        | RecordStatus::Submitted { .. }
        | RecordStatus::Withdrawn
        | RecordStatus::Silent { .. } => String::new(),
    }
}

fn ledger_tokens(rows: &[LedgerRow]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for row in current_rows(rows) {
        out.insert(
            link_key(&row.link).to_string(),
            map_ledger_status(row.status).token,
        );
    }
    out
}

fn record_tokens(
    events: &[EventEnvelope<AppEventPayload>],
) -> Result<BTreeMap<String, String>, String> {
    let rows = replay(events).map_err(|err| err.to_string())?;
    let mut out = BTreeMap::new();
    for row in rows {
        out.insert(row.aggregate.0, stage_token(&row.event.payload.status));
    }
    Ok(out)
}

fn classify(
    ledger: &BTreeMap<String, String>,
    record: &BTreeMap<String, String>,
    key: &str,
    matches: &mut Vec<String>,
    mismatches: &mut Vec<CompareMismatch>,
) {
    let left = ledger.get(key).map(String::as_str);
    let right = record.get(key).map(String::as_str);
    if left.is_some() && left == right {
        matches.push(key.to_string());
        return;
    }
    mismatches.push(CompareMismatch {
        link_key: key.to_string(),
        ledger: left.unwrap_or("missing").to_string(),
        record: right.unwrap_or("missing").to_string(),
    });
}
