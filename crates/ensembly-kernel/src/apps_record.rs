//! Read the application ledger and append record events. Compare only reads.

use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

use ensembly_schema::{
    commit_json, compare_ledger, import_plan, parse_event_log, AppEventPayload, CommitFault,
    CompareReport, EventEnvelope,
};
use serde::Serialize;

use crate::application_ledger::{self, link_key};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportReceipt {
    pub appended: usize,
    pub skipped: usize,
}

pub fn import_ledger(
    ledger: &Path,
    log: &Path,
    writer: &str,
    observed_at: &str,
) -> Result<ImportReceipt, String> {
    let before = read_only(ledger)?;
    let text = std::str::from_utf8(&before).map_err(|err| err.to_string())?;
    let rows = application_ledger::parse_ledger(text).map_err(|err| err.to_string())?;
    let existing = read_log(log, true)?;
    let planned = import_plan(&existing, &rows, writer, observed_at)?;
    let current = current_count(&rows);
    if !planned.is_empty() {
        append_events(log, &planned)?;
    }
    let after = read_only(ledger)?;
    if after != before {
        return Err("ledger bytes changed during import".into());
    }
    Ok(ImportReceipt {
        appended: planned.len(),
        skipped: current.saturating_sub(planned.len()),
    })
}

pub fn compare_files(ledger: &Path, log: &Path) -> Result<CompareReport, String> {
    let bytes = read_only(ledger)?;
    let text = std::str::from_utf8(&bytes).map_err(|err| err.to_string())?;
    let rows = application_ledger::parse_ledger(text).map_err(|err| err.to_string())?;
    let events = read_log(log, false)?;
    compare_ledger(&rows, &events)
}

fn current_count(rows: &[application_ledger::Row]) -> usize {
    let mut keys = std::collections::BTreeSet::new();
    for row in rows {
        keys.insert(link_key(&row.link));
    }
    keys.len()
}

fn read_log(
    path: &Path,
    create_missing: bool,
) -> Result<Vec<EventEnvelope<AppEventPayload>>, String> {
    if path.is_dir() {
        return Err(format!("record log is a directory: {}", path.display()));
    }
    if !path.is_file() {
        return if create_missing {
            Ok(Vec::new())
        } else {
            Err(format!("record log is missing: {}", path.display()))
        };
    }
    let bytes = read_only(path)?;
    let text = std::str::from_utf8(&bytes).map_err(|err| err.to_string())?;
    parse_event_log(text).map_err(|err| err.to_string())
}

fn read_only(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(false)
        .open(path)
        .map_err(|err| format!("read {}: {err}", path.display()))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|err| format!("read {}: {err}", path.display()))?;
    Ok(buf)
}

fn append_events(log: &Path, events: &[EventEnvelope<AppEventPayload>]) -> Result<(), String> {
    let json = serde_json::to_string(events).map_err(|err| err.to_string())?;
    commit_json(log, &json, &CommitFault::default()).map_err(|err| err.to_string())?;
    Ok(())
}
