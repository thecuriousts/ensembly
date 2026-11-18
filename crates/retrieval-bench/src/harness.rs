//! Latency harness using production query entry points.

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use ensembly_kernel::pulse_core as pc;
use ensembly_kernel::pulse_supersede as ps;
use ensembly_schema::{commit_json, feed_json, snapshot_json, CommitFault};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::generate::{
    GeneratedBench, FALLBACK_BENCH_QUERY, FTS_WARM_QUERY, LIST_LIVE_LIMIT, PREFIX_BENCH,
};
use crate::score::{score_quality, QualityScores};

const WARMUP: usize = 5;
const ITERS: usize = 40;

#[derive(Debug, Clone, Serialize)]
pub struct LatencyMs {
    pub p50: f64,
    pub p95: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SizeReport {
    pub size: usize,
    pub seed: u64,
    pub latency_ms: OperationLatencies,
    pub quality: QualityScores,
    pub explain_query_plan: ExplainPlans,
    pub record_versions: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationLatencies {
    pub prefix_read: LatencyMs,
    pub fts_search: LatencyMs,
    pub fallback_text_search: LatencyMs,
    pub pulse_list: LatencyMs,
    pub dangling_supersession_check: LatencyMs,
    pub record_log_snapshot: LatencyMs,
    pub single_event_commit: LatencyMs,
    pub feed_last_10: LatencyMs,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExplainPlans {
    pub prefix_read: Vec<String>,
    pub fts_search: Vec<String>,
    pub fallback_text_search: Vec<String>,
    pub pulse_list: Vec<String>,
    pub dangling_supersession: Vec<String>,
}

pub fn run_size(work: &Path, size: usize, seed: u64) -> Result<SizeReport> {
    std::env::set_var(ensembly_schema::WRITER_ENV, "laptop-1");
    let gen = GeneratedBench::generate(work, size, seed)?;
    let conn = pc::open_ro(&gen.pulse_db)?;
    let quality = score_quality(&conn, &gen.queries)?;
    let latency_ms = measure_latencies(&gen, &conn)?;
    let explain_query_plan = collect_explains(&gen.pulse_db)?;
    Ok(SizeReport {
        size,
        seed,
        latency_ms,
        quality,
        explain_query_plan,
        record_versions: gen.record_versions,
    })
}

fn measure_latencies(gen: &GeneratedBench, conn: &Connection) -> Result<OperationLatencies> {
    Ok(OperationLatencies {
        prefix_read: bench(|| {
            let _ = pc::read_prefix(conn, PREFIX_BENCH, 20)?;
            Ok(())
        }),
        fts_search: bench(|| {
            let _ = pc::read_search(conn, FTS_WARM_QUERY, 10)?;
            Ok(())
        }),
        fallback_text_search: bench(|| {
            let _ = pc::read_search(conn, FALLBACK_BENCH_QUERY, 10)?;
            Ok(())
        }),
        pulse_list: bench(|| {
            let _ = ps::list_live(conn, LIST_LIVE_LIMIT)?;
            Ok(())
        }),
        dangling_supersession_check: bench(|| {
            let _ = pc::db_stats(conn)?;
            Ok(())
        }),
        record_log_snapshot: bench(|| {
            let _ = snapshot_json(&gen.record_log, None)?;
            Ok(())
        }),
        single_event_commit: bench(|| {
            let uid = ulid::Ulid::new();
            let unit = serde_json::json!([{
                "id": uid.to_string(),
                "aggregate": format!("agg://bench.test/commit/{uid}"),
                "key": format!("bench-single-{uid}"),
                "version": 1,
                "event_time": "2026-06-15T10:00:00+00:00",
                "record_time": "2026-06-15T10:00:01+00:00",
                "source": "owner",
                "schema_v": 1,
                "payload": { "writer": "laptop-1", "note": "bench single commit" }
            }]);
            let _ = commit_json(&gen.record_log, &unit.to_string(), &CommitFault::default())?;
            Ok(())
        }),
        feed_last_10: bench(|| {
            let since = gen.record_versions.saturating_sub(10);
            let _ = feed_json(&gen.record_log, since)?;
            Ok(())
        }),
    })
}

fn bench<F: FnMut() -> Result<()>>(mut op: F) -> LatencyMs {
    for _ in 0..WARMUP {
        let _ = op();
    }
    let mut samples = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let t0 = Instant::now();
        let _ = op();
        samples.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    LatencyMs {
        p50: percentile(&samples, 0.50),
        p95: percentile(&samples, 0.95),
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn collect_explains(db_path: &Path) -> Result<ExplainPlans> {
    let conn = Connection::open(db_path)?;
    Ok(ExplainPlans {
        prefix_read: explain(
            &conn,
            "SELECT nat_key, snippet, source, status, kind, lock, seen_count, superseded_by, first_seen, last_seen \
             FROM memory_traces WHERE substr(nat_key, 1, length(?1)) = ?1 ORDER BY nat_key LIMIT ?2",
            params![PREFIX_BENCH, 20i64],
        )?,
        fts_search: explain(
            &conn,
            "SELECT t.nat_key, t.snippet, t.source, t.status, t.kind, t.lock, t.seen_count, t.superseded_by, t.first_seen, t.last_seen \
             FROM memory_traces t JOIN memory_fts f ON f.rowid = t.id WHERE memory_fts MATCH ?1 ORDER BY t.last_seen DESC LIMIT ?2",
            params![format!("\"{}\"", FTS_WARM_QUERY), 10i64],
        )?,
        fallback_text_search: explain(
            &conn,
            "SELECT nat_key, snippet, source, status, kind, lock, seen_count, superseded_by, first_seen, last_seen \
             FROM memory_traces WHERE nat_key LIKE ?1 ESCAPE '\\' OR snippet LIKE ?1 ESCAPE '\\' ORDER BY last_seen DESC LIMIT ?2",
            params![format!("%{}%", FALLBACK_BENCH_QUERY.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")), 10i64],
        )?,
        pulse_list: explain(
            &conn,
            "SELECT nat_key, snippet, source, status, kind, lock, seen_count, superseded_by, first_seen, last_seen \
             FROM memory_traces WHERE kind = 'memory' AND (superseded_by IS NULL OR superseded_by = '') \
             ORDER BY time DESC, nat_key ASC LIMIT ?1",
            params![LIST_LIVE_LIMIT],
        )?,
        dangling_supersession: explain(
            &conn,
            "SELECT nat_key FROM memory_traces t WHERE superseded_by IS NOT NULL AND superseded_by <> '' \
             AND NOT EXISTS (SELECT 1 FROM memory_traces u WHERE u.nat_key = t.superseded_by) ORDER BY nat_key",
            [],
        )?,
    })
}

fn explain(conn: &Connection, sql: &str, params: impl rusqlite::Params) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
    let rows = stmt.query_map(params, |r| r.get::<_, String>(3))?;
    Ok(rows
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .map(|s| s.trim().to_string())
        .collect())
}
