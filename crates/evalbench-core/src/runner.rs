//! Runner: render the frozen prompt, ask a `ModelClient`, judge the reply with the
//! frozen checker, emit one `TraceRow` per task (attempt 1). No network here (D12).

use serde::{Deserialize, Serialize};

use crate::answer::check_answer;
use crate::taskset::{Task, TaskSet};
use crate::trace::{jsonl_lines, valid_run_id, TraceRow, TRACE_SCHEMA_VERSION};

pub const PROMPT_ID: &str = "gaps-v1";
pub const PROMPT_TEMPLATE: &str = include_str!("../fixtures/prompts/gaps-v1.txt");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelReply {
    pub task_id: String,
    pub model_id: String,
    pub text: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_micro_usd: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("model client failed on task {task_id}: {message}")]
pub struct ClientError {
    pub task_id: String,
    pub message: String,
}

pub trait ModelClient {
    fn complete(&mut self, task_id: &str, prompt: &str) -> Result<ModelReply, ClientError>;
}

/// Replays recorded replies keyed by task id.
#[derive(Debug, Clone, Default)]
pub struct ReplayClient {
    replies: std::collections::BTreeMap<String, ModelReply>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("replies line {line}: {msg}")]
pub struct ReplyParseError {
    pub line: usize,
    pub msg: String,
}

impl ReplayClient {
    /// Duplicate task_id is an error.
    pub fn from_jsonl(jsonl: &str) -> Result<ReplayClient, ReplyParseError> {
        let lines = jsonl_lines(jsonl).map_err(|line| ReplyParseError {
            line,
            msg: "blank line".into(),
        })?;
        let mut replies = std::collections::BTreeMap::new();
        for (line, text) in lines {
            let reply: ModelReply = serde_json::from_str(text).map_err(|e| ReplyParseError {
                line,
                msg: e.to_string(),
            })?;
            if replies.contains_key(&reply.task_id) {
                return Err(ReplyParseError {
                    line,
                    msg: format!("duplicate reply for task {}", reply.task_id),
                });
            }
            replies.insert(reply.task_id.clone(), reply);
        }
        Ok(ReplayClient { replies })
    }
}

impl ModelClient for ReplayClient {
    /// Unknown task -> `ClientError` naming it.
    fn complete(&mut self, task_id: &str, prompt: &str) -> Result<ModelReply, ClientError> {
        let _ = prompt;
        self.replies
            .get(task_id)
            .cloned()
            .ok_or_else(|| ClientError {
                task_id: task_id.to_string(),
                message: "no recorded reply for this task".into(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    #[error("bad run id: {0}")]
    BadRunId(String),
    #[error(transparent)]
    Client(ClientError),
    #[error("reply for task {got}, expected {expected}")]
    ReplyTaskMismatch { expected: String, got: String },
}

/// Replaces `{{posting}}` and `{{pack}}` with the raw file text, once each.
pub fn render_prompt(task: &Task) -> String {
    // One pass over the template, so a marker inside a fixture is never expanded.
    let mut out =
        String::with_capacity(PROMPT_TEMPLATE.len() + task.posting_raw.len() + task.pack_raw.len());
    let mut rest = PROMPT_TEMPLATE;
    let mut posting_done = false;
    let mut pack_done = false;
    loop {
        let next = [("{{posting}}", posting_done), ("{{pack}}", pack_done)]
            .into_iter()
            .filter(|(_, done)| !done)
            .filter_map(|(m, _)| rest.find(m).map(|i| (i, m)))
            .min();
        let Some((i, marker)) = next else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..i]);
        if marker == "{{posting}}" {
            out.push_str(&task.posting_raw);
            posting_done = true;
        } else {
            out.push_str(&task.pack_raw);
            pack_done = true;
        }
        rest = &rest[i + marker.len()..];
    }
}

/// `pass` = `check_answer(task.expected_gaps, reply.text)`; prompt_id = `PROMPT_ID`.
pub fn trace_for(
    task: &Task,
    task_set_hash: &str,
    run_id: &str,
    attempt: u32,
    reply: &ModelReply,
) -> TraceRow {
    TraceRow {
        schema_version: TRACE_SCHEMA_VERSION,
        run_id: run_id.to_string(),
        task_id: task.id.clone(),
        attempt,
        model_id: reply.model_id.clone(),
        prompt_id: PROMPT_ID.to_string(),
        tokens_in: reply.tokens_in,
        tokens_out: reply.tokens_out,
        cost_micro_usd: reply.cost_micro_usd,
        pass: check_answer(&task.expected_gaps, &reply.text),
        task_set_hash: task_set_hash.to_string(),
    }
}

/// attempt = 1 for every task, in id order. A client error stops the run (D11).
pub fn run_set(
    client: &mut dyn ModelClient,
    set: &TaskSet,
    run_id: &str,
) -> Result<Vec<TraceRow>, RunError> {
    if !valid_run_id(run_id) {
        return Err(RunError::BadRunId(run_id.to_string()));
    }
    let mut rows = Vec::with_capacity(set.tasks.len());
    for task in &set.tasks {
        let reply = client
            .complete(&task.id, &render_prompt(task))
            .map_err(RunError::Client)?;
        if reply.task_id != task.id {
            return Err(RunError::ReplyTaskMismatch {
                expected: task.id.clone(),
                got: reply.task_id,
            });
        }
        rows.push(trace_for(task, &set.hash, run_id, 1, &reply));
    }
    Ok(rows)
}
