//! Trace schema v1.
mod common;
use common::*;
use evalbench_core::trace::{parse_traces, TraceErrorKind, TraceRow, TRACE_SCHEMA_VERSION};

fn row() -> TraceRow {
    TraceRow {
        schema_version: TRACE_SCHEMA_VERSION,
        run_id: "replay-001".into(),
        task_id: "t01-full".into(),
        attempt: 1,
        model_id: "synthetic-replay-model".into(),
        prompt_id: "gaps-v1".into(),
        tokens_in: 410,
        tokens_out: 12,
        cost_micro_usd: 1410,
        pass: true,
        task_set_hash: committed_hash(),
    }
}

#[test]
fn trace_row_round_trips_byte_exact() {
    let golden = read(&fixtures().join("replay/traces.jsonl"));
    let first = golden.lines().next().unwrap();
    assert_eq!(row().to_line(), format!("{first}\n"));
    let rows = parse_traces(&golden).unwrap();
    assert_eq!(rows.len(), 12);
    assert_eq!(rows[0], row());
    let again: String = rows.iter().map(TraceRow::to_line).collect();
    assert_eq!(again, golden);
}

#[test]
fn trace_rejects_bad_rows() {
    let good = row().to_line();
    let swap = |from: &str, to: &str| good.replacen(from, to, 1);
    let hash = committed_hash();
    let cases: Vec<(String, &str)> = vec![
        (
            swap("\"schema_version\":1", "\"schema_version\":2"),
            "version",
        ),
        (swap("\"attempt\":1", "\"attempt\":0"), "attempt 0"),
        (swap("\"attempt\":1", "\"attempt\":-1"), "attempt -1"),
        (swap("\"attempt\":1", "\"attempt\":1.5"), "attempt float"),
        (
            swap("\"cost_micro_usd\":1410", "\"cost_micro_usd\":\"1410\""),
            "cost string",
        ),
        (
            swap("\"tokens_in\":410", "\"tokens_in\":4.1e2"),
            "tokens float",
        ),
        (swap("\"pass\":true,", ""), "missing field"),
        (
            swap("\"pass\":true", "\"pass\":true,\"extra\":1"),
            "unknown field",
        ),
        (swap(&hash, &hash.to_uppercase()), "uppercase hash"),
        (swap(&hash, &hash[..63]), "short hash"),
        (swap("\"synthetic-replay-model\"", "\"\""), "empty model_id"),
        (swap("\"replay-001\"", "\"\""), "empty run_id"),
        (swap("\"replay-001\"", "\"run 1\""), "bad run_id"),
        (swap("\"t01-full\"", "\"T01\""), "bad task_id"),
    ];
    for (line, why) in cases {
        assert_ne!(line, good, "case {why} did not change the row");
        assert!(
            parse_traces(&line).is_err(),
            "{why} must be rejected: {line}"
        );
    }
    assert_eq!(
        parse_traces(&swap("\"schema_version\":1", "\"schema_version\":2"))
            .unwrap_err()
            .kind,
        TraceErrorKind::Version(2)
    );
    assert_eq!(
        parse_traces(&swap("\"attempt\":1", "\"attempt\":0"))
            .unwrap_err()
            .kind,
        TraceErrorKind::Attempt
    );
    assert_eq!(
        parse_traces(&swap(&hash, &hash[..63])).unwrap_err().kind,
        TraceErrorKind::BadHash
    );
}

#[test]
fn parse_traces_reports_line_numbers() {
    let good = row().to_line();
    let bad = good.replacen("\"attempt\":1", "\"attempt\":0", 1);
    let err = parse_traces(&format!("{good}{good}{bad}")).unwrap_err();
    assert_eq!(err.line, 3);
}

#[test]
fn blank_lines_rejected() {
    let good = row().to_line();
    assert_eq!(
        parse_traces(&format!("{good}\n{good}")).unwrap_err().kind,
        TraceErrorKind::BlankLine
    );
    assert_eq!(
        parse_traces(&good).unwrap().len(),
        1,
        "one final newline is fine"
    );
    assert_eq!(parse_traces("").unwrap().len(), 0);
}
