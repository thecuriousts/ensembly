//! Map application-ledger rows onto record events.
//!
//! The ledger stays the source of truth. This module only builds events.
//! `Confirmed` has no ledger source. `Rejected` and `NextStage` carry evidence
//! source `legacy-unknown` because the record type requires evidence and the
//! ledger has none. A row with no submit time stores the caller clock and sets
//! `synthetic_time`; that clock is not part of idempotency. Sameness uses the
//! ledger fields only, so a second import with another writer or another clock
//! appends nothing. A return to an earlier stage is refused.

use crate::record::{
    AggregateId, EventEnvelope, EventId, Evidence, IdempotencyKey, Rfc3339Time, Source,
    Status as RecordStatus, SubmitTime as RecordSubmitTime, TextForm, Version,
};
use crate::replay::replay;
use crate::surface::{require_configured_writer, validate_writer_identity};
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
    /// True when `event_time` is the import clock because the ledger has no submit time.
    pub synthetic_time: bool,
    pub status: RecordStatus,
}

impl crate::Checked for AppEventPayload {
    fn check(&self) -> Result<(), String> {
        validate_writer_identity(&self.writer)?;
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
    pub ledger_submit: String,
    pub record_submit: String,
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
    match map_open_status(status) {
        Some(mapped) => mapped,
        None => closed_stage(status).expect("closed ledger status"),
    }
}

pub fn closed_stage(status: crate::ledger::Status) -> Result<MappedStage, String> {
    match status {
        crate::ledger::Status::Interview => Ok(stage("next-stage:1", Some("legacy-unknown"))),
        crate::ledger::Status::Rejected => Ok(stage("rejected", Some("legacy-unknown"))),
        crate::ledger::Status::SilentClosed => Ok(stage("silent", None)),
        crate::ledger::Status::Prepared
        | crate::ledger::Status::Submitted
        | crate::ledger::Status::Withdrawn => Err(format!(
            "open ledger status {status:?} cannot be mapped as intended"
        )),
    }
}

pub fn closed_record_status(row: &LedgerRow, observed_at: &str) -> Result<RecordStatus, String> {
    let at = Rfc3339Time::new(observed_at)?;
    match row.status {
        crate::ledger::Status::Interview => Ok(RecordStatus::NextStage {
            n: 1,
            evidence: evidence(row, &at)?,
        }),
        crate::ledger::Status::Rejected => Ok(RecordStatus::Rejected {
            evidence: evidence(row, &at)?,
        }),
        crate::ledger::Status::SilentClosed => Ok(RecordStatus::Silent {
            since: when(row, &at)?,
        }),
        crate::ledger::Status::Prepared
        | crate::ledger::Status::Submitted
        | crate::ledger::Status::Withdrawn => Err(format!(
            "open ledger status {:?} cannot be mapped as intended",
            row.status
        )),
    }
}

pub fn import_plan(
    existing: &[EventEnvelope<AppEventPayload>],
    rows: &[LedgerRow],
    writer: &str,
    observed_at: &str,
) -> Result<Vec<EventEnvelope<AppEventPayload>>, String> {
    require_configured_writer(writer)?;
    let at = Rfc3339Time::new(observed_at)?;
    let mut planned = Vec::new();
    let mut staged = existing.to_vec();
    for row in current_rows(rows) {
        let event = build_event(row, writer, &at, &staged)?;
        match seen(&staged, &event) {
            Seen::Same => {}
            Seen::Conflict { submit_time } => return Err(conflict(&event.key.0, submit_time)),
            Seen::New => {
                staged.push(event.clone());
                planned.push(event);
            }
        }
    }
    let report = diff(rows, &staged, false)?;
    if report.mismatches.is_empty() {
        Ok(planned)
    } else {
        Err(stage_diff(&report))
    }
}

pub fn compare_ledger(
    rows: &[LedgerRow],
    events: &[EventEnvelope<AppEventPayload>],
) -> Result<CompareReport, String> {
    diff(rows, events, true)
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
    Conflict { submit_time: bool },
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
        Some(row) if same_ledger(&row.payload, &event.payload) => Seen::Same,
        Some(row) => Seen::Conflict {
            submit_time: submit_time_changed(&row.payload, &event.payload),
        },
    }
}

fn same_ledger(left: &AppEventPayload, right: &AppEventPayload) -> bool {
    left.company == right.company
        && left.role == right.role
        && left.synthetic_time == right.synthetic_time
        && same_status(&left.status, &right.status, left.synthetic_time)
}

fn same_status(left: &RecordStatus, right: &RecordStatus, synthetic: bool) -> bool {
    match same_plain(left, right) {
        Some(same) => same,
        None => same_timed(left, right, synthetic),
    }
}

fn same_plain(left: &RecordStatus, right: &RecordStatus) -> Option<bool> {
    match (left, right) {
        (RecordStatus::Intended, RecordStatus::Intended)
        | (RecordStatus::Withdrawn, RecordStatus::Withdrawn) => Some(true),
        (RecordStatus::Submitted { submit: left }, RecordStatus::Submitted { submit: right }) => {
            Some(left == right)
        }
        (RecordStatus::Intended | RecordStatus::Withdrawn | RecordStatus::Submitted { .. }, _) => {
            Some(false)
        }
        _ => None,
    }
}

fn same_timed(left: &RecordStatus, right: &RecordStatus, synthetic: bool) -> bool {
    match (left, right) {
        (RecordStatus::Rejected { evidence: left }, RecordStatus::Rejected { evidence: right }) => {
            same_evidence(left, right, synthetic)
        }
        (
            RecordStatus::NextStage {
                n: left_n,
                evidence: left,
            },
            RecordStatus::NextStage {
                n: right_n,
                evidence: right,
            },
        ) => left_n == right_n && same_evidence(left, right, synthetic),
        (RecordStatus::Silent { since: left }, RecordStatus::Silent { since: right })
        | (RecordStatus::Expired { at: left }, RecordStatus::Expired { at: right }) => {
            synthetic || left == right
        }
        (
            RecordStatus::Confirmed { evidence: left },
            RecordStatus::Confirmed { evidence: right },
        ) => same_evidence(left, right, synthetic),
        _ => false,
    }
}

fn same_evidence(left: &Evidence, right: &Evidence, synthetic: bool) -> bool {
    left.source == right.source && left.text == right.text && (synthetic || left.at == right.at)
}

fn submit_time_changed(left: &AppEventPayload, right: &AppEventPayload) -> bool {
    if left.synthetic_time && right.synthetic_time {
        return false;
    }
    clock_of(&left.status) != clock_of(&right.status)
}

fn conflict(key: &str, submit_time: bool) -> String {
    if submit_time {
        format!(
            "idempotency conflict on key {key}: submit time on this stage differs from the log. Import does not rewrite a stored event. Append a later ledger stage, then import again."
        )
    } else {
        format!("idempotency conflict on key {key}")
    }
}

fn build_event(
    row: &LedgerRow,
    writer: &str,
    at: &Rfc3339Time,
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
            synthetic_time: !known_submit(row),
            status,
        },
    })
}

fn known_submit(row: &LedgerRow) -> bool {
    matches!(row.submit_time(), Some(crate::ledger::SubmitTime::Known(_)))
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

fn status_for(row: &LedgerRow, at: &Rfc3339Time) -> Result<RecordStatus, String> {
    if let Some(status) = status_open(row) {
        return status;
    }
    closed_record_status(row, at.as_str())
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

fn submitted_status(row: &LedgerRow) -> Result<RecordStatus, String> {
    Ok(RecordStatus::Submitted {
        submit: submit_of(row)?,
    })
}

fn submit_of(row: &LedgerRow) -> Result<RecordSubmitTime, String> {
    match row.submit_time() {
        Some(crate::ledger::SubmitTime::Known(time)) => Ok(RecordSubmitTime::Known {
            at: Rfc3339Time::new(time)?,
        }),
        Some(crate::ledger::SubmitTime::Unknown) | None => Ok(RecordSubmitTime::Unknown),
    }
}

fn evidence(row: &LedgerRow, at: &Rfc3339Time) -> Result<Evidence, String> {
    Ok(Evidence {
        source: Source::LegacyUnknown,
        at: when(row, at)?,
        text: TextForm("legacy-unknown".into()),
    })
}

fn when(row: &LedgerRow, at: &Rfc3339Time) -> Result<Rfc3339Time, String> {
    match row.submit_time() {
        Some(crate::ledger::SubmitTime::Known(time)) => Rfc3339Time::new(time),
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

struct Fact {
    stage: String,
    submit: String,
}

fn diff(
    rows: &[LedgerRow],
    events: &[EventEnvelope<AppEventPayload>],
    times: bool,
) -> Result<CompareReport, String> {
    let ledger = ledger_facts(rows);
    let record = record_facts(events)?;
    let mut keys = BTreeSet::new();
    keys.extend(ledger.keys().cloned());
    keys.extend(record.keys().cloned());
    let mut matches = Vec::new();
    let mut mismatches = Vec::new();
    for key in keys {
        place(&ledger, &record, &key, times, &mut matches, &mut mismatches);
    }
    Ok(CompareReport {
        matches,
        mismatches,
    })
}

fn stage_diff(report: &CompareReport) -> String {
    let parts: Vec<String> = report
        .mismatches
        .iter()
        .map(|row| {
            format!(
                "{} ledger {} record {}",
                row.link_key, row.ledger, row.record
            )
        })
        .collect();
    format!("record stage differs from the ledger: {}", parts.join("; "))
}

fn ledger_facts(rows: &[LedgerRow]) -> BTreeMap<String, Fact> {
    let mut out = BTreeMap::new();
    for row in current_rows(rows) {
        out.insert(
            link_key(&row.link).to_string(),
            Fact {
                stage: map_ledger_status(row.status).token,
                submit: ledger_submit(row),
            },
        );
    }
    out
}

fn ledger_submit(row: &LedgerRow) -> String {
    match row.submit_time() {
        Some(crate::ledger::SubmitTime::Known(time)) => time.to_string(),
        Some(crate::ledger::SubmitTime::Unknown) | None => "unknown".to_string(),
    }
}

fn record_facts(
    events: &[EventEnvelope<AppEventPayload>],
) -> Result<BTreeMap<String, Fact>, String> {
    let rows = replay(events).map_err(|err| err.to_string())?;
    let mut out = BTreeMap::new();
    for row in rows {
        out.insert(row.aggregate.0, fact_of(&row.event.payload));
    }
    Ok(out)
}

fn fact_of(payload: &AppEventPayload) -> Fact {
    Fact {
        stage: stage_token(&payload.status),
        submit: if payload.synthetic_time {
            "unknown".to_string()
        } else {
            clock_of(&payload.status)
        },
    }
}

fn clock_of(status: &RecordStatus) -> String {
    match clock_open(status) {
        Some(clock) => clock,
        None => clock_closed(status),
    }
}

fn clock_open(status: &RecordStatus) -> Option<String> {
    match status {
        RecordStatus::Submitted {
            submit: RecordSubmitTime::Known { at },
        } => Some(at.as_str().to_string()),
        RecordStatus::Submitted {
            submit: RecordSubmitTime::Unknown,
        }
        | RecordStatus::Intended
        | RecordStatus::Withdrawn => Some("unknown".to_string()),
        RecordStatus::Silent { .. }
        | RecordStatus::Rejected { .. }
        | RecordStatus::NextStage { .. }
        | RecordStatus::Confirmed { .. }
        | RecordStatus::Expired { .. } => None,
    }
}

fn clock_closed(status: &RecordStatus) -> String {
    match status {
        RecordStatus::Silent { since } | RecordStatus::Expired { at: since } => {
            since.as_str().to_string()
        }
        RecordStatus::Rejected { evidence }
        | RecordStatus::NextStage { evidence, .. }
        | RecordStatus::Confirmed { evidence } => evidence.at.as_str().to_string(),
        RecordStatus::Intended | RecordStatus::Submitted { .. } | RecordStatus::Withdrawn => {
            "unknown".to_string()
        }
    }
}

fn place(
    ledger: &BTreeMap<String, Fact>,
    record: &BTreeMap<String, Fact>,
    key: &str,
    times: bool,
    matches: &mut Vec<String>,
    mismatches: &mut Vec<CompareMismatch>,
) {
    let left = ledger.get(key);
    let right = record.get(key);
    if agreed(left, right, times) {
        matches.push(key.to_string());
        return;
    }
    mismatches.push(mismatch(key, left, right));
}

fn agreed(left: Option<&Fact>, right: Option<&Fact>, times: bool) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.stage == right.stage && (!times || left.submit == right.submit)
        }
        _ => false,
    }
}

fn mismatch(key: &str, left: Option<&Fact>, right: Option<&Fact>) -> CompareMismatch {
    CompareMismatch {
        link_key: key.to_string(),
        ledger: left
            .map(|fact| fact.stage.clone())
            .unwrap_or_else(|| "missing".into()),
        record: right
            .map(|fact| fact.stage.clone())
            .unwrap_or_else(|| "missing".into()),
        ledger_submit: left
            .map(|fact| fact.submit.clone())
            .unwrap_or_else(|| "missing".into()),
        record_submit: right
            .map(|fact| fact.submit.clone())
            .unwrap_or_else(|| "missing".into()),
    }
}
