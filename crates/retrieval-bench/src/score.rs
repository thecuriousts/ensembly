//! Recall and fresh-first scoring against a query set.

use ensembly_kernel::pulse_core::{self as pc, TraceRow};

use crate::queries::QuerySet;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct QualityScores {
    pub recall_at_5: f64,
    pub recall_at_10: f64,
    pub fresh_first_accuracy: f64,
    pub unique_hits_at_10: u32,
    pub fresh_correct: u32,
    pub fresh_total: u32,
}

pub fn score_quality(
    conn: &rusqlite::Connection,
    queries: &QuerySet,
) -> anyhow::Result<QualityScores> {
    let mut unique_hits = 0u32;
    for q in &queries.unique {
        let rows = pc::read_search(conn, &q.query, 10)?;
        if rows.iter().any(|r| r.nat_key == q.expect_key) {
            unique_hits += 1;
        }
    }
    let recall_at_5 = recall_unique(conn, &queries.unique, 5)?;
    let recall_at_10 = recall_unique(conn, &queries.unique, 10)?;
    let (fresh_correct, fresh_total) = score_fresh(conn, &queries.fresh_pairs)?;
    Ok(QualityScores {
        recall_at_5,
        recall_at_10,
        fresh_first_accuracy: if fresh_total == 0 {
            0.0
        } else {
            fresh_correct as f64 / fresh_total as f64
        },
        unique_hits_at_10: unique_hits,
        fresh_correct,
        fresh_total,
    })
}

fn recall_unique(
    conn: &rusqlite::Connection,
    items: &[crate::queries::UniqueQuery],
    k: usize,
) -> anyhow::Result<f64> {
    if items.is_empty() {
        return Ok(0.0);
    }
    let mut hits = 0u32;
    for q in items {
        let rows = pc::read_search(conn, &q.query, k)?;
        if rows.iter().any(|r| r.nat_key == q.expect_key) {
            hits += 1;
        }
    }
    Ok(hits as f64 / items.len() as f64)
}

fn score_fresh(
    conn: &rusqlite::Connection,
    pairs: &[crate::queries::FreshPairQuery],
) -> anyhow::Result<(u32, u32)> {
    let mut correct = 0u32;
    for p in pairs {
        let rows = pc::read_search(conn, &p.query, 5)?;
        if first_live_key(&rows) == p.expect_first_key {
            correct += 1;
        }
    }
    Ok((correct, pairs.len() as u32))
}

fn first_live_key(rows: &[TraceRow]) -> String {
    rows.first().map(|r| r.nat_key.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::GeneratedBench;
    use crate::DEFAULT_SEED;

    #[test]
    fn recall_perfect_on_planted_data() {
        let tmp = tempfile::tempdir().unwrap();
        let g = GeneratedBench::generate(tmp.path(), 1_000, DEFAULT_SEED).unwrap();
        let conn = rusqlite::Connection::open(&g.pulse_db).unwrap();
        let qs = QuerySet::build();
        let s = score_quality(&conn, &qs).unwrap();
        assert!(s.recall_at_10 >= 0.99, "recall@10={}", s.recall_at_10);
        assert!(s.fresh_first_accuracy >= 0.99);
    }
}
