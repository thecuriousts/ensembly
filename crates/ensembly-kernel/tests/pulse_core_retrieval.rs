//! Pulse read-path gates: index use, scaling, recall, and fresh-first ranking.
//! Uses fixture DBs only; never touches a live pulse-memory store.

use ensembly_kernel::pulse_core as pc;
use rusqlite::Connection;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::TempDir;

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
CREATE VIRTUAL TABLE memory_fts USING fts5(nat_key, snippet, source, content='memory_traces', content_rowid='id');
INSERT INTO schema_migrations VALUES (1, '2026-01-01T00:00:00+00:00');
";

/// Slice 1a measure-only baseline (fixture). After #93 merges to master, align with
/// `retrieval-bench` `results.json` (not-below gate).
const BASELINE_RECALL_AT_5: f64 = 1.0;
const BASELINE_RECALL_AT_10: f64 = 1.0;
const BASELINE_FRESH_FIRST: f64 = 1.0;
const PREFIX_SCALE_MAX_RATIO: f64 = 5.0;
const PREFIX_SCALE_SMALL_ROWS: i64 = 800;
const PREFIX_SCALE_LARGE_ROWS: i64 = 4_000;

fn open_fixture_db(path: &PathBuf) -> Connection {
    let c = Connection::open(path).unwrap();
    c.execute_batch(SCHEMA).unwrap();
    c
}

fn insert_trace(c: &Connection, key: &str, snippet: &str, last_seen: &str) {
    c.execute(
        "INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, first_seen, last_seen)
         VALUES (?1, ?2, 'fixture', ?3, 'tool-verified', 'memory', ?3, ?3)",
        rusqlite::params![key, snippet, last_seen],
    )
    .unwrap();
    let id = c.last_insert_rowid();
    c.execute(
        "INSERT INTO memory_fts(rowid, nat_key, snippet, source) VALUES (?1, ?2, ?3, 'fixture')",
        rusqlite::params![id, key, snippet],
    )
    .unwrap();
}

fn fill_synthetic(c: &Connection, n: i64, key_prefix: &str) {
    for i in 0..n {
        let key = format!("{key_prefix}{i:06}");
        let snippet = format!("synthetic row {i} token_zeta");
        let ts = format!("2026-01-{:02}T12:00:00+00:00", (i % 28) + 1);
        insert_trace(c, &key, &snippet, &ts);
    }
}

fn prefix_explain_scans_table(conn: &Connection, prefix: &str) -> bool {
    let upper = pc::nat_key_prefix_upper_bound(prefix);
    let plan_sql = match upper {
        Some(ub) => format!(
            "EXPLAIN QUERY PLAN SELECT nat_key FROM memory_traces \
             WHERE nat_key >= '{prefix}' AND nat_key < '{ub}' ORDER BY nat_key LIMIT 10"
        ),
        None => format!(
            "EXPLAIN QUERY PLAN SELECT nat_key FROM memory_traces \
             WHERE nat_key >= '{prefix}' ORDER BY nat_key LIMIT 10"
        ),
    };
    let mut st = conn.prepare(&plan_sql).unwrap();
    let lines: Vec<String> = st
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    lines
        .iter()
        .any(|l| l.contains("SCAN memory_traces"))
}

fn timed_prefix(conn: &Connection, prefix: &str, limit: usize) -> Duration {
    let start = Instant::now();
    let rows = pc::read_prefix(conn, prefix, limit).unwrap();
    assert!(!rows.is_empty(), "prefix read returned nothing");
    start.elapsed()
}

#[test]
fn nat_key_prefix_upper_bound_cases() {
    assert_eq!(pc::nat_key_prefix_upper_bound(""), None);
    assert_eq!(pc::nat_key_prefix_upper_bound("demo/"), Some("demo0".into()));
    assert_eq!(pc::nat_key_prefix_upper_bound("demo/a"), Some("demo/b".into()));
    assert_eq!(
        pc::nat_key_prefix_upper_bound("demo/\u{ff0e}\u{ff0e}"),
        Some("demo/\u{ff0e}\u{ff0f}".into())
    );
}

#[test]
fn prefix_read_unicode_and_percent_literal() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("pulse.sqlite");
    let c = open_fixture_db(&path);
    insert_trace(
        &c,
        "demo/\u{ff3a}",
        "fullwidth z",
        "2026-10-01T00:00:00+00:00",
    );
    insert_trace(&c, "demo/alpha", "ascii", "2026-10-02T00:00:00+00:00");
    insert_trace(&c, "demo/%", "percent key", "2026-10-03T00:00:00+00:00");
    let ro = pc::open_ro(&path).unwrap();
    let rows = pc::read_prefix(&ro, "demo/", 10).unwrap();
    let keys: Vec<_> = rows.iter().map(|r| r.nat_key.as_str()).collect();
    assert_eq!(keys, vec!["demo/%", "demo/alpha", "demo/\u{ff3a}"]);
    assert!(pc::read_prefix(&ro, "%", 10).unwrap().is_empty());
}

#[test]
fn prefix_query_plan_avoids_table_scan() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("pulse.sqlite");
    let c = open_fixture_db(&path);
    fill_synthetic(&c, 2_000, "ix/");
    let ro = pc::open_ro(&path).unwrap();
    assert!(
        !prefix_explain_scans_table(&ro, "ix/0005"),
        "prefix read must use the nat_key index, not SCAN memory_traces"
    );
}

#[test]
fn prefix_read_scale_ratio_under_bound() {
    let dir_small = TempDir::new().unwrap();
    let dir_large = TempDir::new().unwrap();
    let small = dir_small.path().join("small.sqlite");
    let large = dir_large.path().join("large.sqlite");
    let c_small = open_fixture_db(&small);
    let c_large = open_fixture_db(&large);
    fill_synthetic(&c_small, PREFIX_SCALE_SMALL_ROWS, "syn/p/");
    fill_synthetic(&c_large, PREFIX_SCALE_LARGE_ROWS, "syn/p/");
    let ro_small = pc::open_ro(&small).unwrap();
    let ro_large = pc::open_ro(&large).unwrap();
    let t_small = timed_prefix(&ro_small, "syn/p/000100", 50);
    let t_large = timed_prefix(&ro_large, "syn/p/002000", 50);
    let ratio = t_large.as_secs_f64() / t_small.as_secs_f64().max(1e-9);
    assert!(
        ratio < PREFIX_SCALE_MAX_RATIO,
        "prefix scale ratio {ratio:.2} exceeds {PREFIX_SCALE_MAX_RATIO} (small {:?}, large {:?})",
        t_small,
        t_large
    );
}

struct RecallCase {
    query: &'static str,
    want_keys: &'static [&'static str],
}

fn build_recall_fixture(path: &PathBuf) {
    let c = open_fixture_db(path);
    let rows: &[(&str, &str, &str)] = &[
        ("rec/k1", "alpha bravo mission one", "2026-08-01T00:00:00+00:00"),
        ("rec/k2", "alpha bravo mission two", "2026-09-01T00:00:00+00:00"),
        ("rec/k3", "gamma delta elsewhere", "2026-08-15T00:00:00+00:00"),
        ("rec/k4", "alpha bravo", "2026-07-01T00:00:00+00:00"),
        ("rec/k5", "mission three alone", "2026-08-20T00:00:00+00:00"),
        ("rec/k6", "alpha elsewhere", "2026-08-10T00:00:00+00:00"),
        ("rec/k7", "bravo elsewhere", "2026-08-11T00:00:00+00:00"),
        ("rec/k8", "noise", "2026-01-01T00:00:00+00:00"),
        ("rec/k9", "alpha bravo mission fresh", "2026-11-01T00:00:00+00:00"),
        ("rec/k10", "alpha bravo mission stale", "2026-01-01T00:00:00+00:00"),
        (
            "rec/s-old",
            "alpha bravo twin",
            "2026-01-01T00:00:00+00:00",
        ),
        (
            "rec/s-new",
            "alpha bravo twin",
            "2026-11-01T00:00:00+00:00",
        ),
    ];
    for (k, snip, ts) in rows {
        insert_trace(&c, k, snip, ts);
    }
}

fn recall_at(conn: &Connection, cases: &[RecallCase], k: usize) -> f64 {
    let mut hits = 0usize;
    let mut total = 0usize;
    for case in cases {
        total += case.want_keys.len();
        let got = pc::read_search(conn, case.query, k).unwrap();
        let got_keys: std::collections::BTreeSet<_> =
            got.iter().map(|r| r.nat_key.as_str()).collect();
        for w in case.want_keys {
            if got_keys.contains(w) {
                hits += 1;
            }
        }
    }
    if total == 0 {
        1.0
    } else {
        hits as f64 / total as f64
    }
}

#[test]
fn search_recall_and_fresh_first_meet_baseline() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("pulse.sqlite");
    build_recall_fixture(&path);
    let conn = pc::open_ro(&path).unwrap();
    let cases = [
        RecallCase {
            query: "alpha bravo mission",
            want_keys: &["rec/k9", "rec/k2", "rec/k1"],
        },
        RecallCase {
            query: "gamma delta",
            want_keys: &["rec/k3"],
        },
    ];
    let r5 = recall_at(&conn, &cases, 5);
    let r10 = recall_at(&conn, &cases, 10);
    assert!(
        r5 >= BASELINE_RECALL_AT_5,
        "recall@5 {r5} < baseline {BASELINE_RECALL_AT_5}"
    );
    assert!(
        r10 >= BASELINE_RECALL_AT_10,
        "recall@10 {r10} < baseline {BASELINE_RECALL_AT_10}"
    );

    let fresh = pc::read_search(&conn, "alpha bravo mission fresh", 5).unwrap();
    assert!(
        fresh.first().is_some_and(|r| r.nat_key == "rec/k9"),
        "newer exact match should rank first, got {:?}",
        fresh.iter().map(|r| &r.nat_key).collect::<Vec<_>>()
    );
    let twins = pc::read_search(&conn, "alpha bravo twin", 5).unwrap();
    let fresh_ok = twins.first().is_some_and(|r| r.nat_key == "rec/s-new");
    assert!(
        fresh_ok,
        "fresh-first: rec/s-new should beat rec/s-old, order {:?}",
        twins.iter().map(|r| &r.nat_key).collect::<Vec<_>>()
    );
    let fresh_rate = if fresh_ok { 1.0 } else { 0.0 };
    assert!(
        fresh_rate >= BASELINE_FRESH_FIRST,
        "fresh-first {fresh_rate} < baseline {BASELINE_FRESH_FIRST}"
    );
}

#[test]
fn search_relevance_beats_one_hour_recency_gap() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("pulse.sqlite");
    let c = open_fixture_db(&path);
    insert_trace(
        &c,
        "rank/strong-old",
        "widget calibration procedure widget calibration procedure detailed steps",
        "2026-06-01T10:00:00Z",
    );
    insert_trace(
        &c,
        "rank/weak-new",
        "widget noise unrelated",
        "2026-06-01T11:00:00Z",
    );
    let conn = pc::open_ro(&path).unwrap();
    let rows = pc::read_search(&conn, "widget calibration procedure", 5).unwrap();
    assert_eq!(
        rows.first().map(|r| r.nat_key.as_str()),
        Some("rank/strong-old"),
        "higher bm25 must beat a one-hour newer weak match, got {:?}",
        rows.iter().map(|r| &r.nat_key).collect::<Vec<_>>()
    );
}

#[test]
fn search_same_terms_newer_beats_older() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("pulse.sqlite");
    let c = open_fixture_db(&path);
    let snippet = "parity twin terms for ranking";
    insert_trace(
        &c,
        "parity/old",
        snippet,
        "2026-05-01T00:00:00Z",
    );
    insert_trace(
        &c,
        "parity/new",
        snippet,
        "2026-10-01T00:00:00Z",
    );
    let conn = pc::open_ro(&path).unwrap();
    let rows = pc::read_search(&conn, "parity twin terms ranking", 5).unwrap();
    assert_eq!(
        rows.first().map(|r| r.nat_key.as_str()),
        Some("parity/new"),
        "same terms: newer row should win, got {:?}",
        rows.iter().map(|r| &r.nat_key).collect::<Vec<_>>()
    );
}

#[test]
fn search_natural_language_strips_stopwords() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("pulse.sqlite");
    let c = open_fixture_db(&path);
    insert_trace(
        &c,
        "nl/box7",
        "box-7 invoice cutoff lands Friday for fixture",
        "2026-10-05T12:00:00+00:00",
    );
    insert_trace(
        &c,
        "nl/noise",
        "unrelated delta token",
        "2026-09-01T12:00:00+00:00",
    );
    let conn = pc::open_ro(&path).unwrap();
    let rows = pc::read_search(
        &conn,
        "when is the box-7 invoice cutoff for Friday",
        5,
    )
    .unwrap();
    assert!(
        rows.iter().any(|r| r.nat_key == "nl/box7"),
        "stopword-heavy query should still hit content tokens, got {:?}",
        rows.iter().map(|r| &r.nat_key).collect::<Vec<_>>()
    );
}

#[test]
fn search_fresh_first_respects_mixed_offset_timestamps() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("pulse.sqlite");
    let c = open_fixture_db(&path);
    let snippet = "offset twin fact for ranking";
    // Text order would favor +05:30; unixepoch makes +00:00Z row newer.
    insert_trace(
        &c,
        "tz/late-text",
        snippet,
        "2026-06-01T10:00:00+05:30",
    );
    insert_trace(
        &c,
        "tz/newer-instant",
        snippet,
        "2026-06-01T04:35:00Z",
    );
    let conn = pc::open_ro(&path).unwrap();
    let rows = pc::read_search(&conn, "offset twin fact ranking", 3).unwrap();
    assert_eq!(
        rows.first().map(|r| r.nat_key.as_str()),
        Some("tz/newer-instant"),
        "mixed-offset last_seen must rank by instant, got {:?}",
        rows.iter().map(|r| (&r.nat_key, &r.last_seen)).collect::<Vec<_>>()
    );
}

#[test]
fn search_or_fallback_fills_when_and_is_sparse() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("pulse.sqlite");
    let c = open_fixture_db(&path);
    insert_trace(
        &c,
        "or/a",
        "only alpha token",
        "2026-10-01T00:00:00+00:00",
    );
    insert_trace(
        &c,
        "or/b",
        "only bravo token",
        "2026-10-02T00:00:00+00:00",
    );
    let conn = pc::open_ro(&path).unwrap();
    let rows = pc::read_search(&conn, "alpha bravo", 5).unwrap();
    let keys: std::collections::BTreeSet<_> = rows.iter().map(|r| r.nat_key.as_str()).collect();
    assert!(keys.contains("or/a") && keys.contains("or/b"));
}