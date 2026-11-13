//! Runner over a replay client: golden traces, no network.
mod common;
use common::*;
use evalbench_core::runner::{
    run_set, trace_for, ClientError, ModelClient, ModelReply, ReplayClient, RunError,
};
use evalbench_core::score::score;
use evalbench_core::trace::TraceRow;

fn replies() -> String {
    read(&fixtures().join("replay/replies.jsonl"))
}

#[test]
fn replay_run_matches_golden_traces() {
    let set = frozen_set();
    let mut client = ReplayClient::from_jsonl(&replies()).unwrap();
    let rows = run_set(&mut client, &set, "replay-001").unwrap();
    let lines: String = rows.iter().map(TraceRow::to_line).collect();
    assert_eq!(lines, read(&fixtures().join("replay/traces.jsonl")));
}

#[test]
fn trace_copies_tokens_cost_model_from_reply() {
    let set = frozen_set();
    let reply = ModelReply {
        task_id: "t01-full".into(),
        model_id: "synthetic-model-x".into(),
        text: "{\"gaps\":[]}".into(),
        tokens_in: 7,
        tokens_out: 3,
        cost_micro_usd: 99,
    };
    let row = trace_for(&set.tasks[0], &set.hash, "r9", 2, &reply);
    assert_eq!(
        (
            row.model_id.as_str(),
            row.tokens_in,
            row.tokens_out,
            row.cost_micro_usd,
            row.attempt,
            row.pass
        ),
        ("synthetic-model-x", 7, 3, 99, 2, true)
    );
    assert_eq!(
        (row.prompt_id.as_str(), row.task_set_hash.as_str()),
        ("gaps-v1", set.hash.as_str())
    );
    let wrong = ModelReply {
        text: "{\"gaps\":[\"RAG\"]}".into(),
        ..reply
    };
    assert!(!trace_for(&set.tasks[0], &set.hash, "r9", 1, &wrong).pass);
}

struct Failing;
impl ModelClient for Failing {
    fn complete(&mut self, task_id: &str, _prompt: &str) -> Result<ModelReply, ClientError> {
        Err(ClientError {
            task_id: task_id.into(),
            message: "synthetic outage".into(),
        })
    }
}

#[test]
fn client_error_aborts_naming_task() {
    let set = frozen_set();
    match run_set(&mut Failing, &set, "r1") {
        Err(RunError::Client(e)) => assert_eq!(e.task_id, "t01-full"),
        other => panic!("expected client error, got {other:?}"),
    }
    let mut short = ReplayClient::from_jsonl(replies().lines().next().unwrap()).unwrap();
    assert!(
        matches!(run_set(&mut short, &set, "r1"), Err(RunError::Client(e)) if e.task_id == "t02-gappy")
    );
}

#[test]
fn reply_for_wrong_task_is_rejected() {
    struct Liar;
    impl ModelClient for Liar {
        fn complete(&mut self, _t: &str, _p: &str) -> Result<ModelReply, ClientError> {
            Ok(ModelReply {
                task_id: "t12-no-proof-key".into(),
                model_id: "m".into(),
                text: "{\"gaps\":[]}".into(),
                tokens_in: 1,
                tokens_out: 1,
                cost_micro_usd: 0,
            })
        }
    }
    let set = frozen_set();
    assert!(matches!(
        run_set(&mut Liar, &set, "r1"),
        Err(RunError::ReplyTaskMismatch { expected, got }) if expected == "t01-full" && got == "t12-no-proof-key"
    ));
}

#[test]
fn bad_run_id_rejected() {
    let set = frozen_set();
    let mut client = ReplayClient::from_jsonl(&replies()).unwrap();
    assert!(matches!(
        run_set(&mut client, &set, "run 1"),
        Err(RunError::BadRunId(_))
    ));
    assert!(matches!(
        run_set(&mut client, &set, ""),
        Err(RunError::BadRunId(_))
    ));
}

#[test]
fn replay_rejects_duplicate_task() {
    let first = replies().lines().next().unwrap().to_string();
    let err = ReplayClient::from_jsonl(&format!("{first}\n{first}\n")).unwrap_err();
    assert_eq!(err.line, 2);
}

#[test]
fn replay_end_to_end_score_is_golden() {
    let set = frozen_set();
    let mut client = ReplayClient::from_jsonl(&replies()).unwrap();
    let rows = run_set(&mut client, &set, "replay-001").unwrap();
    let s = score(&set.hash, &set.ids(), &rows).unwrap();
    assert_eq!(s.to_json(), read(&fixtures().join("replay/score.json")));
}
