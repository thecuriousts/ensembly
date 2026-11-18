//! Generator determinism: same seed → same row count and key sample.

use retrieval_bench::generate::GeneratedBench;
use retrieval_bench::DEFAULT_SEED;
use rusqlite::Connection;

fn key_sample(db: &std::path::Path) -> Vec<String> {
    let conn = Connection::open(db).unwrap();
    let mut stmt = conn
        .prepare("SELECT nat_key FROM memory_traces ORDER BY nat_key LIMIT 5")
        .unwrap();
    stmt.query_map([], |r| r.get(0))
        .unwrap()
        .filter_map(Result::ok)
        .collect()
}

#[test]
fn same_seed_same_keys() {
    let s1 = tempfile::tempdir().unwrap();
    let s2 = tempfile::tempdir().unwrap();
    let g1 = GeneratedBench::generate(s1.path(), 1_000, DEFAULT_SEED).unwrap();
    let g2 = GeneratedBench::generate(s2.path(), 1_000, DEFAULT_SEED).unwrap();
    assert_eq!(key_sample(&g1.pulse_db), key_sample(&g2.pulse_db));
    assert_eq!(
        std::fs::metadata(&g1.record_log).unwrap().len(),
        std::fs::metadata(&g2.record_log).unwrap().len()
    );
}
