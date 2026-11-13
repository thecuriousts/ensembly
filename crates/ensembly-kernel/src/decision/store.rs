//! Append-only decision event log. Callers outside this crate go through
//! [`crate::KernelWriter`].

use anyhow::{bail, Context};
use ensembly_schema::{decode_line, encode_line, Checked, DecisionEvent};
use std::path::Path;

pub(crate) fn parse_log(text: &str) -> anyhow::Result<Vec<DecisionEvent>> {
    let mut out = Vec::new();
    for (i, raw) in text.split('\n').enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        let decoded =
            decode_line(raw).map_err(|e| anyhow::anyhow!("decision event line {}: {e}", i + 1))?;
        out.push(decoded.value);
    }
    Ok(out)
}

pub fn load_events(path: &Path) -> anyhow::Result<Vec<DecisionEvent>> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_log(&text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err).with_context(|| format!("read {}", path.display())),
    }
}

pub(crate) fn events_for<'a>(log: &'a [DecisionEvent], rid: &str) -> Vec<&'a DecisionEvent> {
    log.iter().filter(|e| e.rid == rid).collect()
}

pub(crate) fn encode_event(ev: &DecisionEvent) -> anyhow::Result<String> {
    ev.check().map_err(|e| anyhow::anyhow!(e))?;
    encode_line(ensembly_schema::SCHEMA_VERSION, ev).map_err(|e| anyhow::anyhow!(e))
}

/// Append one event line. Returns false when the same (rid, event_type) is
/// already present (idempotent retry). Refuses a second, different payload
/// for the same step.
pub(crate) fn append_event_text(
    before: &str,
    ev: &DecisionEvent,
) -> anyhow::Result<(String, bool)> {
    ev.check().map_err(|e| anyhow::anyhow!(e))?;
    let existing = parse_log(before)?;
    if let Some(old) = existing
        .iter()
        .find(|e| e.rid == ev.rid && e.event_type == ev.event_type)
    {
        if old.decision == ev.decision
            && old.writer == ev.writer
            && old.snapshot_hash == ev.snapshot_hash
            && old.reason == ev.reason
        {
            return Ok((before.to_string(), false));
        }
        bail!(
            "idempotency conflict on rid {} step {}: a different payload is already stored",
            ev.rid,
            ev.event_type.as_str()
        );
    }
    let line = encode_event(ev)?;
    let mut after = before.to_string();
    if !after.is_empty() && !after.ends_with('\n') {
        after.push('\n');
    }
    after.push_str(&line);
    if !after.ends_with('\n') {
        after.push('\n');
    }
    Ok((after, true))
}
