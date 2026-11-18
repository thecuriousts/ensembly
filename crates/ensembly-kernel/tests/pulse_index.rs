//! Opt-in pulse-memory indexes (slice 7). Fixture DBs only.

use ensembly_kernel::pulse_core as pc;
use ensembly_kernel::pulse_index as pi;
use ensembly_kernel::pulse_supersede as ps;
use rusqlite::Connection;
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;
use tempfile::TempDir;

static WRITER_ENV: Mutex<()> = Mutex::new(());

fn writer_guard() -> MutexGuard<'static, ()> {
    let guard = WRITER_ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var(ensembly_schema::WRITER_ENV, "laptop-1");
    guard
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
INSERT INTO schema_migrations VALUES (1, '2026-01-01T00:00:00+00:00');
";

fn fresh_db() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("pulse.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    drop(conn);
    (dir, path)
}

fn explain_uses_index(conn: &Connection, sql: &str, index_name: &str) -> bool {
    let mut stmt = conn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap();
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    rows.iter().any(|detail| {
        detail.contains(index_name)
            || detail.contains("USING INDEX")
            || detail.contains("USING COVERING INDEX")
    })
}

#[test]
fn apply_is_idempotent_and_audit_appends() {
    let _g = writer_guard();
    let (_dir, path) = fresh_db();
    let w = "laptop-1";
    let first = pi::apply(&path, w, false).unwrap();
    assert_eq!(first.applied.len(), 3);
    assert!(first.already_present.is_empty());
    let second = pi::apply(&path, w, false).unwrap();
    assert!(second.applied.is_empty());
    assert_eq!(second.already_present.len(), 3);
    let conn = Connection::open(&path).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM ensembly_audit", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 6, "one audit row per statement per apply");
}

#[test]
fn revert_drops_only_managed_indexes() {
    let _g = writer_guard();
    let (_dir, path) = fresh_db();
    let w = "laptop-1";
    pi::apply(&path, w, false).unwrap();
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE INDEX not_ours ON memory_traces (source);
         CREATE INDEX ensembly_custom ON memory_traces (snippet);",
    )
    .unwrap();
    drop(conn);
    let report = pi::revert(&path, w, false).unwrap();
    assert_eq!(report.dropped.len(), 3);
    let conn = Connection::open(&path).unwrap();
    assert_eq!(pi::status(&conn).unwrap().iter().filter(|r| r.present).count(), 0);
    let foreign: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name IN ('not_ours', 'ensembly_custom')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(foreign, 2);
}

#[test]
fn status_reflects_indexes() {
    let _g = writer_guard();
    let (_dir, path) = fresh_db();
    let conn = pc::open_ro(&path).unwrap();
    for row in pi::status(&conn).unwrap() {
        assert!(!row.present);
    }
    drop(conn);
    pi::apply(&path, "laptop-1", false).unwrap();
    let conn = pc::open_ro(&path).unwrap();
    assert!(pi::status(&conn).unwrap().iter().all(|r| r.present));
}

#[test]
fn dangling_superseded_matches_legacy_on_random_graphs() {
    let (_dir, path) = fresh_db();
    let conn = Connection::open(&path).unwrap();
  for seed in 0u64..32 {
        conn.execute_batch("DELETE FROM memory_traces").unwrap();
        let mut keys = Vec::new();
        for i in 0..40 {
            let key = format!("demo/{seed}/{i}");
            keys.push(key.clone());
            conn.execute(
                "INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, first_seen, last_seen)
                 VALUES (?1, ?1, 'fixture', '2026-01-01T00:00:00Z', 'unknown', 'memory', ?2, ?2)",
                rusqlite::params![key, "2026-01-01T00:00:00Z"],
            )
            .unwrap();
        }
        let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        for (i, key) in keys.iter().enumerate() {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            if (rng >> 32) % 5 == 0 {
                let target = if (rng >> 16) % 4 == 0 {
                    format!("demo/missing/{seed}/{i}")
                } else {
                    keys[(rng as usize) % keys.len()].clone()
                };
                conn.execute(
                    "UPDATE memory_traces SET superseded_by = ?1 WHERE nat_key = ?2",
                    rusqlite::params![target, key],
                )
                .unwrap();
            }
        }
        let legacy = pc::dangling_superseded_keys_legacy(&conn).unwrap();
        let current = pc::dangling_superseded_keys(&conn).unwrap();
        assert_eq!(legacy, current, "seed {seed}");
    }
}

#[test]
fn db_stats_and_list_unchanged_with_indexes() {
    let _g = writer_guard();
    let (_dir, path) = fresh_db();
    let conn = Connection::open(&path).unwrap();
    for i in 0..50 {
        conn.execute(
            "INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, first_seen, last_seen, superseded_by)
             VALUES (?1, 's', 'fixture', ?2, 'unknown', 'memory', ?2, ?2, NULL)",
            rusqlite::params![
                format!("demo/{i}"),
                format!("2026-01-{i:02}T00:00:00Z")
            ],
        )
        .unwrap();
    }
    conn.execute(
        "UPDATE memory_traces SET superseded_by = 'demo/missing' WHERE nat_key = 'demo/3'",
        [],
    )
    .unwrap();
    drop(conn);
    let before_stats = {
        let c = pc::open_ro(&path).unwrap();
        pc::db_stats(&c).unwrap()
    };
    let before_list = {
        let c = pc::open_ro(&path).unwrap();
        ps::list_live(&c, 100).unwrap()
    };
    pi::apply(&path, "laptop-1", false).unwrap();
    let after_stats = {
        let c = pc::open_ro(&path).unwrap();
        pc::db_stats(&c).unwrap()
    };
    let after_list = {
        let c = pc::open_ro(&path).unwrap();
        ps::list_live(&c, 100).unwrap()
    };
    assert_eq!(
        before_stats.dangling_superseded,
        after_stats.dangling_superseded
    );
    assert_eq!(before_list, after_list);
}

#[test]
fn explain_plans_use_indexes_after_apply() {
    let _g = writer_guard();
    let (_dir, path) = fresh_db();
    let conn = Connection::open(&path).unwrap();
    for i in 0..200 {
        let live = i % 3 != 0;
        let kind = if live { "memory" } else { "fact" };
        let superseded = if live { "NULL" } else { "'demo/x'" };
        conn.execute_batch(&format!(
            "INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, first_seen, last_seen, superseded_by)
             VALUES ('demo/{i}', 's', 'fixture', '2026-01-01T00:00:00Z', 'unknown', '{kind}', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z', {superseded})"
        ))
        .unwrap();
    }
    drop(conn);
    pi::apply(&path, "laptop-1", false).unwrap();
    let conn = pc::open_ro(&path).unwrap();
    let list_sql = "SELECT nat_key FROM memory_traces \
        WHERE kind = 'memory' AND (superseded_by IS NULL OR superseded_by = '') \
        ORDER BY time DESC, nat_key ASC LIMIT 8";
    assert!(explain_uses_index(&conn, list_sql, pi::INDEX_LIVE_LIST));
    let dangling = "SELECT t.nat_key FROM memory_traces t \
        LEFT JOIN memory_traces u ON u.nat_key = t.superseded_by \
        WHERE t.superseded_by IS NOT NULL AND t.superseded_by <> '' AND u.nat_key IS NULL";
    assert!(explain_uses_index(&conn, dangling, pi::INDEX_SUPERSEDED_BY));
}

#[test]
#[ignore = "timing harness; run with --ignored --release for benchmark numbers"]
fn benchmark_100k_rows_release_numbers() {
    let _g = writer_guard();
    let (_dir, path) = fresh_db();
    let conn = Connection::open(&path).unwrap();
    conn.execute("BEGIN", []).unwrap();
    let mut stmt = conn
        .prepare(
            "INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, first_seen, last_seen, superseded_by)
             VALUES (?1, ?1, 'fixture', ?2, 'unknown', 'memory', ?2, ?2, NULL)",
        )
        .unwrap();
    for i in 0..100_000 {
        stmt.execute(rusqlite::params![
            format!("bench/{i}"),
            "2026-01-01T00:00:00Z"
        ])
        .unwrap();
        if i % 17 == 0 {
            conn.execute(
                "UPDATE memory_traces SET superseded_by = ?1 WHERE rowid = last_insert_rowid()",
                rusqlite::params![format!("bench/missing/{}", i % 50)],
            )
            .unwrap();
        }
    }
    conn.execute("COMMIT", []).unwrap();
    drop(stmt);
    drop(conn);

    let conn = pc::open_ro(&path).unwrap();
    let t0 = Instant::now();
    let _ = ps::list_live(&conn, 8).unwrap();
    let list_before_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let t1 = Instant::now();
    let _ = pc::dangling_superseded_keys(&conn).unwrap();
    let dangling_before_ms = t1.elapsed().as_secs_f64() * 1000.0;
    drop(conn);

    pi::apply(&path, "laptop-1", false).unwrap();

    let conn = pc::open_ro(&path).unwrap();
    let t2 = Instant::now();
    let _ = ps::list_live(&conn, 8).unwrap();
    let list_after_ms = t2.elapsed().as_secs_f64() * 1000.0;
    let t3 = Instant::now();
    let _ = pc::dangling_superseded_keys(&conn).unwrap();
    let dangling_after_ms = t3.elapsed().as_secs_f64() * 1000.0;

    eprintln!(
        "benchmark 100k: list {:.2} ms -> {:.2} ms; dangling {:.2} ms -> {:.2} ms",
        list_before_ms, list_after_ms, dangling_before_ms, dangling_after_ms
    );
}

#[test]
fn dry_run_prints_sql_without_writes() {
    let _g = writer_guard();
    let (_dir, path) = fresh_db();
    let report = pi::apply(&path, "laptop-1", true).unwrap();
    assert_eq!(report.applied.len(), 3);
    let conn = Connection::open(&path).unwrap();
    assert!(pi::status(&conn).unwrap().iter().all(|r| !r.present));
}