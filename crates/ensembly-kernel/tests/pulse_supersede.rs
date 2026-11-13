//! Supersession on the pulse store. Fixture rows only. The old snippet stays
//! in the table; list hides it; history reads it back.

use ensembly_kernel::pulse_core as pc;
use ensembly_kernel::pulse_supersede as ps;
use ensembly_kernel::KernelWriter;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

static WRITER_ENV: Mutex<()> = Mutex::new(());

fn with_writer(name: &str) -> MutexGuard<'static, ()> {
    let guard = WRITER_ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var(ensembly_schema::WRITER_ENV, name);
    guard
}

fn setup_writer() -> MutexGuard<'static, ()> {
    with_writer("laptop-1")
}

fn kernel(path: &Path) -> KernelWriter {
    KernelWriter::open_pulse(path).unwrap()
}

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

struct Db {
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
}

fn db() -> Db {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pulse.sqlite");
    Connection::open(&path)
        .unwrap()
        .execute_batch(SCHEMA)
        .unwrap();
    Db { _dir: dir, path }
}

fn add(path: &Path, key: &str, snippet: &str, time: &str, kind: &str) {
    let c = Connection::open(path).unwrap();
    c.execute(
        "INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, first_seen, last_seen)
         VALUES (?1, ?2, 'fixture', ?3, 'user-stated', ?4, ?3, ?3)",
        rusqlite::params![key, snippet, time, kind],
    )
    .unwrap();
}

fn snippet(path: &Path, key: &str) -> String {
    let c = Connection::open(path).unwrap();
    c.query_row(
        "SELECT snippet FROM memory_traces WHERE nat_key = ?1",
        [key],
        |r| r.get(0),
    )
    .unwrap()
}

fn pointer(path: &Path, key: &str) -> Option<String> {
    let c = Connection::open(path).unwrap();
    c.query_row(
        "SELECT superseded_by FROM memory_traces WHERE nat_key = ?1",
        [key],
        |r| r.get(0),
    )
    .unwrap()
}

fn req(old: &str, new: &str) -> ps::SupersedeRequest {
    ps::SupersedeRequest {
        old_key: old.into(),
        new_key: new.into(),
        source: "operator".into(),
        event_time: "2026-11-09T12:00:00+00:00".into(),
        reason: "preference changed".into(),
        writer: "laptop-1".into(),
        conflict_id: None,
    }
}

fn supersede(path: &Path, old: &str, new: &str) -> ps::SupersedeReceipt {
    kernel(path)
        .supersede(&req(old, new), &ps::SupersedeFault::default())
        .unwrap()
}

#[test]
fn supersede_archives_old_row_and_hides_it_from_list() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/drink",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/drink-2",
        "coffee",
        "2026-11-09T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/fact",
        "a locked note",
        "2026-11-09T00:00:00+00:00",
        "fact",
    );
    let before = snippet(&db.path, "pref/drink");

    let receipt = supersede(&db.path, "pref/drink", "pref/drink-2");
    assert_eq!(receipt.phase, "verified");
    assert!(!receipt.idempotent);
    assert_eq!(snippet(&db.path, "pref/drink"), before);
    assert_eq!(
        pointer(&db.path, "pref/drink").as_deref(),
        Some("pref/drink-2")
    );

    let conn = pc::open_ro(&db.path).unwrap();
    let live = ps::list_live(&conn, 8).unwrap();
    assert_eq!(
        live.iter().map(|r| r.nat_key.as_str()).collect::<Vec<_>>(),
        vec!["pref/drink-2"]
    );
    let history = ps::read_history(&conn, "pref/drink").unwrap();
    assert_eq!(
        history
            .chain
            .iter()
            .map(|r| r.nat_key.as_str())
            .collect::<Vec<_>>(),
        vec!["pref/drink", "pref/drink-2"]
    );
    assert_eq!(history.events.len(), 1);
    assert_eq!(history.events[0].reason, "preference changed");
    assert_eq!(history.events[0].source, "operator");
    assert_eq!(history.events[0].writer, "laptop-1");
    assert_eq!(history.events[0].phase, "verified");

    let stats = pc::db_stats(&conn).unwrap();
    assert!(stats.dangling_superseded.is_empty());
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM memory_traces", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 3, "archive rows are not deleted");
}

#[test]
fn dangling_superseded_pointer_is_still_reported() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/old",
        "tea",
        "2026-11-09T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/new",
        "coffee",
        "2026-11-09T00:00:00+00:00",
        "memory",
    );
    // Plant the dangling pointer before the writer installs accident-guard
    // triggers, which would refuse a raw rusqlite UPDATE.
    let c = Connection::open(&db.path).unwrap();
    c.execute(
        "UPDATE memory_traces SET superseded_by = 'pref/missing' WHERE nat_key = 'pref/old'",
        [],
    )
    .unwrap();
    drop(c);
    let conn = pc::open_ro(&db.path).unwrap();
    let stats = pc::db_stats(&conn).unwrap();
    assert_eq!(stats.dangling_superseded, vec!["pref/old".to_string()]);
}

#[test]
fn supersede_is_idempotent_and_refuses_a_different_provenance() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/old",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/new",
        "coffee",
        "2026-11-09T00:00:00+00:00",
        "memory",
    );
    supersede(&db.path, "pref/old", "pref/new");
    let again = supersede(&db.path, "pref/old", "pref/new");
    assert!(again.idempotent);
    let conn = Connection::open(&db.path).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM supersession_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);

    let mut other = req("pref/old", "pref/new");
    other.reason = "a different reason".into();
    let err = kernel(&db.path)
        .supersede(&other, &ps::SupersedeFault::default())
        .unwrap_err();
    assert!(err.to_string().contains("provenance"), "{err}");
    assert_eq!(pointer(&db.path, "pref/old").as_deref(), Some("pref/new"));
}

#[test]
fn supersede_refuses_self_unknown_cycle_and_retarget() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/a",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/b",
        "coffee",
        "2026-11-02T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/c",
        "water",
        "2026-11-03T00:00:00+00:00",
        "memory",
    );

    let self_link =
        kernel(&db.path).supersede(&req("pref/a", "pref/a"), &ps::SupersedeFault::default());
    assert!(self_link
        .unwrap_err()
        .to_string()
        .contains("self-supersession"));
    assert!(pointer(&db.path, "pref/a").is_none());

    let missing = kernel(&db.path).supersede(
        &req("pref/missing", "pref/a"),
        &ps::SupersedeFault::default(),
    );
    assert!(missing.unwrap_err().to_string().contains("unknown key"));

    supersede(&db.path, "pref/a", "pref/b");
    let retarget =
        kernel(&db.path).supersede(&req("pref/a", "pref/c"), &ps::SupersedeFault::default());
    let retarget = retarget.unwrap_err().to_string();
    assert!(retarget.contains("already superseded"), "{retarget}");
    assert_eq!(pointer(&db.path, "pref/a").as_deref(), Some("pref/b"));

    supersede(&db.path, "pref/b", "pref/c");
    assert_eq!(pointer(&db.path, "pref/b").as_deref(), Some("pref/c"));
    let cycle =
        kernel(&db.path).supersede(&req("pref/c", "pref/a"), &ps::SupersedeFault::default());
    let cycle = cycle.unwrap_err().to_string();
    assert!(cycle.contains("cycle"), "{cycle}");
    assert!(pointer(&db.path, "pref/c").is_none());

    let conn = pc::open_ro(&db.path).unwrap();
    let chain = ps::read_history(&conn, "pref/a").unwrap();
    assert_eq!(
        chain
            .chain
            .iter()
            .map(|r| r.nat_key.as_str())
            .collect::<Vec<_>>(),
        vec!["pref/a", "pref/b", "pref/c"]
    );
}

#[test]
fn bad_writer_is_refused_before_any_pointer_change() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/old",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/new",
        "coffee",
        "2026-11-09T00:00:00+00:00",
        "memory",
    );
    let mut bad = req("pref/old", "pref/new");
    bad.writer = "office-box".into();
    let err = kernel(&db.path)
        .supersede(&bad, &ps::SupersedeFault::default())
        .unwrap_err();
    assert!(err.to_string().contains("configured identity"), "{err}");
    assert!(pointer(&db.path, "pref/old").is_none());
    let conn = Connection::open(&db.path).unwrap();
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'supersession_events'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 0, "a refused writer must not open the event table");
}

#[test]
fn intent_then_retry_reaches_verified() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/old",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/new",
        "coffee",
        "2026-11-09T00:00:00+00:00",
        "memory",
    );
    let stopped = kernel(&db.path)
        .supersede(
            &req("pref/old", "pref/new"),
            &ps::SupersedeFault {
                crash_entering: Some(ps::SupersedePhase::Applied),
            },
        )
        .unwrap();
    assert_eq!(stopped.phase, "intent");
    assert!(pointer(&db.path, "pref/old").is_none());

    let done = supersede(&db.path, "pref/old", "pref/new");
    assert_eq!(done.phase, "verified");
    assert!(!done.idempotent);
    assert_eq!(pointer(&db.path, "pref/old").as_deref(), Some("pref/new"));
}

#[test]
fn conflict_resolves_into_a_supersession() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/left",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/right",
        "coffee",
        "2026-11-09T00:00:00+00:00",
        "memory",
    );
    let c = Connection::open(&db.path).unwrap();
    let left_id: i64 = c
        .query_row(
            "SELECT id FROM memory_traces WHERE nat_key = 'pref/left'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let right_id: i64 = c
        .query_row(
            "SELECT id FROM memory_traces WHERE nat_key = 'pref/right'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    c.execute(
        "INSERT INTO conflicts (subject, left_id, right_id, resolution, ts) VALUES ('pref/left', ?1, ?2, 'not in the evidence', '2026-11-09T00:00:00+00:00')",
        rusqlite::params![left_id, right_id],
    )
    .unwrap();
    c.execute(
        "INSERT INTO conflicts (subject, left_id, right_id, resolution, ts) VALUES ('pref/left', ?1, ?2, 'open', '2026-11-09T00:00:00+00:00')",
        rusqlite::params![left_id, right_id],
    )
    .unwrap();
    drop(c);

    let open_id = 2;
    let receipt = kernel(&db.path)
        .resolve_conflict(
            &ps::ResolveRequest {
                conflict_id: open_id,
                winner: "pref/right".into(),
                source: "operator".into(),
                event_time: "2026-11-09T12:30:00+00:00".into(),
                reason: "the later preference stands".into(),
                writer: "laptop-1".into(),
            },
            &ps::SupersedeFault::default(),
        )
        .unwrap();
    assert_eq!(receipt.new_key, "pref/right");
    assert_eq!(
        pointer(&db.path, "pref/left").as_deref(),
        Some("pref/right")
    );
    assert_eq!(snippet(&db.path, "pref/left"), "tea");

    let conn = pc::open_ro(&db.path).unwrap();
    let resolution: String = conn
        .query_row("SELECT resolution FROM conflicts WHERE id = 2", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(resolution, "superseded by pref/right");
    let abstained: String = conn
        .query_row("SELECT resolution FROM conflicts WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        abstained, "not in the evidence",
        "a different row is left alone"
    );
    assert_eq!(pc::db_stats(&conn).unwrap().unresolved_conflicts, 0);
    assert!(pc::db_stats(&conn).unwrap().dangling_superseded.is_empty());

    let not_a_side = kernel(&db.path).resolve_conflict(
        &ps::ResolveRequest {
            conflict_id: 1,
            winner: "pref/missing".into(),
            source: "operator".into(),
            event_time: "2026-11-09T12:30:00+00:00".into(),
            reason: "no".into(),
            writer: "laptop-1".into(),
        },
        &ps::SupersedeFault::default(),
    );
    let not_a_side = not_a_side.unwrap_err().to_string();
    assert!(not_a_side.contains("winner"), "{not_a_side}");
    let still: String = Connection::open(&db.path)
        .unwrap()
        .query_row("SELECT resolution FROM conflicts WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(still, "not in the evidence");
}

#[test]
fn admit_supersedes_links_the_new_row_and_refuses_an_unlinked_fork() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/drink",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    let c = Connection::open(&db.path).unwrap();
    c.execute(
        "UPDATE memory_traces SET lock = 'locked' WHERE nat_key = 'pref/drink'",
        [],
    )
    .unwrap();
    drop(c);

    let fork = kernel(&db.path).admit(
        &ps::AdmitRequest {
            key: "pref/drink".into(),
            snippet: "coffee".into(),
            source: "operator".into(),
            time: "2026-11-09T12:00:00+00:00".into(),
            status: "user-stated".into(),
            kind: "memory".into(),
            lock: "locked".into(),
            supersedes: None,
            reason: None,
            writer: "laptop-1".into(),
        },
        &ps::SupersedeFault::default(),
    );
    let fork = fork.unwrap_err().to_string();
    assert!(fork.contains("locked snippet"), "{fork}");
    assert!(fork.contains("supersedes"), "{fork}");
    assert_eq!(snippet(&db.path, "pref/drink"), "tea");
    let names: Vec<String> = {
        let conn = Connection::open(&db.path).unwrap();
        let mut st = conn
            .prepare("SELECT nat_key FROM memory_traces ORDER BY nat_key")
            .unwrap();
        st.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    assert_eq!(names, vec!["pref/drink".to_string()]);

    let admitted = kernel(&db.path)
        .admit(
            &ps::AdmitRequest {
                key: "pref/drink-2".into(),
                snippet: "coffee".into(),
                source: "operator".into(),
                time: "2026-11-09T12:00:00+00:00".into(),
                status: "user-stated".into(),
                kind: "memory".into(),
                lock: "locked".into(),
                supersedes: Some("pref/drink".into()),
                reason: Some("preference changed".into()),
                writer: "laptop-1".into(),
            },
            &ps::SupersedeFault::default(),
        )
        .unwrap();
    assert_eq!(admitted.nat_key, "pref/drink-2");
    assert_eq!(
        pointer(&db.path, "pref/drink").as_deref(),
        Some("pref/drink-2")
    );
    let conn = pc::open_ro(&db.path).unwrap();
    let live = ps::list_live(&conn, 8).unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].nat_key, "pref/drink-2");
    assert_eq!(live[0].snippet, "coffee");
    let again = kernel(&db.path)
        .admit(
            &ps::AdmitRequest {
                key: "pref/drink-2".into(),
                snippet: "coffee".into(),
                source: "operator".into(),
                time: "2026-11-09T12:00:00+00:00".into(),
                status: "user-stated".into(),
                kind: "memory".into(),
                lock: "locked".into(),
                supersedes: Some("pref/drink".into()),
                reason: Some("preference changed".into()),
                writer: "laptop-1".into(),
            },
            &ps::SupersedeFault::default(),
        )
        .unwrap();
    assert!(again.idempotent);
    assert_eq!(
        Connection::open(&db.path)
            .unwrap()
            .query_row(
                "SELECT seen_count FROM memory_traces WHERE nat_key = 'pref/drink-2'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1,
        "an idempotent admit must not bump seen_count"
    );
}

#[test]
fn cli_supersede_exits_ok_and_refused() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/old",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/new",
        "coffee",
        "2026-11-09T00:00:00+00:00",
        "memory",
    );
    let ok = std::process::Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .env(ensembly_schema::WRITER_ENV, "mac-mini")
        .args([
            "pulse",
            "--pulse-db",
            db.path.to_str().unwrap(),
            "supersede",
            "pref/old",
            "pref/new",
            "--source",
            "operator",
            "--time",
            "2026-11-09T12:00:00+00:00",
            "--reason",
            "preference changed",
            "--writer",
            "mac-mini",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        ok.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let body: serde_json::Value = serde_json::from_slice(&ok.stdout).unwrap();
    assert_eq!(body["phase"], "verified");
    assert_eq!(body["old_key"], "pref/old");

    let refused = std::process::Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .env(ensembly_schema::WRITER_ENV, "mac-mini")
        .args([
            "pulse",
            "--pulse-db",
            db.path.to_str().unwrap(),
            "supersede",
            "pref/new",
            "pref/new",
            "--source",
            "operator",
            "--time",
            "2026-11-09T12:00:00+00:00",
            "--reason",
            "no",
            "--writer",
            "mac-mini",
        ])
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(pc::REFUSED_WRITE_EXIT));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("self-supersession"));

    let list = std::process::Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .args(["pulse", "--pulse-db", db.path.to_str().unwrap(), "list"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("pref/new"), "{stdout}");
    assert!(!stdout.contains("pref/old"), "{stdout}");

    let history = std::process::Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .args([
            "pulse",
            "--pulse-db",
            db.path.to_str().unwrap(),
            "history",
            "pref/old",
        ])
        .output()
        .unwrap();
    let hist = String::from_utf8_lossy(&history.stdout);
    assert!(hist.contains("pref/old"), "{hist}");
    assert!(hist.contains("tea"), "{hist}");
}

fn admit_req(key: &str, snippet: &str, supersedes: Option<&str>, writer: &str) -> ps::AdmitRequest {
    ps::AdmitRequest {
        key: key.into(),
        snippet: snippet.into(),
        source: "operator".into(),
        time: "2026-11-09T12:00:00+00:00".into(),
        status: "user-stated".into(),
        kind: "memory".into(),
        lock: "open".into(),
        supersedes: supersedes.map(str::to_string),
        reason: supersedes.map(|_| "preference changed".to_string()),
        writer: writer.into(),
    }
}

fn nat_keys(path: &Path) -> Vec<String> {
    let conn = Connection::open(path).unwrap();
    let mut st = conn
        .prepare("SELECT nat_key FROM memory_traces ORDER BY nat_key")
        .unwrap();
    st.query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn event_count(path: &Path) -> i64 {
    let conn = Connection::open(path).unwrap();
    let table: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'supersession_events'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    if table == 0 {
        return 0;
    }
    conn.query_row("SELECT COUNT(*) FROM supersession_events", [], |r| r.get(0))
        .unwrap()
}

fn assert_unchanged(path: &Path, keys: &[String], events: i64) {
    assert_eq!(
        nat_keys(path),
        keys,
        "a refused admit must not insert a row"
    );
    assert_eq!(
        event_count(path),
        events,
        "a refused admit must not write an event"
    );
}

#[test]
fn refused_admit_leaves_no_row_and_no_event() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/drink",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/drink-2",
        "coffee",
        "2026-11-09T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/c",
        "water",
        "2026-11-03T00:00:00+00:00",
        "fact",
    );
    let c = Connection::open(&db.path).unwrap();
    c.execute(
        "UPDATE memory_traces SET superseded_by = 'pref/drink' WHERE nat_key = 'pref/c'",
        [],
    )
    .unwrap();
    drop(c);
    let keys = nat_keys(&db.path);
    let events = event_count(&db.path);

    let self_link = kernel(&db.path)
        .admit(
            &admit_req("pref/drink-X", "x", Some("pref/drink-X"), "laptop-1"),
            &ps::SupersedeFault::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(self_link.contains("self-supersession"), "{self_link}");
    assert_unchanged(&db.path, &keys, events);

    let unknown = kernel(&db.path)
        .admit(
            &admit_req("pref/drink-X", "x", Some("pref/missing"), "laptop-1"),
            &ps::SupersedeFault::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(unknown.contains("unknown key"), "{unknown}");
    assert_unchanged(&db.path, &keys, events);

    let writer = kernel(&db.path)
        .admit(
            &admit_req("pref/drink-X", "x", Some("pref/drink"), "office-box"),
            &ps::SupersedeFault::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(writer.contains("configured identity"), "{writer}");
    assert_unchanged(&db.path, &keys, events);

    supersede(&db.path, "pref/drink", "pref/drink-2");
    let keys = nat_keys(&db.path);
    let events = event_count(&db.path);
    let already = kernel(&db.path)
        .admit(
            &admit_req("pref/drink-X", "x", Some("pref/drink"), "laptop-1"),
            &ps::SupersedeFault::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(already.contains("already superseded"), "{already}");
    assert_unchanged(&db.path, &keys, events);
    assert_eq!(
        pointer(&db.path, "pref/drink").as_deref(),
        Some("pref/drink-2")
    );

    let keys = nat_keys(&db.path);
    let events = event_count(&db.path);
    // c → drink → drink-2. Admitting c over the live head closes that loop.
    let cycle = kernel(&db.path)
        .admit(
            &admit_req("pref/c", "water", Some("pref/drink-2"), "laptop-1"),
            &ps::SupersedeFault::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(cycle.contains("cycle"), "{cycle}");
    assert_unchanged(&db.path, &keys, events);

    // Brand-new key. c → drink → drink-2, and drink-2 has no pointer.
    // Point the live head at the key we are about to admit, via a row that
    // already names it, so the link would loop once that row is the successor.
    // Here the refusal is the cycle on `pref/c` above. A second new key that
    // supersedes an already-superseded ancestor must also leave nothing.
    let looped = kernel(&db.path)
        .admit(
            &admit_req("pref/cycle-new", "juice", Some("pref/drink"), "laptop-1"),
            &ps::SupersedeFault::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(looped.contains("already superseded"), "{looped}");
    assert_unchanged(&db.path, &keys, events);
}

#[test]
fn open_row_snippet_change_does_not_say_locked() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/drink",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    let err = kernel(&db.path)
        .admit(
            &ps::AdmitRequest {
                snippet: "coffee".into(),
                supersedes: None,
                reason: None,
                ..admit_req("pref/drink", "tea", None, "laptop-1")
            },
            &ps::SupersedeFault::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("snippet changed"), "{err}");
    assert!(!err.contains("locked snippet"), "{err}");
    assert_eq!(nat_keys(&db.path), vec!["pref/drink".to_string()]);
}

#[test]
fn admit_link_refusal_rolls_back_the_new_row() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/drink",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    let conn = Connection::open(&db.path).unwrap();
    conn.execute_batch(
        "CREATE TABLE supersession_events (
            id INTEGER PRIMARY KEY,
            old_key TEXT NOT NULL,
            new_key TEXT NOT NULL,
            source TEXT NOT NULL,
            event_time TEXT NOT NULL,
            reason TEXT NOT NULL,
            writer TEXT NOT NULL,
            conflict_id INTEGER,
            phase TEXT NOT NULL CHECK (phase IN ('intent','applied','verified')),
            UNIQUE (old_key, new_key)
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO supersession_events (old_key, new_key, source, event_time, reason, writer, phase)
         VALUES ('pref/drink', 'pref/drink-X', 'operator', '2026-11-09T12:00:00+00:00', 'other reason', 'laptop-1', 'intent')",
        [],
    )
    .unwrap();
    drop(conn);
    let keys = nat_keys(&db.path);
    let err = kernel(&db.path)
        .admit(
            &admit_req("pref/drink-X", "x", Some("pref/drink"), "laptop-1"),
            &ps::SupersedeFault::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("provenance"), "{err}");
    assert_eq!(nat_keys(&db.path), keys);
    assert_eq!(event_count(&db.path), 1);
}

#[test]
fn concurrent_supersede_cannot_form_a_cycle() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/a",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/b",
        "coffee",
        "2026-11-02T00:00:00+00:00",
        "memory",
    );
    let left = db.path.clone();
    let right = db.path.clone();
    let h1 = std::thread::spawn(move || {
        kernel(&left).supersede(&req("pref/a", "pref/b"), &ps::SupersedeFault::default())
    });
    let h2 = std::thread::spawn(move || {
        kernel(&right).supersede(&req("pref/b", "pref/a"), &ps::SupersedeFault::default())
    });
    let r1 = h1.join().expect("thread");
    let r2 = h2.join().expect("thread");
    let ok = u8::from(r1.is_ok()) + u8::from(r2.is_ok());
    assert_eq!(ok, 1, "r1={r1:?} r2={r2:?}");
    let pa = pointer(&db.path, "pref/a");
    let pb = pointer(&db.path, "pref/b");
    assert!(
        pa.is_none() || pb.is_none(),
        "pointers formed a cycle: a={pa:?} b={pb:?}"
    );
    assert!(
        pa.is_some() || pb.is_some(),
        "neither link landed: a={pa:?} b={pb:?} r1={r1:?} r2={r2:?}"
    );
    let err = match (r1, r2) {
        (Err(err), _) | (_, Err(err)) => err.to_string(),
        (Ok(_), Ok(_)) => unreachable!("one writer must refuse"),
    };
    assert!(
        err.contains("cycle") || err.contains("already superseded"),
        "{err}"
    );
}

#[test]
fn cli_refused_admit_leaves_no_row() {
    let _w = setup_writer();
    let db = db();
    add(
        &db.path,
        "pref/drink",
        "tea",
        "2026-11-01T00:00:00+00:00",
        "memory",
    );
    add(
        &db.path,
        "pref/drink-2",
        "coffee",
        "2026-11-09T00:00:00+00:00",
        "memory",
    );
    supersede(&db.path, "pref/drink", "pref/drink-2");
    let refused = std::process::Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .env(ensembly_schema::WRITER_ENV, "laptop-1")
        .args([
            "pulse",
            "--pulse-db",
            db.path.to_str().unwrap(),
            "admit",
            "--key",
            "pref/drink-X",
            "--snippet",
            "x",
            "--supersedes",
            "pref/drink",
            "--source",
            "operator",
            "--time",
            "2026-11-09T12:00:00+00:00",
            "--status",
            "user-stated",
            "--kind",
            "memory",
            "--reason",
            "preference changed",
            "--writer",
            "laptop-1",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert_eq!(
        refused.status.code(),
        Some(pc::REFUSED_WRITE_EXIT),
        "{stderr}"
    );
    assert!(stderr.contains("already superseded"), "{stderr}");
    assert!(!nat_keys(&db.path).iter().any(|key| key == "pref/drink-X"));

    let help = std::process::Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .args(["pulse", "--help"])
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(!help.contains("opened read-only"), "{help}");
    assert!(help.contains("conflict resolve write"), "{help}");
}
