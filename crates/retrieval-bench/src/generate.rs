//! Seeded synthetic pulse DB and record log generation.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, FixedOffset, TimeZone, Utc};
use ensembly_schema::{commit_json, CommitFault};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rusqlite::{params, Connection};
use ulid::Ulid;

use crate::queries::QuerySet;
use crate::schema::PULSE_SCHEMA;
pub const FTS_WARM_QUERY: &str = "uqterm-bench-0000";
/// FTS-hostile input (see `pulse_core` tests); `read_search` falls back to LIKE.
pub const FALLBACK_BENCH_QUERY: &str = "\"broken\" AND (";
pub const PREFIX_BENCH: &str = "bench/pfx/00/";
pub const LIST_LIVE_LIMIT: i64 = 50;

#[derive(Debug, Clone)]
pub struct GeneratedBench {
    pub pulse_db: PathBuf,
    pub record_log: PathBuf,
    pub queries: QuerySet,
    pub size: usize,
    pub seed: u64,
    pub record_versions: u64,
}

/// Deterministic fingerprint for regression tests (hash of planted key list + filler count).
pub fn generation_fingerprint(work: &Path, size: usize, seed: u64) -> String {
    let g = GeneratedBench::generate(work, size, seed).unwrap();
    let conn = Connection::open(&g.pulse_db).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM memory_traces", [], |r| r.get(0))
        .unwrap();
    format!("{size}:{seed}:{n}")
}

pub struct GenerateBench {
    work: PathBuf,
    size: usize,
    seed: u64,
}

impl GenerateBench {
    pub fn new(work: impl AsRef<Path>, size: usize, seed: u64) -> Self {
        Self {
            work: work.as_ref().to_path_buf(),
            size,
            seed,
        }
    }

    pub fn generate(self) -> Result<GeneratedBench> {
        GeneratedBench::generate(&self.work, self.size, self.seed)
    }
}

impl GeneratedBench {
    pub fn generate(work: &Path, size: usize, seed: u64) -> Result<Self> {
        let planted = QuerySet::planted_row_count();
        if size < planted {
            anyhow::bail!("size must be at least {planted} rows ({planted} planted + filler)");
        }
        std::fs::create_dir_all(work)?;
        let pulse_db = work.join("pulse.sqlite");
        let record_log = work.join("events.jsonl");
        let _ = std::fs::remove_file(&pulse_db);
        let _ = std::fs::remove_file(&record_log);
        let queries = QuerySet::build();
        let mut rng = StdRng::seed_from_u64(seed);
        populate_pulse(&pulse_db, size, &queries, &mut rng)?;
        let record_versions = populate_record_log(&record_log, size, seed, &mut rng)?;
        Ok(Self {
            pulse_db,
            record_log,
            queries,
            size,
            seed,
            record_versions,
        })
    }
}

fn populate_pulse(path: &Path, size: usize, queries: &QuerySet, rng: &mut StdRng) -> Result<()> {
    let conn = Connection::open(path).context("open pulse db")?;
    conn.execute_batch(PULSE_SCHEMA)?;
    let base = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
    let filler_target = size.saturating_sub(QuerySet::planted_row_count());
    let tx = conn.unchecked_transaction()?;
    for i in 0..filler_target {
        let key = format!("bench/filler/{i:06}");
        let snippet = format!("filler note {} noise {}", i, rng.gen::<u32>());
        let t = (base + Duration::seconds(i as i64)).to_rfc3339();
        insert_trace(&tx, &key, &snippet, &t, &t, None)?;
    }
    for q in &queries.unique {
        let snippet = format!("planted unique token {}", q.query);
        let t = (base + Duration::days(10)).to_rfc3339();
        insert_trace(&tx, &q.expect_key, &snippet, &t, &t, None)?;
    }
    for q in &queries.natural_phrase {
        let anchor = q
            .query
            .split_whitespace()
            .find(|w| w.starts_with("anchor-nl-"))
            .unwrap_or("anchor-nl-00");
        let snippet = format!("operator note about {anchor} on laptop-1 desk");
        let t = (base + Duration::days(11)).to_rfc3339();
        insert_trace(&tx, &q.expect_key, &snippet, &t, &t, None)?;
    }
    for q in &queries.paraphrase_extra {
        let topic = q.query.split_whitespace().next().unwrap_or("para-topic-00");
        let snippet = format!("{topic} certref stored for bench");
        let t = (base + Duration::days(12)).to_rfc3339();
        insert_trace(&tx, &q.expect_key, &snippet, &t, &t, None)?;
    }
    for q in &queries.fts_zero_hit {
        let parts: Vec<_> = q.query.split_whitespace().collect();
        let topic = parts.get(1).copied().unwrap_or("topic-00");
        let snippet = format!("ftszero {topic} only in corpus");
        let t = (base + Duration::days(13)).to_rfc3339();
        insert_trace(&tx, &q.expect_key, &snippet, &t, &t, None)?;
    }
    for (i, pair) in queries.fresh_pairs.iter().enumerate() {
        let old_dt = base + Duration::days(20) + Duration::hours(i as i64);
        let new_dt = base + Duration::days(40) + Duration::hours(i as i64);
        let old_ls = format_last_seen(old_dt, &pair.tz_style);
        let new_ls = format_last_seen(new_dt, &pair.tz_style);
        let snippet_old = format!("shared term {} variant old", pair.query);
        let snippet_new = format!("shared term {} variant new", pair.query);
        insert_trace(
            &tx,
            &pair.old_key,
            &snippet_old,
            &old_ls,
            &old_ls,
            Some(&pair.new_key),
        )?;
        insert_trace(&tx, &pair.new_key, &snippet_new, &new_ls, &new_ls, None)?;
    }
    for p in &queries.prefixes {
        let t = (base + Duration::days(5)).to_rfc3339();
        let key = p.expect_keys[0].clone();
        let snippet = format!("prefix leaf for {}", p.prefix);
        insert_trace(&tx, &key, &snippet, &t, &t, None)?;
    }
    insert_trace(
        &tx,
        "bench/dangle/stale",
        "dangling supersession sample",
        &base.to_rfc3339(),
        &base.to_rfc3339(),
        None,
    )?;
    tx.execute(
        "UPDATE memory_traces SET superseded_by = 'bench/dangle/missing' WHERE nat_key = 'bench/dangle/stale'",
        [],
    )?;
    tx.commit()?;
    Ok(())
}

fn format_last_seen(dt: DateTime<Utc>, style: &str) -> String {
    match style {
        "Z" => {
            let s = dt.to_rfc3339();
            s.replacen("+00:00", "Z", 1)
        }
        "+00:00" => dt.to_rfc3339(),
        "+02:00" => dt
            .with_timezone(&FixedOffset::east_opt(2 * 3600).unwrap())
            .to_rfc3339(),
        "+05:30" => dt
            .with_timezone(&FixedOffset::east_opt((5 * 3600) + (30 * 60)).unwrap())
            .to_rfc3339(),
        _ => dt.to_rfc3339(),
    }
}

fn insert_trace(
    conn: &Connection,
    key: &str,
    snippet: &str,
    time: &str,
    last_seen: &str,
    superseded_by: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, first_seen, last_seen, superseded_by)
         VALUES (?1, ?2, 'bench-fixture', ?3, 'user-stated', 'memory', ?3, ?4, ?5)",
        params![key, snippet, time, last_seen, superseded_by],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO memory_fts(rowid, nat_key, snippet, source) VALUES (?1, ?2, ?3, 'bench-fixture')",
        params![id, key, snippet],
    )?;
    Ok(())
}

fn populate_record_log(path: &Path, size: usize, seed: u64, rng: &mut StdRng) -> Result<u64> {
    std::env::set_var(ensembly_schema::WRITER_ENV, "laptop-1");
    let batch = 50usize;
    let mut version = 0u64;
    let base = Utc.with_ymd_and_hms(2026, 5, 1, 8, 0, 0).unwrap();
    let mut remaining = size;
    let mut batch_idx = 0u64;
    while remaining > 0 {
        let take = remaining.min(batch);
        let events_json = build_unit_json(take, version, batch_idx, seed, base, rng)?;
        commit_json(path, &events_json, &CommitFault::default())
            .map_err(|e| anyhow::anyhow!("commit batch {batch_idx}: {e}"))?;
        version += take as u64;
        remaining -= take;
        batch_idx += 1;
    }
    Ok(version)
}

fn build_unit_json(
    count: usize,
    start_version: u64,
    batch: u64,
    seed: u64,
    base: chrono::DateTime<Utc>,
    rng: &mut StdRng,
) -> Result<String> {
    let mut events = Vec::new();
    for i in 0..count {
        let v = start_version + i as u64 + 1;
        let id = Ulid::from_parts(
            (seed ^ batch).wrapping_mul(1_000_003),
            v.wrapping_mul(0x9E37_79B9_7F4A_7C15) as u128,
        );
        let agg = format!("agg://bench.test/run/{batch}/{i}");
        let key = format!("bench-record-{batch}-{i}");
        let event_time = (base + Duration::seconds((v * 17) as i64)).to_rfc3339();
        let record_time = (base + Duration::seconds((v * 17 + 3) as i64)).to_rfc3339();
        let note = format!(
            "synthetic record event seed={seed} batch={batch} idx={i} noise={}",
            rng.gen::<u32>()
        );
        events.push(serde_json::json!({
            "id": id.to_string(),
            "aggregate": agg,
            "key": key,
            "version": v,
            "event_time": event_time,
            "record_time": record_time,
            "source": "owner",
            "schema_v": 1,
            "payload": { "writer": "laptop-1", "note": note }
        }));
    }
    Ok(serde_json::to_string(&events)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_SEED;

    #[test]
    fn same_seed_same_fingerprint() {
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();
        let a = generation_fingerprint(d1.path(), 500, DEFAULT_SEED);
        let b = generation_fingerprint(d2.path(), 500, DEFAULT_SEED);
        assert_eq!(a, b);
    }

    #[test]
    fn different_seed_different_fingerprint() {
        let d1 = tempfile::tempdir().unwrap();
        let d2 = tempfile::tempdir().unwrap();
        let a = generation_fingerprint(d1.path(), 500, DEFAULT_SEED);
        let b = generation_fingerprint(d2.path(), 500, DEFAULT_SEED + 1);
        assert_ne!(a, b);
    }
}
