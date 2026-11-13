//! KernelWriter pulse-store single-writer checks.
//! Fixtures are synthetic. Writer identity is configured via ENSEMBLY_WRITER.

use ensembly_kernel::pulse_supersede::{AdmitRequest, SupersedeFault, SupersedeRequest};
use ensembly_kernel::writer::KernelWriter;
use rusqlite::Connection;
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
