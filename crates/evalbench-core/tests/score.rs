//! Score: first-pass percent (floor, 0..=100) plus lower-median tokens of passing tasks.
mod common;
use common::*;
use evalbench_core::score::{
    lower_median, percent_floor, score, score_from_passes, select_run, Score, ScoreError,
};
use evalbench_core::trace::{parse_traces, TraceRow};

const H: &str = "55faad2e303c22039226da1295312218c6f3b51af4d643d00ab5af8364b4b3df";

fn ids(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("t{i:02}")).collect()
}

fn tr(task: &str, attempt: u32, pass: bool, tin: u64, tout: u64) -> TraceRow {
    TraceRow {
        schema_version: 1,
        run_id: "r1".into(),
        task_id: task.into(),
        attempt,
        model_id: "synthetic-model".into(),
        prompt_id: "gaps-v1".into(),
        tokens_in: tin,
        tokens_out: tout,
        cost_micro_usd: 0,
        pass,
        task_set_hash: H.into(),
    }
}

fn golden_score() -> Score {
    let traces = parse_traces(&read(&fixtures().join("replay/traces.jsonl"))).unwrap();
    let set = frozen_set();
    score(&set.hash, &set.ids(), &traces).unwrap()
}

#[test]
fn score_is_floor_of_first_pass_percent() {
    for (p, t, want) in [
        (0, 3, 0),
        (1, 3, 33),
        (2, 3, 66),
        (3, 3, 100),
        (7, 12, 58),
        (0, 0, 0),
        (5, 3, 100),
    ] {
        assert_eq!(percent_floor(p, t), want, "{p}/{t}");
    }
    assert_eq!(score_from_passes(&[true, false, true]), 66);
    assert_eq!(score_from_passes(&[]), 0);

    let t = ids(3);
    let rows = vec![
        tr("t01", 1, true, 10, 0),
        tr("t02", 1, true, 20, 0),
        tr("t03", 1, false, 30, 0),
    ];
    let s = score(H, &t, &rows).unwrap();
    assert_eq!((s.tasks, s.first_pass, s.score, s.missing), (3, 2, 66, 0));
}

#[test]
fn missing_first_attempt_counts_as_fail() {
    let s = score(H, &ids(3), &[tr("t01", 1, true, 5, 5)]).unwrap();
    assert_eq!((s.first_pass, s.score, s.missing), (1, 33, 2));
    let none = score(H, &ids(3), &[]).unwrap();
    assert_eq!(
        (none.score, none.missing, none.median_tokens_passing),
        (0, 3, None)
    );
}

#[test]
fn retries_do_not_count() {
    let rows = vec![tr("t01", 1, false, 5, 5), tr("t01", 2, true, 5, 5)];
    let s = score(H, &ids(1), &rows).unwrap();
    assert_eq!(
        (s.first_pass, s.score, s.median_tokens_passing),
        (0, 0, None)
    );
}

#[test]
fn median_is_lower_median_of_passing_tokens() {
    assert_eq!(lower_median(&[10, 30, 20]), Some(20));
    assert_eq!(lower_median(&[10, 40, 20, 30]), Some(20));
    assert_eq!(lower_median(&[7]), Some(7));
    assert_eq!(lower_median(&[]), None);

    let rows = vec![
        tr("t01", 1, true, 8, 2),     // 10
        tr("t02", 1, true, 30, 10),   // 40
        tr("t03", 1, false, 900, 99), // failing: excluded
        tr("t04", 1, true, 15, 5),    // 20
        tr("t05", 1, true, 25, 5),    // 30
    ];
    let s = score(H, &ids(5), &rows).unwrap();
    assert_eq!(
        s.median_tokens_passing,
        Some(20),
        "tokens = in + out, passing only"
    );
    assert_eq!(s.score, 80);
}

/// P2: empty evidence gives 0 — empty task set.
#[test]
fn empty_task_set_scores_zero() {
    let s = score(H, &[], &[]).unwrap();
    assert_eq!((s.tasks, s.first_pass, s.score), (0, 0, 0));
    assert_eq!((s.median_tokens_passing, s.run_id.clone()), (None, None));
}

/// P2: empty evidence gives 0 — no evidenced first-attempt pass.
#[test]
fn no_passing_first_attempt_scores_zero() {
    let rows: Vec<TraceRow> = ids(4).iter().map(|t| tr(t, 1, false, 10, 10)).collect();
    let s = score(H, &ids(4), &rows).unwrap();
    assert_eq!(
        (s.first_pass, s.score, s.median_tokens_passing),
        (0, 0, None)
    );
}

#[test]
fn rescoring_same_traces_is_identical() {
    let text = read(&fixtures().join("replay/traces.jsonl"));
    let traces = parse_traces(&text).unwrap();
    let set = frozen_set();
    let a = score(&set.hash, &set.ids(), &traces).unwrap();
    let b = score(&set.hash, &set.ids(), &traces).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.to_json(), b.to_json());

    let mut shuffled = traces.clone();
    shuffled.reverse();
    shuffled.swap(0, 5);
    assert_eq!(
        score(&set.hash, &set.ids(), &shuffled).unwrap().to_json(),
        a.to_json()
    );

    assert_eq!(a.to_json(), read(&fixtures().join("replay/score.json")));
    assert_eq!(
        (a.tasks, a.first_pass, a.score, a.median_tokens_passing),
        (12, 7, 58, Some(421))
    );
}

#[test]
fn score_errors() {
    let t = ids(2);
    let mut other_run = tr("t02", 1, true, 1, 1);
    other_run.run_id = "r2".into();
    assert_eq!(
        score(H, &t, &[tr("t01", 1, true, 1, 1), other_run.clone()]),
        Err(ScoreError::MixedRuns(vec!["r1".into(), "r2".into()]))
    );
    let mut wrong = tr("t01", 1, true, 1, 1);
    wrong.task_set_hash = "0".repeat(64);
    assert!(
        matches!(score(H, &t, &[wrong]), Err(ScoreError::WrongTaskSet { task_id, .. }) if task_id == "t01")
    );
    assert_eq!(
        score(H, &t, &[tr("t09", 1, true, 1, 1)]),
        Err(ScoreError::UnknownTask("t09".into()))
    );
    assert_eq!(
        score(
            H,
            &t,
            &[tr("t01", 1, true, 1, 1), tr("t01", 1, false, 1, 1)]
        ),
        Err(ScoreError::DuplicateFirstAttempt("t01".into()))
    );
    assert_eq!(
        score(H, &["t01".into(), "t01".into()], &[]),
        Err(ScoreError::DuplicateTaskId("t01".into()))
    );
    assert_eq!(
        score(H, &t, &[tr("t01", 1, true, u64::MAX, 1)]),
        Err(ScoreError::TokenOverflow("t01".into()))
    );

    let rows = vec![tr("t01", 1, true, 1, 1), other_run];
    let only_r2 = select_run(&rows, "r2");
    assert_eq!(only_r2.len(), 1);
    assert_eq!(
        score(H, &t, &only_r2).unwrap().run_id.as_deref(),
        Some("r2")
    );
}

#[test]
fn score_json_field_order() {
    let s = golden_score();
    let j = s.to_json();
    assert!(!j.ends_with('\n'));
    let keys = [
        "schema_version",
        "run_id",
        "task_set_hash",
        "tasks",
        "first_pass",
        "score",
        "median_tokens_passing",
        "missing",
        "model_ids",
    ];
    let pos: Vec<usize> = keys
        .iter()
        .map(|k| j.find(&format!("\"{k}\":")).expect(k))
        .collect();
    let mut sorted = pos.clone();
    sorted.sort();
    assert_eq!(pos, sorted, "{j}");
    let empty = score(H, &[], &[]).unwrap().to_json();
    assert!(
        empty.contains("\"run_id\":null") && empty.contains("\"median_tokens_passing\":null"),
        "{empty}"
    );
}
