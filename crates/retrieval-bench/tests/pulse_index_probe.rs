//! Pulse index honesty: pristine DB has no ensembly indexes; list matches kernel SQL.

use retrieval_bench::generate::GeneratedBench;
use retrieval_bench::DEFAULT_SEED;
use rusqlite::{params, Connection};

const LIST_SQL: &str = "SELECT nat_key FROM memory_traces WHERE kind = 'memory' AND (superseded_by IS NULL OR superseded_by = '') ORDER BY time DESC, nat_key ASC LIMIT 50";

#[test]
fn generated_db_starts_without_ensembly_indexes() {
    let dir = tempfile::tempdir().unwrap();
    let g = GeneratedBench::generate(dir.path(), 1_000, DEFAULT_SEED).unwrap();
    let n: i64 = Connection::open(&g.pulse_db)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name LIKE 'ensembly_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 0);
}

#[test]
fn explain_before_indexes_uses_no_ensembly_index() {
    let dir = tempfile::tempdir().unwrap();
    let g = GeneratedBench::generate(dir.path(), 10_000, DEFAULT_SEED).unwrap();
    let conn = Connection::open(&g.pulse_db).unwrap();
    let live: f64 = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM memory_traces WHERE kind = 'memory' AND (superseded_by IS NULL OR superseded_by = '')) * 1.0 / COUNT(*) FROM memory_traces",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        live > 0.99,
        "retrieval-bench corpus is almost all live memory: {live}"
    );
    let mut stmt = conn
        .prepare(&format!("EXPLAIN QUERY PLAN {LIST_SQL}"))
        .unwrap();
    let details: Vec<String> = stmt
        .query_map(params![], |r| r.get::<_, String>(3))
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    let joined = details.join(" | ");
    assert!(
        !joined.contains("ensembly_memory_traces_live_list"),
        "partial list index should not be chosen on dense live corpus: {joined}"
    );
}
