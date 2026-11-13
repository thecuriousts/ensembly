//! E3 decision model: synthetic fixtures only. No operator labels, no accuracy claim.

use ensembly_kernel::decision::{
    audit, load_events, load_labels, run_eval, score, silence_took_action, FlagList, GATE_REASON,
};
use ensembly_kernel::{decision, KernelWriter};
use ensembly_schema::{assign_split, FitLabel, SplitBucket, MODEL_VERSION, WRITER_ENV};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static WRITER_ENV_LOCK: Mutex<()> = Mutex::new(());

fn setup_writer() -> MutexGuard<'static, ()> {
    let guard = WRITER_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var(WRITER_ENV, "laptop-1");
    guard
}

fn fixture_labels() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/decision/synthetic/labels.jsonl")
}

fn open_log(dir: &Path) -> KernelWriter {
    let path = dir.join("decisions.jsonl");
    let w = KernelWriter::open_decisions(&path).unwrap();
    w.init_decisions().unwrap();
    w
}

#[test]
fn synthetic_fixtures_are_balanced_and_labelled() {
    let rows = load_labels(&fixture_labels()).unwrap();
    assert_eq!(rows.len(), 60);
    assert!(rows.iter().all(|r| r.provenance == "synthetic-fixture"));
    assert!(rows.iter().all(|r| r.decision.is_some()));
    assert_eq!(
        rows.iter()
            .filter(|r| r.split() == SplitBucket::Train)
            .count(),
        36
    );
    assert_eq!(
        rows.iter()
            .filter(|r| r.split() == SplitBucket::Val)
            .count(),
        12
    );
    assert_eq!(
        rows.iter()
            .filter(|r| r.split() == SplitBucket::HeldOut)
            .count(),
        12
    );
    assert!(rows.iter().any(|r| r.decision == Some(FitLabel::Apply)));
    assert!(rows.iter().any(|r| r.decision == Some(FitLabel::Skip)));
}

#[test]
fn held_out_metrics_are_omitted_when_labels_are_missing() {
    let mut rows = load_labels(&fixture_labels()).unwrap();
    for row in rows
        .iter_mut()
        .filter(|r| r.split() == SplitBucket::HeldOut)
    {
        row.decision = None;
        row.reason_tag = None;
    }
    let report = run_eval(&rows, &FlagList::default(), 0.90).unwrap();
    let held = report
        .splits
        .iter()
        .find(|s| s.bucket == "held-out")
        .unwrap();
    assert!(!held.complete);
    assert!(held.model.is_none());
    assert!(held.note.as_deref().unwrap().contains("not reported"));
    assert!(report.claim.contains("No accuracy"));
}

#[test]
fn flipping_held_out_labels_does_not_change_the_band() {
    let rows = load_labels(&fixture_labels()).unwrap();
    let a = run_eval(&rows, &FlagList::default(), 0.90).unwrap();
    let mut flipped = rows.clone();
    for row in flipped
        .iter_mut()
        .filter(|r| r.split() == SplitBucket::HeldOut)
    {
        row.decision = Some(match row.decision {
            Some(FitLabel::Apply) => FitLabel::Skip,
            _ => FitLabel::Apply,
        });
    }
    let b = run_eval(&flipped, &FlagList::default(), 0.90).unwrap();
    assert_eq!(a.band.t_apply, b.band.t_apply);
    assert_eq!(a.band.t_skip, b.band.t_skip);
    assert_eq!(a.trained_on, b.trained_on);
}

#[test]
fn gate_catches_every_synthetic_scam_cue() {
    let rows = load_labels(&fixture_labels()).unwrap();
    let flags = FlagList::default();
    let flagged: Vec<_> = rows
        .iter()
        .filter(|r| r.reason_tag.as_deref() == Some("scam-gate"))
        .collect();
    assert!(!flagged.is_empty());
    for row in flagged {
        let g = audit(&row.url, &row.posting_text, &flags);
        assert_eq!(g.verdict, ensembly_schema::GateVerdict::Fail, "{}", row.id);
        assert_eq!(g.reason.as_deref(), Some(GATE_REASON));
    }
}

#[test]
fn expire_writes_no_apply_and_late_confirm_is_refused() {
    let _w = setup_writer();
    let dir = tempfile::tempdir().unwrap();
    let writer = open_log(dir.path());
    let rows = load_labels(&fixture_labels()).unwrap();
    let row = rows
        .iter()
        .find(|r| r.decision == Some(FitLabel::Apply))
        .unwrap();
    let flags = FlagList::default();
    // A dummy score is enough to propose; fit is not required for the wait path.
    let model = decision::fit(
        &rows
            .iter()
            .filter(|r| r.split() == SplitBucket::Train && r.decision.is_some())
            .collect::<Vec<_>>(),
        &flags,
        0.90,
    )
    .unwrap();
    let scored = score(&model, row, &flags);
    let proposed = writer
        .propose_decision(
            row,
            &scored,
            "laptop-1",
            "2026-11-13T12:00:00+01:00",
            Some("2026-11-16T12:00:00+01:00"),
        )
        .unwrap();
    assert!(proposed.appended);
    let rid = proposed.rid;
    let expired = writer
        .expire_decision(&rid, "laptop-1", "2026-11-16T13:00:00+01:00")
        .unwrap();
    assert!(expired.appended);
    assert_eq!(
        expired.event_type,
        ensembly_schema::DecisionEventType::Expired
    );
    let log = load_events(&dir.path().join("decisions.jsonl")).unwrap();
    assert!(!silence_took_action(&log));
    let late = writer
        .confirm_decision(&rid, "laptop-1", "2026-11-16T14:00:00+01:00")
        .unwrap_err()
        .to_string();
    assert!(
        late.contains("late signal") || late.contains("new proposal"),
        "{late}"
    );
}

#[test]
fn confirm_is_idempotent_and_a_second_payload_conflicts() {
    let _w = setup_writer();
    let dir = tempfile::tempdir().unwrap();
    let writer = open_log(dir.path());
    let rows = load_labels(&fixture_labels()).unwrap();
    let row = rows
        .iter()
        .find(|r| r.reason_tag.as_deref() == Some("fit"))
        .unwrap();
    let flags = FlagList::default();
    let model = decision::fit(
        &rows
            .iter()
            .filter(|r| r.split() == SplitBucket::Train && r.decision.is_some())
            .collect::<Vec<_>>(),
        &flags,
        0.90,
    )
    .unwrap();
    let scored = score(&model, row, &flags);
    writer
        .propose_decision(row, &scored, "laptop-1", "2026-11-13T12:00:00+01:00", None)
        .unwrap();
    let rid = ensembly_schema::proposal_rid(&row.link_key, MODEL_VERSION, &row.snapshot_hash());
    let first = writer
        .confirm_decision(&rid, "laptop-1", "2026-11-13T13:00:00+01:00")
        .unwrap();
    assert!(first.appended);
    let again = writer
        .confirm_decision(&rid, "laptop-1", "2026-11-13T13:00:00+01:00")
        .unwrap();
    assert!(!again.appended);
    // a different writer on the same step is a conflict
    std::env::set_var(WRITER_ENV, "laptop-2");
    let conflict = writer
        .confirm_decision(&rid, "laptop-2", "2026-11-13T13:00:00+01:00")
        .unwrap_err()
        .to_string();
    assert!(
        conflict.contains("idempotency") || conflict.contains("already"),
        "{conflict}"
    );
}

#[test]
fn split_helper_matches_fixture_urls() {
    assert_eq!(
        assign_split("https://jobs.example.test/posting/1"),
        SplitBucket::Train
    );
    assert_eq!(
        assign_split("https://jobs.example.test/posting/6"),
        SplitBucket::Val
    );
    assert_eq!(
        assign_split("https://jobs.example.test/posting/10"),
        SplitBucket::HeldOut
    );
}
