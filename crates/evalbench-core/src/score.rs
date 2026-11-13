//! Score, verbatim from the plan: first-pass correctness = tasks passing their frozen
//! checker on the first run, divided by tasks, plus median tokens per passing task.
//! `score` = floor(100 * first_pass / tasks) (D1); 0 when there are no tasks or no
//! first-attempt pass (P2). Median tokens are reported beside, never folded in.

use crate::trace::TraceRow;

pub const SCORE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Score {
    pub schema_version: u32,
    pub run_id: Option<String>,
    pub task_set_hash: String,
    pub tasks: u32,
    pub first_pass: u32,
    /// 0..=100.
    pub score: u8,
    /// Lower median of tokens_in + tokens_out over passing first attempts (D4).
    pub median_tokens_passing: Option<u64>,
    /// Tasks with no attempt-1 row; counted as fails (D8).
    pub missing: u32,
    /// Sorted, deduplicated.
    pub model_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScoreError {
    #[error("traces hold more than one run: {0:?}")]
    MixedRuns(Vec<String>),
    #[error("task {task_id}: trace hash {found} is not the task set {expected}")]
    WrongTaskSet {
        task_id: String,
        expected: String,
        found: String,
    },
    #[error("unknown task: {0}")]
    UnknownTask(String),
    #[error("two first attempts for task {0}")]
    DuplicateFirstAttempt(String),
    #[error("duplicate task id in the set: {0}")]
    DuplicateTaskId(String),
    #[error("token count overflows for task {0}")]
    TokenOverflow(String),
    #[error("more tasks than u32")]
    TooManyTasks,
}

impl Score {
    /// Compact JSON, fields in declaration order, no trailing newline.
    pub fn to_json(&self) -> String {
        unimplemented!("E1: Score::to_json")
    }
}

/// floor(100 * min(passed, total) / total) in u64 math; 0 when total == 0.
pub fn percent_floor(passed: u32, total: u32) -> u8 {
    let _ = (passed, total);
    unimplemented!("E1: percent_floor")
}

/// `percent_floor(count of true, len)`. `score()` must call this on the shipped path.
pub fn score_from_passes(passes: &[bool]) -> u8 {
    let _ = passes;
    unimplemented!("E1: score_from_passes")
}

/// `sorted[(n - 1) / 2]`; `None` when empty.
pub fn lower_median(values: &[u64]) -> Option<u64> {
    let _ = values;
    unimplemented!("E1: lower_median")
}

/// Rows whose `run_id` equals `run_id`, in input order.
pub fn select_run(traces: &[TraceRow], run_id: &str) -> Vec<TraceRow> {
    let _ = (traces, run_id);
    unimplemented!("E1: select_run")
}

pub fn score(
    task_set_hash: &str,
    task_ids: &[String],
    traces: &[TraceRow],
) -> Result<Score, ScoreError> {
    let _ = (task_set_hash, task_ids, traces);
    unimplemented!("E1: score")
}
