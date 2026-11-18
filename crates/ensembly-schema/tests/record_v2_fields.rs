//! Validation and round-trip for v2 optional bus fields.

use ensembly_schema::envelope_v2::{validate_ingress, validate_parsed};
use ensembly_schema::{
    decode_line, encode_line, validate_parser_id, validate_raw_sha256, validate_subjects,
    validate_supersedes, Checked, EventEnvelope, EventId, IngressInfo, IngressPath, ParsedInfo,
    ParsedState, RecordEventPayload, Supersedes, PARSED_SUMMARY_MAX, SCHEMA_VERSION,
    SCHEMA_VERSION_LEGACY, SUBJECTS_MAX, SUBJECT_MAX_CHARS, SUPERSEDES_MAX,
};

#[test]
fn v1_fixture_still_byte_identical() {
    let text = std::fs::read_to_string(format!(
        "{}/fixtures/record-events-v1.jsonl",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap();
    let decoded = decode_line::<EventEnvelope<RecordEventPayload>>(line).unwrap();
    let again = encode_line(SCHEMA_VERSION_LEGACY, &decoded.value).unwrap();
    assert_eq!(again, line);
}

#[test]
fn v2_minimal_round_trip() {
    let path = format!(
        "{}/fixtures/record-events-v2-minimal.jsonl",
        env!("CARGO_MANIFEST_DIR")
    );
    let line = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap()
        .to_string();
    let decoded = decode_line::<EventEnvelope<RecordEventPayload>>(&line).unwrap();
    assert_eq!(decoded.version, SCHEMA_VERSION);
    decoded.value.check().unwrap();
    let again = encode_line(SCHEMA_VERSION, &decoded.value).unwrap();
    let back = decode_line::<EventEnvelope<RecordEventPayload>>(&again).unwrap();
    assert_eq!(decoded.value, back.value);
}

#[test]
fn v2_full_round_trip_every_optional_field() {
    let path = format!(
        "{}/tests/fixtures/record-events-v2-full.jsonl",
        env!("CARGO_MANIFEST_DIR")
    );
    let line = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_string();
    let decoded = decode_line::<EventEnvelope<RecordEventPayload>>(&line).unwrap();
    decoded.value.check().unwrap();
    assert!(decoded.value.supersedes_ids().is_some());
    let again = encode_line(SCHEMA_VERSION, &decoded.value).unwrap();
    assert!(!again.contains("\"fleet_unknown\""));
    let back = decode_line::<EventEnvelope<RecordEventPayload>>(&again).unwrap();
    assert_eq!(decoded.value, back.value);
}

#[test]
fn mixed_v1_v2_log_reads() {
    let text = std::fs::read_to_string(format!(
        "{}/fixtures/record-events-mixed-v1-v2.jsonl",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let mut vers = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let d = decode_line::<EventEnvelope<RecordEventPayload>>(line).unwrap();
        d.value.check().unwrap();
        vers.push(d.value.schema_v);
    }
    assert_eq!(vers, vec![SCHEMA_VERSION_LEGACY, SCHEMA_VERSION]);
}

#[test]
fn deny_unknown_top_level_field() {
    let line = include_str!("../fixtures/record-events-v2-minimal.jsonl")
        .lines()
        .next()
        .unwrap()
        .replacen("\"v\":2", "\"fleet_unknown\":1,\"v\":2", 1);
    assert!(decode_line::<EventEnvelope<RecordEventPayload>>(&line).is_err());
}

#[test]
fn ingress_path_required_when_ingress_present() {
    let mut i = IngressInfo {
        via: None,
        provider: None,
        msg_id: None,
        sig: None,
        sig_ok: None,
        recv_ts_us: None,
        path: IngressPath::Push,
    };
    validate_ingress(&i).unwrap();
    i.path = IngressPath::Reconcile;
    validate_ingress(&i).unwrap();
}

#[test]
fn supersedes_accepts_string_or_array_on_wire() {
    let one = r#""01JCV1Q8Z00000000000000000""#;
    let many = r#"["01JCV1Q8Z00000000000000000","01JCV1Q8Z10000000000000000"]"#;
    let s1: Supersedes = serde_json::from_str(one).unwrap();
    let s2: Supersedes = serde_json::from_str(many).unwrap();
    validate_supersedes(&s1).unwrap();
    validate_supersedes(&s2).unwrap();
    assert_eq!(s1.0.len(), 1);
    assert_eq!(s2.0.len(), 2);
}

#[test]
fn supersedes_max_enforced() {
    let base: EventId =
        serde_json::from_value(serde_json::json!("01JCV1Q8Z00000000000000000")).unwrap();
    let ids: Vec<EventId> = (0..SUPERSEDES_MAX + 1).map(|_| base.clone()).collect();
    assert!(validate_supersedes(&Supersedes(ids)).is_err());
}

#[test]
fn subjects_bounds() {
    assert!(validate_subjects(&[]).is_err());
    let ok = vec!["alpha".into()];
    validate_subjects(&ok).unwrap();
    let long = "x".repeat(SUBJECT_MAX_CHARS + 1);
    assert!(validate_subjects(&[long]).is_err());
    let many = vec!["a".into(); SUBJECTS_MAX + 1];
    assert!(validate_subjects(&many).is_err());
}

#[test]
fn parsed_summary_and_parser_version() {
    assert!(validate_parser_id("bad").is_err());
    validate_parser_id("mail@1").unwrap();
    let p = ParsedInfo {
        state: ParsedState::Raw,
        parser: "mail@2".into(),
        r#type: None,
        summary: Some("x".repeat(PARSED_SUMMARY_MAX + 1)),
    };
    assert!(validate_parsed(&p).is_err());
}

#[test]
fn raw_sha256_lowercase_hex_only() {
    assert!(validate_raw_sha256("A".repeat(64).as_str()).is_err());
    assert!(validate_raw_sha256("g".repeat(64).as_str()).is_err());
    validate_raw_sha256(&"a".repeat(64)).unwrap();
}
