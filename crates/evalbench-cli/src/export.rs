//! E4 export pack: the canonical pulse pack and the record view of evalbench runs in
//! one file, `ensembly-export-v1`, written as canonical JSON (sorted keys, integers
//! only, trailing newline) so the same inputs give the same bytes.
//!
//! The record view is what a reader needs to check a run: per task the result, the
//! tokens and the model, per run the first-pass score and the dataset hash, and the
//! commit the traces came from. The pulse part is read-only: a pack file goes through
//! the canonical checker; a store is opened `mode=ro` + `query_only`. Nothing here
//! writes to a pulse store.

use std::collections::BTreeMap;
use std::path::Path;

use ensembly_kernel::pulse_pack::canonical as canon;
use evalbench_core::score::score;
use evalbench_core::taskset::TaskSet;
use evalbench_core::trace::TraceRow;
use serde_json::{json, Value};

pub const EXPORT_FORMAT: &str = "ensembly-export-v1";

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("commit must be 40 lowercase hex characters: {0:?}")]
    BadCommit(String),
    #[error("no trace rows")]
    NoRuns,
    #[error("pulse pack is not valid: {0}")]
    Pulse(String),
    #[error("score: {0}")]
    Score(#[from] evalbench_core::score::ScoreError),
    #[error("pulse store: {0:#}")]
    Store(anyhow::Error),
}

/// Where the pulse part comes from.
pub enum PulseSource<'a> {
    None,
    /// Pack bytes in any format the canonical checker reads.
    Pack(&'a [u8]),
    /// A pulse store, opened read-only.
    Store {
        path: &'a Path,
        exported_at: &'a str,
        host: &'a str,
    },
}

fn valid_commit(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn issues(v: &canon::Verdict) -> String {
    v.errors
        .iter()
        .map(|i| format!("{} {}", i.code, if i.path.is_empty() { "/" } else { &i.path }))
        .collect::<Vec<_>>()
        .join(", ")
}

fn pulse_part(src: PulseSource) -> Result<Value, ExportError> {
    let doc = match src {
        PulseSource::None => return Ok(Value::Null),
        PulseSource::Pack(bytes) => {
            let (doc, v) = canon::check_bytes(bytes);
            doc.ok_or_else(|| ExportError::Pulse(issues(&v)))?
        }
        PulseSource::Store {
            path,
            exported_at,
            host,
        } => {
            let (doc, v) = canon::canonical_from_db_path(path, exported_at, host)
                .map_err(ExportError::Store)?;
            doc.ok_or_else(|| ExportError::Pulse(issues(&v)))?
        }
    };
    let traces = doc["traces"].as_array().map_or(0, Vec::len);
    Ok(json!({
        "content_sha256": canon::content_sha256(&doc),
        "pack": doc.clone(),
        "pack_sha256": canon::pack_sha256(&doc),
        "traces": traces,
    }))
}

/// The export document. Runs are sorted by run id; rows by task id, then attempt.
pub fn build(
    set: &TaskSet,
    traces: &[TraceRow],
    commit: &str,
    pulse: PulseSource,
) -> Result<Value, ExportError> {
    if !valid_commit(commit) {
        return Err(ExportError::BadCommit(commit.to_string()));
    }
    let mut by_run: BTreeMap<&str, Vec<&TraceRow>> = BTreeMap::new();
    for t in traces {
        by_run.entry(t.run_id.as_str()).or_default().push(t);
    }
    if by_run.is_empty() {
        return Err(ExportError::NoRuns);
    }
    let mut runs = Vec::new();
    for (run_id, rows) in by_run {
        let owned: Vec<TraceRow> = rows.iter().map(|r| (*r).clone()).collect();
        let s = score(&set.hash, &set.ids(), &owned)?;
        let mut rows = owned;
        rows.sort_by(|a, b| (&a.task_id, a.attempt).cmp(&(&b.task_id, b.attempt)));
        let rows: Vec<Value> = rows
            .iter()
            .map(|r| {
                json!({
                    "attempt": r.attempt,
                    "model_id": r.model_id,
                    "pass": r.pass,
                    "prompt_id": r.prompt_id,
                    "task_id": r.task_id,
                    "tokens_in": r.tokens_in,
                    "tokens_out": r.tokens_out,
                })
            })
            .collect();
        runs.push(json!({
            "first_pass": s.first_pass,
            "median_tokens_passing": s.median_tokens_passing,
            "missing": s.missing,
            "model_ids": s.model_ids,
            "rows": rows,
            "run_id": run_id,
            "score": s.score,
            "task_set_hash": s.task_set_hash,
            "tasks": s.tasks,
        }));
    }
    Ok(json!({
        "commit": commit,
        "format": EXPORT_FORMAT,
        "pulse": pulse_part(pulse)?,
        "runs": runs,
    }))
}

/// Canonical bytes of an export document.
pub fn to_bytes(doc: &Value) -> Vec<u8> {
    canon::canonical_bytes(doc).expect("export holds integers only")
}
