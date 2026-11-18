//! Golden encode/decode for v1, v2, and mixed logs.
//! v1 byte-identical round-trip is locked; v2/mixed goldens are provisional until Fleet
//! publishes the final envelope field list.

use ensembly_schema::{
    commit_json, decode_line, encode_line, parse_event_log, CommitFault, SCHEMA_VERSION,
    SCHEMA_VERSION_LEGACY,
};
use ensembly_schema::{EventEnvelope, JsonPayload, RecordEventPayload};
use std::path::PathBuf;

fn temp_log() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "record-golden-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("events.jsonl")
}

#[test]
fn v1_fixture_line_round_trips_byte_identical() {
    let text = std::fs::read_to_string(format!(
        "{}/fixtures/record-events-v1.jsonl",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap();
    let decoded = decode_line::<EventEnvelope<RecordEventPayload>>(line).unwrap();
    assert_eq!(decoded.version, SCHEMA_VERSION_LEGACY);
    let again = encode_line(SCHEMA_VERSION_LEGACY, &decoded.value).unwrap();
    assert_eq!(
        again, line,
        "v1 golden must round-trip without mutating bytes"
    );
}

#[test]
fn mixed_v1_then_v2_commits() {
    std::env::set_var("ENSEMBLY_WRITER", "laptop-1");
    let log = temp_log();
    let v1_unit = include_str!("../fixtures/scenarios/a-happy-events.json");
    commit_json(&log, v1_unit, &CommitFault::default()).unwrap();
    let v1_bytes = std::fs::read_to_string(&log).unwrap();
    assert!(v1_bytes.contains("\"schema_v\":2") || v1_bytes.contains("\"schema_v\":1"));
    let dup = commit_json(&log, v1_unit, &CommitFault::default()).unwrap();
    assert!(dup.duplicate);
    let text = std::fs::read_to_string(&log).unwrap();
    let events = parse_event_log::<JsonPayload>(&text).unwrap();
    assert!(events.len() >= 3);
    assert!(events.iter().any(|e| e.schema_v == SCHEMA_VERSION));
}
