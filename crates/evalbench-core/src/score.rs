//! Score, verbatim from the plan: first-pass correctness = tasks passing their frozen
//! checker on the first run, divided by tasks, plus median tokens per passing task.
//! `score` = floor(100 * first_pass / tasks) (D1); 0 when there are no tasks or no
//! first-attempt pass (P2). Median tokens are reported beside, never folded in.

use std::collections::{BTreeMap, BTreeSet};

use crate::trace::TraceRow;

fn json_str(s: &str) -> String {
    serde_json::to_string(s).expect("string serializes")
}

fn json_opt<T: ToString>(v: Option<T>) -> String {
    v.map_or_else(|| "null".to_string(), |x| x.to_string())
}

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
        let model_ids: Vec<String> = self.model_ids.iter().map(|m| json_str(m)).collect();
        format!(
            concat!(
                "{{\"schema_version\":{},\"run_id\":{},\"task_set_hash\":{},",
                "\"tasks\":{},\"first_pass\":{},\"score\":{},",
                "\"median_tokens_passing\":{},\"missing\":{},\"model_ids\":[{}]}}"
            ),
            self.schema_version,
            json_opt(self.run_id.as_deref().map(json_str)),
            json_str(&self.task_set_hash),
            self.tasks,
            self.first_pass,
            self.score,
            json_opt(self.median_tokens_passing),
            self.missing,
            model_ids.join(","),
        )
    }
}

/// floor(100 * min(passed, total) / total) in u64 math; 0 when total == 0.
pub fn percent_floor(passed: u32, total: u32) -> u8 {
    if total == 0 {
        return 0;
    }
    let passed = passed.min(total) as u64;
    // passed <= total, so the quotient is in 0..=100 and fits a u8.
    (100 * passed / total as u64) as u8
}

/// `percent_floor(count of true, len)`. `score()` must call this on the shipped path.
pub fn score_from_passes(passes: &[bool]) -> u8 {
    // Saturate rather than wrap: a slice longer than u32::MAX keeps the ratio in range.
    let total = u32::try_from(passes.len()).unwrap_or(u32::MAX);
    let passed = u32::try_from(passes.iter().filter(|p| **p).count()).unwrap_or(u32::MAX);
    percent_floor(passed, total)
}

/// `sorted[(n - 1) / 2]`; `None` when empty.
pub fn lower_median(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    Some(sorted[(sorted.len() - 1) / 2])
}

/// Rows whose `run_id` equals `run_id`, in input order.
pub fn select_run(traces: &[TraceRow], run_id: &str) -> Vec<TraceRow> {
    traces
        .iter()
        .filter(|t| t.run_id == run_id)
        .cloned()
        .collect()
}

pub fn score(
    task_set_hash: &str,
    task_ids: &[String],
    traces: &[TraceRow],
) -> Result<Score, ScoreError> {
    let tasks = u32::try_from(task_ids.len()).map_err(|_| ScoreError::TooManyTasks)?;
    let mut known = BTreeSet::new();
    for id in task_ids {
        if !known.insert(id.as_str()) {
            return Err(ScoreError::DuplicateTaskId(id.clone()));
        }
    }

    let runs: BTreeSet<&str> = traces.iter().map(|t| t.run_id.as_str()).collect();
    if runs.len() > 1 {
        return Err(ScoreError::MixedRuns(
            runs.into_iter().map(str::to_string).collect(),
        ));
    }

    // task id -> its attempt-1 row. Retries (attempt >= 2) are checked, then ignored.
    let mut first: BTreeMap<&str, &TraceRow> = BTreeMap::new();
    for row in traces {
        if !known.contains(row.task_id.as_str()) {
            return Err(ScoreError::UnknownTask(row.task_id.clone()));
        }
        if row.task_set_hash != task_set_hash {
            return Err(ScoreError::WrongTaskSet {
                task_id: row.task_id.clone(),
                expected: task_set_hash.to_string(),
                found: row.task_set_hash.clone(),
            });
        }
        if row.attempt == 1 && first.insert(row.task_id.as_str(), row).is_some() {
            return Err(ScoreError::DuplicateFirstAttempt(row.task_id.clone()));
        }
    }

    let mut passes = Vec::with_capacity(task_ids.len());
    let mut passing_tokens = Vec::new();
    let mut missing = 0u32;
    for id in task_ids {
        match first.get(id.as_str()) {
            Some(row) => {
                let tokens = row
                    .tokens_in
                    .checked_add(row.tokens_out)
                    .ok_or_else(|| ScoreError::TokenOverflow(row.task_id.clone()))?;
                passes.push(row.pass);
                if row.pass {
                    passing_tokens.push(tokens);
                }
            }
            None => {
                passes.push(false);
                missing += 1;
            }
        }
    }

    let first_pass = passes.iter().filter(|p| **p).count() as u32;
    let model_ids: BTreeSet<&str> = traces.iter().map(|t| t.model_id.as_str()).collect();
    Ok(Score {
        schema_version: SCORE_SCHEMA_VERSION,
        run_id: traces.first().map(|t| t.run_id.clone()),
        task_set_hash: task_set_hash.to_string(),
        tasks,
        first_pass,
        // The shipped path goes through score_from_passes, which P1 and P2 cover.
        score: score_from_passes(&passes),
        median_tokens_passing: lower_median(&passing_tokens),
        missing,
        model_ids: model_ids.into_iter().map(str::to_string).collect(),
    })
}
