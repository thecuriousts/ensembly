//! Ledger status maps onto record events. The ledger file is not this crate's writer.

use ensembly_schema::{
    compare_ledger, import_plan, map_ledger_status, map_ledger_status_name, AppEventPayload,
    EventEnvelope, LedgerRow, RecordStatus, Status,
};

const AT: &str = "2026-11-01T12:00:00+01:00";

fn row(status: Status, link: &str, time: Option<&str>, unknown: bool) -> LedgerRow {
    LedgerRow {
        company: "Birch Fictional".into(),
        role: "Ledger Clerk".into(),
        link: link.into(),
        apply_url: None,
        status,
        submit_time: time.map(str::to_string),
        submit_time_unknown: unknown.then_some(true),
        cv_version: "cv-sample-1".into(),
    }
}

fn status_json(status: Status, link: &str, time: Option<&str>, unknown: bool) -> serde_json::Value {
    let events = import_plan(&[], &[row(status, link, time, unknown)], "laptop-1", AT).unwrap();
    assert_eq!(events.len(), 1, "{status:?}");
    serde_json::to_value(&events[0].payload.status).unwrap()
}

#[test]
fn every_ledger_status_maps_and_confirmed_does_not() {
    let cases = [
        (Status::Prepared, "prepared", "intended", None),
        (Status::Submitted, "submitted", "submitted", None),
        (
            Status::Interview,
            "interview",
            "next-stage:1",
            Some("legacy-unknown"),
        ),
        (
            Status::Rejected,
            "rejected",
            "rejected",
            Some("legacy-unknown"),
        ),
        (Status::Withdrawn, "withdrawn", "withdrawn", None),
        (Status::SilentClosed, "silent-closed", "silent", None),
    ];
    for (status, name, token, evidence) in cases {
        let by_enum = map_ledger_status(status);
        let by_name = map_ledger_status_name(name).unwrap();
        assert_eq!(by_enum.token, token, "{name}");
        assert_eq!(by_name.token, token, "{name}");
        assert_eq!(by_enum.evidence_source.as_deref(), evidence, "{name}");
        assert_eq!(by_name.evidence_source.as_deref(), evidence, "{name}");
        assert_ne!(token, "confirmed");
    }
    let err = map_ledger_status_name("confirmed").unwrap_err();
    assert!(err.contains("unknown"), "{err}");
    assert!(err.contains("confirmed"), "{err}");
}

#[test]
fn unknown_status_names_the_value_and_the_known_set() {
    let err = map_ledger_status_name("offered").unwrap_err();
    assert!(err.contains("unknown"), "{err}");
    assert!(err.contains("offered"), "{err}");
    for name in [
        "prepared",
        "submitted",
        "interview",
        "rejected",
        "withdrawn",
        "silent-closed",
    ] {
        assert!(err.contains(name), "{err}");
    }
}

#[test]
fn mapped_events_carry_the_stage_and_legacy_unknown_evidence() {
    let link = "https://jobs.example.test/birch/2";
    let submitted = status_json(
        Status::Submitted,
        link,
        Some("2026-10-26T18:59:00+01:00"),
        false,
    );
    assert_eq!(submitted["status"], "submitted");
    assert_eq!(submitted["submit"]["kind"], "known");
    assert_eq!(submitted["submit"]["at"], "2026-10-26T18:59:00+01:00");

    let interview = status_json(
        Status::Interview,
        "https://jobs.example.test/cedar/3",
        Some("2026-10-27T09:00:00+01:00"),
        false,
    );
    assert_eq!(interview["status"], "next-stage");
    assert_eq!(interview["n"], 1);
    assert_eq!(interview["evidence"]["source"], "legacy-unknown");

    let rejected = status_json(
        Status::Rejected,
        "https://jobs.example.test/drift/4",
        None,
        true,
    );
    assert_eq!(rejected["status"], "rejected");
    assert_eq!(rejected["evidence"]["source"], "legacy-unknown");
    assert_eq!(rejected["evidence"]["at"], AT);

    let silent = status_json(
        Status::SilentClosed,
        "https://jobs.example.test/fern/6",
        Some("2026-10-28T15:00:00+01:00"),
        false,
    );
    assert_eq!(silent["since"], "2026-10-28T15:00:00+01:00");

    let silent_unknown = status_json(
        Status::SilentClosed,
        "https://jobs.example.test/fern/7",
        None,
        true,
    );
    assert_eq!(silent_unknown["since"], AT);

    let intended = status_json(
        Status::Prepared,
        "https://jobs.example.test/amber/1",
        None,
        false,
    );
    assert_eq!(intended["status"], "intended");
    let withdrawn = status_json(
        Status::Withdrawn,
        "https://jobs.example.test/elm/5",
        None,
        false,
    );
    assert_eq!(withdrawn["status"], "withdrawn");
}

#[test]
fn second_plan_is_empty_and_a_different_payload_conflicts() {
    let link = "https://jobs.example.test/birch/2/application";
    let canonical = "https://jobs.example.test/birch/2";
    let older = row(Status::Prepared, link, None, false);
    let current = row(
        Status::Submitted,
        canonical,
        Some("2026-10-26T18:59:00+01:00"),
        false,
    );
    let first = import_plan(&[], &[older.clone(), current.clone()], "laptop-1", AT).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].aggregate.0, canonical);
    assert_eq!(first[0].key.0, format!("{canonical}\tsubmitted"));
    assert_eq!(first[0].payload.writer, "laptop-1");
    let second = import_plan(&first, &[older, current.clone()], "laptop-1", AT).unwrap();
    assert!(second.is_empty());

    let mut other = current;
    other.company = "Other Fictional".into();
    let err = import_plan(&first, &[other], "laptop-1", AT).unwrap_err();
    assert!(err.contains("conflict"), "{err}");
}

#[test]
fn writer_uses_the_shared_public_host_aliases() {
    let rows = [row(
        Status::Prepared,
        "https://jobs.example.test/amber/1",
        None,
        false,
    )];
    let err = import_plan(&[], &rows, "not-a-host", AT).unwrap_err();
    assert!(err.contains("public host alias"), "{err}");
    let events = import_plan(&[], &rows, "laptop-2", AT).unwrap();
    assert_eq!(events[0].payload.writer, "laptop-2");
    assert_eq!(events[0].source, ensembly_schema::Source::Document);
}

#[test]
fn compare_lists_matches_and_an_injected_mismatch() {
    let link = "https://jobs.example.test/birch/2";
    let rows = [row(
        Status::Submitted,
        link,
        Some("2026-10-26T18:59:00+01:00"),
        false,
    )];
    let events = import_plan(&[], &rows, "laptop-1", AT).unwrap();
    let clean = compare_ledger(&rows, &events).unwrap();
    assert_eq!(clean.matches, vec![link.to_string()]);
    assert!(clean.mismatches.is_empty());

    let mut bad = events;
    bad[0].payload.status = RecordStatus::Withdrawn;
    let report = compare_ledger(&rows, &bad).unwrap();
    assert!(report.matches.is_empty());
    assert_eq!(report.mismatches.len(), 1);
    assert_eq!(report.mismatches[0].link_key, link);
    assert_eq!(report.mismatches[0].ledger, "submitted");
    assert_eq!(report.mismatches[0].record, "withdrawn");
}

#[test]
fn reimport_ignores_writer_and_a_new_clock() {
    let rows = [
        row(
            Status::Rejected,
            "https://jobs.example.test/drift/4",
            None,
            true,
        ),
        row(
            Status::SilentClosed,
            "https://jobs.example.test/fern/7",
            None,
            true,
        ),
    ];
    let first = import_plan(&[], &rows, "laptop-1", AT).unwrap();
    assert_eq!(first.len(), 2);
    let second = import_plan(&first, &rows, "laptop-1", "2026-11-02T09:30:00+01:00").unwrap();
    assert!(second.is_empty(), "{second:?}");
    let other = import_plan(&first, &rows, "mac-mini", "2026-12-01T08:00:00+01:00").unwrap();
    assert!(other.is_empty(), "{other:?}");
}

#[test]
fn return_to_an_earlier_stage_names_the_link() {
    let link = "https://jobs.example.test/birch/2";
    let submitted = row(
        Status::Submitted,
        link,
        Some("2026-10-26T18:59:00+01:00"),
        false,
    );
    let interview = row(
        Status::Interview,
        link,
        Some("2026-10-26T18:59:00+01:00"),
        false,
    );
    let first = import_plan(&[], std::slice::from_ref(&submitted), "laptop-1", AT).unwrap();
    let second = import_plan(&first, &[interview], "laptop-1", AT).unwrap();
    let mut log = first;
    log.extend(second);
    let err = import_plan(&log, &[submitted], "laptop-1", AT).unwrap_err();
    assert!(err.contains(link), "{err}");
    assert!(err.contains("submitted"), "{err}");
    assert!(err.contains("next-stage:1"), "{err}");
}

#[test]
fn compare_rejects_a_different_submit_time() {
    let link = "https://jobs.example.test/birch/2";
    let known = "2026-10-26T18:59:00+01:00";
    let rows = [row(Status::Submitted, link, Some(known), false)];
    let events = import_plan(&[], &rows, "laptop-1", AT).unwrap();
    let bad = with_json_at(
        &events,
        "/payload/status/submit/at",
        "2026-11-02T08:00:00+01:00",
    );
    let report = compare_ledger(&rows, &bad).unwrap();
    assert_eq!(report.mismatches.len(), 1, "{report:?}");
    assert_eq!(report.mismatches[0].link_key, link);
    let text = format!("{:?}", report.mismatches[0]);
    assert!(text.contains(known), "{text}");
    assert!(text.contains("2026-11-02T08:00:00+01:00"), "{text}");
}

#[test]
fn compare_rejects_a_different_silent_since() {
    let link = "https://jobs.example.test/fern/6";
    let known = "2026-10-28T15:00:00+01:00";
    let rows = [row(Status::SilentClosed, link, Some(known), false)];
    let events = import_plan(&[], &rows, "laptop-1", AT).unwrap();
    let bad = with_json_at(
        &events,
        "/payload/status/since",
        "2026-01-01T00:00:00+01:00",
    );
    let report = compare_ledger(&rows, &bad).unwrap();
    assert_eq!(report.mismatches.len(), 1, "{report:?}");
    let text = format!("{:?}", report.mismatches[0]);
    assert!(text.contains(known), "{text}");
    assert!(text.contains("2026-01-01T00:00:00+01:00"), "{text}");
}

#[test]
fn submit_time_correction_says_the_log_is_not_rewritten() {
    let link = "https://jobs.example.test/birch/2";
    let first_row = row(
        Status::Submitted,
        link,
        Some("2026-10-26T18:59:00+01:00"),
        false,
    );
    let first = import_plan(&[], &[first_row], "laptop-1", AT).unwrap();
    let corrected = row(
        Status::Submitted,
        link,
        Some("2026-10-27T09:00:00+01:00"),
        false,
    );
    let err = import_plan(&first, &[corrected], "laptop-1", AT).unwrap_err();
    assert!(err.contains("submit time"), "{err}");
    assert!(err.contains("does not rewrite"), "{err}");
    assert!(err.contains("stage"), "{err}");
}

#[test]
fn unknown_submit_time_is_marked_synthetic() {
    let unknown = import_plan(
        &[],
        &[row(
            Status::Rejected,
            "https://jobs.example.test/drift/4",
            None,
            true,
        )],
        "laptop-1",
        AT,
    )
    .unwrap();
    let value = serde_json::to_value(&unknown[0].payload).unwrap();
    assert_eq!(value["synthetic_time"], true, "{value}");
    assert_eq!(unknown[0].event_time.as_str(), AT);

    let known = import_plan(
        &[],
        &[row(
            Status::Submitted,
            "https://jobs.example.test/birch/2",
            Some("2026-10-26T18:59:00+01:00"),
            false,
        )],
        "laptop-1",
        AT,
    )
    .unwrap();
    let value = serde_json::to_value(&known[0].payload).unwrap();
    assert_eq!(value["synthetic_time"], false, "{value}");
    assert_eq!(known[0].event_time.as_str(), "2026-10-26T18:59:00+01:00");
}

#[test]
fn open_status_is_refused_instead_of_intended() {
    let err = ensembly_schema::closed_stage(Status::Prepared).unwrap_err();
    assert!(err.contains("open ledger status"), "{err}");
    assert!(err.contains("intended"), "{err}");
    let withdrawn = row(
        Status::Withdrawn,
        "https://jobs.example.test/elm/5",
        None,
        false,
    );
    let err = ensembly_schema::closed_record_status(&withdrawn, AT).unwrap_err();
    assert!(err.contains("open ledger status"), "{err}");
    assert!(err.contains("intended"), "{err}");
}

fn with_json_at(
    events: &[EventEnvelope<AppEventPayload>],
    pointer: &str,
    time: &str,
) -> Vec<EventEnvelope<AppEventPayload>> {
    let mut value = serde_json::to_value(events).unwrap();
    let mut cursor = value.get_mut(0).unwrap();
    for key in pointer.split('/').filter(|part| !part.is_empty()) {
        cursor = cursor.get_mut(key).unwrap();
    }
    *cursor = serde_json::json!(time);
    serde_json::from_value(value).unwrap()
}
