//! KernelWriter is the published path for ledger and pulse writes.
//! Fixtures are synthetic. Writer identity is configured via ENSEMBLY_WRITER.

use ensembly_kernel::application_ledger::{LedgerRow, Status};
use ensembly_kernel::pulse_supersede::{AdmitRequest, SupersedeFault, SupersedeRequest};
use ensembly_kernel::writer::{idempotency_key, KernelWriter, WriterError};
use rusqlite::Connection;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

const SCHEMA: &str = "
CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
CREATE TABLE memory_traces (
  id INTEGER PRIMARY KEY, nat_key TEXT NOT NULL UNIQUE, snippet TEXT NOT NULL, source TEXT NOT NULL,
  time TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('user-stated','tool-verified','inferred','unknown')),
  kind TEXT NOT NULL CHECK (kind IN ('data','context','fact','memory')),
  lock TEXT NOT NULL DEFAULT 'open' CHECK (lock IN ('open','locked')),
  verify_hash TEXT, prefetch_key TEXT, seen_count INTEGER NOT NULL DEFAULT 1, superseded_by TEXT,
  first_seen TEXT NOT NULL, last_seen TEXT NOT NULL);
CREATE TABLE conflicts (id INTEGER PRIMARY KEY, subject TEXT NOT NULL, left_id INTEGER NOT NULL,
  right_id INTEGER NOT NULL, resolution TEXT NOT NULL, ts TEXT NOT NULL);
CREATE VIRTUAL TABLE memory_fts USING fts5(nat_key, snippet, source, content='memory_traces', content_rowid='id');
INSERT INTO schema_migrations VALUES (1, '2026-01-01T00:00:00+00:00');
";

static WRITER_ENV: Mutex<()> = Mutex::new(());

fn with_writer(name: &str) -> MutexGuard<'static, ()> {
    let guard = WRITER_ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var(ensembly_schema::WRITER_ENV, name);
    guard
}

fn setup_writer() -> MutexGuard<'static, ()> {
    with_writer("laptop-1")
}

fn pulse_db() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pulse.sqlite");
    Connection::open(&path)
        .unwrap()
        .execute_batch(SCHEMA)
        .unwrap();
    (dir, path)
}

fn seed_trace(path: &Path, key: &str, snippet: &str) {
    let conn = Connection::open(path).unwrap();
    conn.execute(
        "INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, first_seen, last_seen)
         VALUES (?1, ?2, 'fixture', '2026-11-09T12:00:00+00:00', 'user-stated', 'fact',
                 '2026-11-09T12:00:00+00:00', '2026-11-09T12:00:00+00:00')",
        rusqlite::params![key, snippet],
    )
    .unwrap();
}

fn row(link: &str, status: Status, cv: &str) -> LedgerRow {
    LedgerRow {
        company: "Northwind Fictional AB".into(),
        role: "Engineer, Widgets".into(),
        link: link.into(),
        apply_url: None,
        status,
        submit_time: match status {
            Status::Prepared | Status::Withdrawn => None,
            _ => Some("2026-10-26T18:59:00+01:00".into()),
        },
        submit_time_unknown: None,
        cv_version: cv.into(),
    }
}

fn snippet(path: &Path, key: &str) -> String {
    let conn = Connection::open(path).unwrap();
    conn.query_row(
        "SELECT snippet FROM memory_traces WHERE nat_key = ?1",
        [key],
        |r| r.get(0),
    )
    .unwrap()
}

fn trace_count(path: &Path) -> i64 {
    let conn = Connection::open(path).unwrap();
    conn.query_row("SELECT COUNT(*) FROM memory_traces", [], |r| r.get(0))
        .unwrap()
}

fn direct_write_refused(err: &str) -> bool {
    err.contains("pulse writes must go through the kernel writer")
        || err.contains("kernel_writer_grant")
}

fn admit_req(key: &str, snippet: &str, writer: &str) -> AdmitRequest {
    AdmitRequest {
        key: key.into(),
        snippet: snippet.into(),
        source: "operator".into(),
        time: "2026-11-09T12:00:00+00:00".into(),
        status: "user-stated".into(),
        kind: "fact".into(),
        lock: "open".into(),
        supersedes: None,
        reason: None,
        writer: writer.into(),
    }
}

fn open_ledger(path: &Path) -> KernelWriter {
    let writer = KernelWriter::open_ledger(path).unwrap();
    writer.init_ledger().unwrap();
    writer
}

fn seal_path(ledger: &Path) -> std::path::PathBuf {
    let mut name = ledger.file_name().unwrap().to_os_string();
    name.push(".writer-hash");
    ledger.with_file_name(name)
}

#[test]
fn idempotency_key_is_link_key_tab_stage() {
    let prepared = row(
        "https://jobs.example.test/demo/1/",
        Status::Prepared,
        "cv-sample-1",
    );
    assert_eq!(
        idempotency_key(&prepared),
        "https://jobs.example.test/demo/1\tintended"
    );
    let interview = row(
        "https://jobs.example.test/demo/1",
        Status::Interview,
        "cv-sample-1",
    );
    assert_eq!(
        idempotency_key(&interview),
        "https://jobs.example.test/demo/1\tnext-stage:1"
    );
}

#[test]
fn ledger_append_is_idempotent_on_the_same_key_and_payload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    let first = row(
        "https://jobs.example.test/demo/1",
        Status::Prepared,
        "cv-sample-1",
    );
    let once = writer.append_ledger(&first).unwrap();
    assert!(once.appended);
    assert_eq!(once.key, idempotency_key(&first));
    let again = writer.append_ledger(&first).unwrap();
    assert!(!again.appended);
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 1);
}

#[test]
fn ledger_same_key_different_payload_is_a_conflict_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    let first = row(
        "https://jobs.example.test/demo/2",
        Status::Prepared,
        "cv-sample-1",
    );
    writer.append_ledger(&first).unwrap();
    let mut changed = first.clone();
    changed.cv_version = "cv-sample-2".into();
    let err = writer.append_ledger(&changed).unwrap_err();
    match err {
        WriterError::Conflict { key, .. } => {
            assert_eq!(key, idempotency_key(&first));
        }
        other => panic!("expected conflict, got {other}"),
    }
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 1);
}

#[test]
fn ledger_later_stage_appends_and_leaves_the_old_line_bytes_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    let prepared = row(
        "https://jobs.example.test/demo/3",
        Status::Prepared,
        "cv-sample-1",
    );
    writer.append_ledger(&prepared).unwrap();
    let before = fs::read(&path).unwrap();
    let submitted = row(
        "https://jobs.example.test/demo/3",
        Status::Submitted,
        "cv-sample-1",
    );
    assert!(writer.append_ledger(&submitted).unwrap().appended);
    let after = fs::read(&path).unwrap();
    assert!(after.starts_with(&before), "old ledger row was rewritten");
    assert!(after.len() > before.len());
    assert_eq!(std::str::from_utf8(&after).unwrap().lines().count(), 2);
}

#[test]
fn ledger_writer_refuses_to_append_after_an_old_line_is_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/4",
            Status::Prepared,
            "cv-sample-1",
        ))
        .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let original = fs::read_to_string(&path).unwrap();
    let tampered = original.replace("Northwind Fictional AB", "Contoso Fictional Ltd");
    fs::write(&path, tampered).unwrap();
    let err = writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/4",
            Status::Submitted,
            "cv-sample-1",
        ))
        .unwrap_err();
    assert!(err.to_string().contains("rewritten"), "{err}");
}

#[test]
fn deleted_seal_is_refused_and_is_not_adopted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/7",
            Status::Prepared,
            "cv-sample-1",
        ))
        .unwrap();
    let seal = seal_path(&path);
    fs::set_permissions(&seal, fs::Permissions::from_mode(0o644)).unwrap();
    fs::remove_file(&seal).unwrap();
    let err = writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/7",
            Status::Submitted,
            "cv-sample-1",
        ))
        .unwrap_err();
    assert!(err.to_string().contains("seal is missing"), "{err}");
    assert!(!seal.exists());
}

#[test]
fn tampering_then_reseal_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/8",
            Status::Prepared,
            "cv-sample-1",
        ))
        .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        original.replace("Northwind Fictional AB", "Contoso Fictional Ltd"),
    )
    .unwrap();
    let seal = seal_path(&path);
    fs::set_permissions(&seal, fs::Permissions::from_mode(0o644)).unwrap();
    fs::remove_file(&seal).unwrap();
    let err = writer.init_ledger().unwrap_err();
    assert!(
        err.to_string().contains("already has rows") || err.to_string().contains("will not adopt"),
        "{err}"
    );
    assert!(!seal.exists());
    let err = writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/8",
            Status::Submitted,
            "cv-sample-1",
        ))
        .unwrap_err();
    assert!(err.to_string().contains("seal is missing"), "{err}");
}

#[test]
fn init_refuses_a_non_empty_unsealed_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    fs::write(
        &path,
        r#"{"company":"Northwind Fictional AB","role":"Engineer, Widgets","link":"https://jobs.example.test/demo/9","status":"prepared","submit_time":null,"cv_version":"cv-sample-1"}
"#,
    )
    .unwrap();
    let writer = KernelWriter::open_ledger(&path).unwrap();
    let err = writer.init_ledger().unwrap_err();
    assert!(err.to_string().contains("already has rows"), "{err}");
}

#[test]
fn direct_ledger_write_is_refused_on_a_writer_managed_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/5",
            Status::Prepared,
            "cv-sample-1",
        ))
        .unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o444, "chmod is Unix-only; this test runs on Unix");
    let direct = fs::OpenOptions::new().write(true).open(&path);
    assert!(
        direct.is_err(),
        "a direct write must not open the locked ledger"
    );
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 1);
    assert!(
        writer
            .append_ledger(&row(
                "https://jobs.example.test/demo/5",
                Status::Submitted,
                "cv-sample-1",
            ))
            .unwrap()
            .appended
    );
}

#[test]
fn writer_has_no_ledger_delete_and_never_shortens_the_file() {
    fn assert_no_delete(_: &KernelWriter) {}
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    assert_no_delete(&writer);
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/6",
            Status::Prepared,
            "cv-sample-1",
        ))
        .unwrap();
    let len = fs::read(&path).unwrap().len();
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/6",
            Status::Submitted,
            "cv-sample-1",
        ))
        .unwrap();
    assert!(fs::read(&path).unwrap().len() > len);
}

#[test]
fn direct_pulse_insert_and_delete_are_refused_after_the_writer_opens_the_store() {
    let _w = setup_writer();
    let (_dir, path) = pulse_db();
    seed_trace(&path, "demo/old", "the earlier fact");
    let writer = KernelWriter::open_pulse(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    let insert = conn.execute(
        "INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, first_seen, last_seen)
         VALUES ('demo/sneak', 'bypass', 'fixture', '2026-11-09T12:00:00+00:00', 'user-stated', 'fact',
                 '2026-11-09T12:00:00+00:00', '2026-11-09T12:00:00+00:00')",
        [],
    );
    let insert_err = insert.unwrap_err().to_string();
    assert!(direct_write_refused(&insert_err), "{insert_err}");
    let delete = conn.execute("DELETE FROM memory_traces WHERE nat_key = 'demo/old'", []);
    let delete_err = delete.unwrap_err().to_string();
    assert!(direct_write_refused(&delete_err), "{delete_err}");
    assert_eq!(trace_count(&path), 1);
    assert_eq!(snippet(&path, "demo/old"), "the earlier fact");
    let _ = writer;
}

#[test]
fn pulse_admit_and_supersede_go_through_the_writer_and_keep_the_old_row() {
    let _w = setup_writer();
    let (_dir, path) = pulse_db();
    seed_trace(&path, "demo/old", "the earlier fact");
    let writer = KernelWriter::open_pulse(&path).unwrap();
    let admitted = writer
        .admit(
            &admit_req("demo/new", "the later fact", "laptop-1"),
            &SupersedeFault::default(),
        )
        .unwrap();
    assert_eq!(admitted.nat_key, "demo/new");
    let supersede = writer
        .supersede(
            &SupersedeRequest {
                old_key: "demo/old".into(),
                new_key: "demo/new".into(),
                source: "operator".into(),
                event_time: "2026-11-09T13:00:00+00:00".into(),
                reason: "preference changed".into(),
                writer: "laptop-1".into(),
                conflict_id: None,
            },
            &SupersedeFault::default(),
        )
        .unwrap();
    assert_eq!(supersede.old_key, "demo/old");
    assert_eq!(snippet(&path, "demo/old"), "the earlier fact");
    assert_eq!(snippet(&path, "demo/new"), "the later fact");
    assert_eq!(trace_count(&path), 2);
}

#[test]
fn pulse_writer_refuses_a_writer_that_does_not_match_config() {
    let _w = setup_writer();
    let (_dir, path) = pulse_db();
    let writer = KernelWriter::open_pulse(&path).unwrap();
    let err = writer
        .admit(
            &admit_req("demo/x", "a fact", "box"),
            &SupersedeFault::default(),
        )
        .unwrap_err();
    assert!(err.to_string().contains("configured identity"), "{err}");
    assert_eq!(trace_count(&path), 0);
}

#[test]
fn pulse_writer_refuses_an_unsafe_identity() {
    let _w = setup_writer();
    let (_dir, path) = pulse_db();
    let writer = KernelWriter::open_pulse(&path).unwrap();
    let err = writer
        .admit(
            &admit_req("demo/x", "a fact", "not/safe"),
            &SupersedeFault::default(),
        )
        .unwrap_err();
    assert!(err.to_string().contains("not a safe identity"), "{err}");
}

#[test]
fn configured_writer_is_accepted_and_another_name_is_not() {
    let (_dir, path) = pulse_db();
    {
        let _w = with_writer("mac-mini");
        let writer = KernelWriter::open_pulse(&path).unwrap();
        writer
            .admit(
                &admit_req("demo/a", "fact a", "mac-mini"),
                &SupersedeFault::default(),
            )
            .unwrap();
        let err = writer
            .admit(
                &admit_req("demo/b", "fact b", "laptop-2"),
                &SupersedeFault::default(),
            )
            .unwrap_err();
        assert!(err.to_string().contains("configured identity"), "{err}");
    }
    let _w = with_writer("laptop-2");
    let writer = KernelWriter::open_pulse(&path).unwrap();
    writer
        .admit(
            &admit_req("demo/b", "fact b", "laptop-2"),
            &SupersedeFault::default(),
        )
        .unwrap();
    assert_eq!(trace_count(&path), 2);
}
