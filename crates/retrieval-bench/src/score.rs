//! Recall and fresh-first scoring per query type.

use ensembly_kernel::pulse_core::{self as pc, TraceRow};

use crate::queries::{KeyedQuery, QuerySet};

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct TypeRecall {
    pub count: u32,
    pub recall_at_5: f64,
    pub recall_at_10: f64,
    pub hits_at_10: u32,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct QualityScores {
    pub unique: TypeRecall,
    pub natural_phrase: TypeRecall,
    pub paraphrase_extra: TypeRecall,
    pub fts_zero_hit: TypeRecall,
    pub prefix: TypeRecall,
    pub fresh_first_accuracy: f64,
    pub fresh_correct: u32,
    pub fresh_total: u32,
}

pub fn score_quality(
    conn: &rusqlite::Connection,
    queries: &QuerySet,
) -> anyhow::Result<QualityScores> {
    let (fresh_correct, fresh_total) = score_fresh(conn, &queries.fresh_pairs)?;
    Ok(QualityScores {
        unique: recall_search(conn, &queries.unique, 5, 10)?,
        natural_phrase: recall_search(conn, &queries.natural_phrase, 5, 10)?,
        paraphrase_extra: recall_search(conn, &queries.paraphrase_extra, 5, 10)?,
        fts_zero_hit: recall_search(conn, &queries.fts_zero_hit, 5, 10)?,
        prefix: recall_prefix(conn, &queries.prefixes)?,
        fresh_first_accuracy: if fresh_total == 0 {
            0.0
        } else {
            fresh_correct as f64 / fresh_total as f64
        },
        fresh_correct,
        fresh_total,
    })
}

fn recall_search(
    conn: &rusqlite::Connection,
    items: &[KeyedQuery],
    k5: usize,
    k10: usize,
) -> anyhow::Result<TypeRecall> {
    if items.is_empty() {
        return Ok(TypeRecall::default());
    }
    let mut hits_at_10 = 0u32;
    let mut hits_at_5 = 0u32;
    for q in items {
        let rows10 = pc::read_search(conn, &q.query, k10)?;
        if rows10.iter().any(|r| r.nat_key == q.expect_key) {
            hits_at_10 += 1;
        }
        let rows5 = pc::read_search(conn, &q.query, k5)?;
        if rows5.iter().any(|r| r.nat_key == q.expect_key) {
            hits_at_5 += 1;
        }
    }
    let n = items.len() as f64;
    Ok(TypeRecall {
        count: items.len() as u32,
        recall_at_5: hits_at_5 as f64 / n,
        recall_at_10: hits_at_10 as f64 / n,
        hits_at_10,
    })
}

fn recall_prefix(
    conn: &rusqlite::Connection,
    prefixes: &[crate::queries::PrefixQuery],
) -> anyhow::Result<TypeRecall> {
    if prefixes.is_empty() {
        return Ok(TypeRecall::default());
    }
    let mut hits = 0u32;
    for p in prefixes {
        let rows = pc::read_prefix(conn, &p.prefix, 32)?;
        let keys: Vec<_> = rows.iter().map(|r| r.nat_key.as_str()).collect();
        if p.expect_keys.iter().all(|ek| keys.contains(&ek.as_str())) {
            hits += 1;
        }
    }
    let n = prefixes.len() as f64;
    Ok(TypeRecall {
        count: prefixes.len() as u32,
        recall_at_5: hits as f64 / n,
        recall_at_10: hits as f64 / n,
        hits_at_10: hits,
    })
}

fn score_fresh(
    conn: &rusqlite::Connection,
    pairs: &[crate::queries::FreshPairQuery],
) -> anyhow::Result<(u32, u32)> {
    let mut correct = 0u32;
    for p in pairs {
        let rows = pc::read_search(conn, &p.query, 5)?;
        if first_key(&rows) == p.expect_first_key {
            correct += 1;
        }
    }
    Ok((correct, pairs.len() as u32))
}

fn first_key(rows: &[TraceRow]) -> String {
    rows.first().map(|r| r.nat_key.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::GeneratedBench;
    use crate::DEFAULT_SEED;

    #[test]
    fn unique_and_prefix_recall_high_on_planted_data() {
        let tmp = tempfile::tempdir().unwrap();
        let g = GeneratedBench::generate(tmp.path(), 1_000, DEFAULT_SEED).unwrap();
        let conn = rusqlite::Connection::open(&g.pulse_db).unwrap();
        let qs = QuerySet::build();
        let s = score_quality(&conn, &qs).unwrap();
        assert!(s.unique.recall_at_10 >= 0.99);
        assert!(s.prefix.recall_at_10 >= 0.99);
        assert!(s.fresh_first_accuracy >= 0.99);
    }

    #[test]
    fn natural_phrase_and_fts_zero_baseline_low() {
        let tmp = tempfile::tempdir().unwrap();
        let g = GeneratedBench::generate(tmp.path(), 1_000, DEFAULT_SEED).unwrap();
        let conn = rusqlite::Connection::open(&g.pulse_db).unwrap();
        let qs = QuerySet::build();
        let s = score_quality(&conn, &qs).unwrap();
        assert!(
            s.natural_phrase.recall_at_10 <= 0.25,
            "nl={}",
            s.natural_phrase.recall_at_10
        );
        assert!(
            s.fts_zero_hit.recall_at_10 <= 0.25,
            "fts0={}",
            s.fts_zero_hit.recall_at_10
        );
        assert!(
            s.paraphrase_extra.recall_at_10 <= 0.25,
            "para={}",
            s.paraphrase_extra.recall_at_10
        );
    }
}
