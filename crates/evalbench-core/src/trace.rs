//! Trace schema v1: one compact JSON object per line, keys in declaration order.
//! Traces never hold prompt or response text (that is `replies.jsonl`).

use serde::{Deserialize, Serialize};

pub const TRACE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceRow {
    pub schema_version: u32,
    /// `[A-Za-z0-9._-]{1,64}`
    pub run_id: String,
    /// `[a-z0-9][a-z0-9-]{0,63}`
    pub task_id: String,
    /// >= 1; 1 is the first run.
    pub attempt: u32,
    pub model_id: String,
    pub prompt_id: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_micro_usd: u64,
    /// Set by the frozen checker, never by the harness (D16).
    pub pass: bool,
    /// 64 lowercase hex.
    pub task_set_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceErrorKind {
    Json(String),
    Version(u32),
    Attempt,
    EmptyField(&'static str),
    BadId(&'static str),
    BadHash,
    BlankLine,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("trace line {line}: {kind:?}")]
pub struct TraceError {
    /// 1-based.
    pub line: usize,
    pub kind: TraceErrorKind,
}

/// `[A-Za-z0-9._-]{1,64}`
pub fn valid_run_id(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// `[a-z0-9][a-z0-9-]{0,63}`
pub fn valid_task_id(s: &str) -> bool {
    let lower_or_digit = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    let bytes = s.as_bytes();
    (1..=64).contains(&bytes.len())
        && lower_or_digit(bytes[0])
        && bytes[1..].iter().all(|&b| lower_or_digit(b) || b == b'-')
}

/// 64 lowercase hex.
pub fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl TraceRow {
    pub fn validate(&self) -> Result<(), TraceErrorKind> {
        if self.schema_version != TRACE_SCHEMA_VERSION {
            return Err(TraceErrorKind::Version(self.schema_version));
        }
        if self.run_id.is_empty() {
            return Err(TraceErrorKind::EmptyField("run_id"));
        }
        if !valid_run_id(&self.run_id) {
            return Err(TraceErrorKind::BadId("run_id"));
        }
        if self.task_id.is_empty() {
            return Err(TraceErrorKind::EmptyField("task_id"));
        }
        if !valid_task_id(&self.task_id) {
            return Err(TraceErrorKind::BadId("task_id"));
        }
        if self.attempt < 1 {
            return Err(TraceErrorKind::Attempt);
        }
        if self.model_id.is_empty() {
            return Err(TraceErrorKind::EmptyField("model_id"));
        }
        if self.prompt_id.is_empty() {
            return Err(TraceErrorKind::EmptyField("prompt_id"));
        }
        if !valid_hash(&self.task_set_hash) {
            return Err(TraceErrorKind::BadHash);
        }
        Ok(())
    }

    /// Compact JSON plus `\n`.
    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).expect("TraceRow serializes");
        s.push('\n');
        s
    }
}

/// Splits JSONL into `(1-based line, text)`. Blank lines are rejected, except that the
/// text may end with a single `\n`. Empty input is zero lines.
pub fn jsonl_lines(jsonl: &str) -> Result<Vec<(usize, &str)>, usize> {
    let body = jsonl.strip_suffix('\n').unwrap_or(jsonl);
    if jsonl.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for (i, line) in body.split('\n').enumerate() {
        if line.trim().is_empty() {
            return Err(i + 1);
        }
        out.push((i + 1, line));
    }
    Ok(out)
}

pub fn parse_traces(jsonl: &str) -> Result<Vec<TraceRow>, TraceError> {
    let lines = jsonl_lines(jsonl).map_err(|line| TraceError {
        line,
        kind: TraceErrorKind::BlankLine,
    })?;
    let mut rows = Vec::with_capacity(lines.len());
    for (line, text) in lines {
        let row: TraceRow = serde_json::from_str(text).map_err(|e| TraceError {
            line,
            kind: TraceErrorKind::Json(e.to_string()),
        })?;
        row.validate().map_err(|kind| TraceError { line, kind })?;
        rows.push(row);
    }
    Ok(rows)
}
