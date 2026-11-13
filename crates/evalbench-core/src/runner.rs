//! Runner: render the frozen prompt, ask a `ModelClient`, judge the reply with the
//! frozen checker, emit one `TraceRow` per task (attempt 1). No network here (D12).

use serde::{Deserialize, Serialize};

use crate::taskset::{Task, TaskSet};
use crate::trace::TraceRow;

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
        let _ = jsonl;
        unimplemented!("E1: ReplayClient::from_jsonl")
    }
}

impl ModelClient for ReplayClient {
    /// Unknown task -> `ClientError` naming it.
    fn complete(&mut self, task_id: &str, prompt: &str) -> Result<ModelReply, ClientError> {
        let _ = (task_id, prompt, &self.replies);
        unimplemented!("E1: ReplayClient::complete")
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
    let _ = task;
    unimplemented!("E1: render_prompt")
}

/// `pass` = `check_answer(task.expected_gaps, reply.text)`; prompt_id = `PROMPT_ID`.
pub fn trace_for(
    task: &Task,
    task_set_hash: &str,
    run_id: &str,
    attempt: u32,
    reply: &ModelReply,
) -> TraceRow {
    let _ = (task, task_set_hash, run_id, attempt, reply);
    unimplemented!("E1: trace_for")
}

/// attempt = 1 for every task, in id order. A client error stops the run (D11).
pub fn run_set(
    client: &mut dyn ModelClient,
    set: &TaskSet,
    run_id: &str,
) -> Result<Vec<TraceRow>, RunError> {
    let _ = (client, set, run_id);
    unimplemented!("E1: run_set")
}
