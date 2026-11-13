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

impl TraceRow {
    pub fn validate(&self) -> Result<(), TraceErrorKind> {
        unimplemented!("E1: TraceRow::validate")
    }

    /// Compact JSON plus `\n`.
    pub fn to_line(&self) -> String {
        unimplemented!("E1: TraceRow::to_line")
    }
}

pub fn parse_traces(jsonl: &str) -> Result<Vec<TraceRow>, TraceError> {
    let _ = jsonl;
    unimplemented!("E1: parse_traces")
}
