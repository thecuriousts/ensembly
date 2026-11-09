//! Supersession on the pulse store. Fixture rows only. The old snippet stays
//! in the table; list hides it; history reads it back.

use ensembly_kernel::pulse_core as pc;
use ensembly_kernel::pulse_supersede as ps;
use rusqlite::Connection;
use std::path::Path;

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
    ps::supersede(path, &req(old, new), &ps::SupersedeFault::default()).unwrap()
}

#[test]
fn supersede_archives_old_row_and_hides_it_from_list() {
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
    supersede(&db.path, "pref/old", "pref/new");
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
    let err = ps::supersede(&db.path, &other, &ps::SupersedeFault::default()).unwrap_err();
    assert!(err.to_string().contains("provenance"), "{err}");
    assert_eq!(pointer(&db.path, "pref/old").as_deref(), Some("pref/new"));
}

#[test]
fn supersede_refuses_self_unknown_cycle_and_retarget() {
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

    let self_link = ps::supersede(
        &db.path,
        &req("pref/a", "pref/a"),
        &ps::SupersedeFault::default(),
    );
    assert!(self_link
        .unwrap_err()
        .to_string()
        .contains("self-supersession"));
    assert!(pointer(&db.path, "pref/a").is_none());

    let missing = ps::supersede(
        &db.path,
        &req("pref/missing", "pref/a"),
        &ps::SupersedeFault::default(),
    );
    assert!(missing.unwrap_err().to_string().contains("unknown key"));

    supersede(&db.path, "pref/a", "pref/b");
    let retarget = ps::supersede(
        &db.path,
        &req("pref/a", "pref/c"),
        &ps::SupersedeFault::default(),
    );
    let retarget = retarget.unwrap_err().to_string();
    assert!(retarget.contains("already superseded"), "{retarget}");
    assert_eq!(pointer(&db.path, "pref/a").as_deref(), Some("pref/b"));

    supersede(&db.path, "pref/b", "pref/c");
    assert_eq!(pointer(&db.path, "pref/b").as_deref(), Some("pref/c"));
    let cycle = ps::supersede(
        &db.path,
        &req("pref/c", "pref/a"),
        &ps::SupersedeFault::default(),
    );
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
    let err = ps::supersede(&db.path, &bad, &ps::SupersedeFault::default()).unwrap_err();
    assert!(err.to_string().contains("public host alias"), "{err}");
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
    let stopped = ps::supersede(
        &db.path,
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
    let receipt = ps::resolve_conflict(
        &db.path,
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

    let not_a_side = ps::resolve_conflict(
        &db.path,
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

    let fork = ps::admit(
        &db.path,
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

    let admitted = ps::admit(
        &db.path,
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
            writer: "laptop-2".into(),
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
    let again = ps::admit(
        &db.path,
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
            writer: "laptop-2".into(),
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
            "box",
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
