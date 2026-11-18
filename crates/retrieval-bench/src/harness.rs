//! Latency harness using production query entry points.

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use ensembly_kernel::pulse_core as pc;
use ensembly_kernel::pulse_index::{self, ApplyOptions};
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
    pub pulse_index: PulseIndexLatency,
    pub quality: QualityScores,
    pub explain_query_plan: ExplainPlans,
    pub record_versions: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PulseIndexLatency {
    pub machine: String,
    /// Fraction of rows matching `pulse list` (kind=memory, not superseded).
    pub live_memory_fraction: f64,
    pub explain_pulse_list_no_index: Vec<String>,
    pub explain_pulse_list_with_index: Vec<String>,
    pub pulse_list_no_index: LatencyMs,
    pub pulse_list_with_index: LatencyMs,
    pub dangling_legacy_no_index: LatencyMs,
    pub dangling_new_no_index: LatencyMs,
    pub dangling_legacy_with_index: LatencyMs,
    pub dangling_new_with_index: LatencyMs,
    /// Synthetic store: ~5% live rows (supersede the rest on a DB copy).
    pub sparse_live_fraction: f64,
    pub sparse_pulse_list_no_index: LatencyMs,
    pub sparse_pulse_list_with_index: LatencyMs,
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
    let live_memory_fraction = live_memory_fraction(&gen.pulse_db)?;
    let probe_db = gen.pulse_db.with_file_name("pulse_index_probe.sqlite");
    if probe_db.exists() {
        std::fs::remove_file(&probe_db)?;
    }
    std::fs::copy(&gen.pulse_db, &probe_db)?;
    let pulse_index = measure_pulse_index_latencies(&probe_db, live_memory_fraction)?;
    let conn = pc::open_ro(&gen.pulse_db)?;
    let quality = score_quality(&conn, &gen.queries)?;
    let latency_ms = measure_latencies(&gen, &conn)?;
    let explain_query_plan = collect_explains(&gen.pulse_db)?;
    Ok(SizeReport {
        size,
        seed,
        latency_ms,
        pulse_index,
        quality,
        explain_query_plan,
        record_versions: gen.record_versions,
    })
}

fn measure_pulse_index_latencies(
    db_path: &Path,
    live_memory_fraction: f64,
) -> Result<PulseIndexLatency> {
    let (sparse_live_fraction, sparse_pulse_list_no_index, sparse_pulse_list_with_index) =
        measure_sparse_pulse_list(db_path)?;
    let explain_pulse_list_no_index = explain_pulse_list(db_path)?;
    let conn = pc::open_ro(db_path)?;
    let pulse_list_no_index = bench(|| {
        let _ = ps::list_live(&conn, LIST_LIVE_LIMIT)?;
        Ok(())
    });
    let dangling_legacy_no_index = bench(|| {
        let _ = pc::dangling_superseded_keys_legacy(&conn)?;
        Ok(())
    });
    let dangling_new_no_index = bench(|| {
        let _ = pc::dangling_superseded_keys(&conn)?;
        Ok(())
    });
    drop(conn);

    pulse_index::apply(
        db_path,
        "laptop-1",
        ApplyOptions {
            dry_run: false,
            analyze: false,
        },
    )?;

    let explain_pulse_list_with_index = explain_pulse_list(db_path)?;
    let conn = pc::open_ro(db_path)?;
    let pulse_list_with_index = bench(|| {
        let _ = ps::list_live(&conn, LIST_LIVE_LIMIT)?;
        Ok(())
    });
    let dangling_legacy_with_index = bench(|| {
        let _ = pc::dangling_superseded_keys_legacy(&conn)?;
        Ok(())
    });
    let dangling_new_with_index = bench(|| {
        let _ = pc::dangling_superseded_keys(&conn)?;
        Ok(())
    });
    Ok(PulseIndexLatency {
        machine: machine_label(),
        live_memory_fraction,
        explain_pulse_list_no_index,
        explain_pulse_list_with_index,
        pulse_list_no_index,
        pulse_list_with_index,
        dangling_legacy_no_index,
        dangling_new_no_index,
        dangling_legacy_with_index,
        dangling_new_with_index,
        sparse_live_fraction,
        sparse_pulse_list_no_index,
        sparse_pulse_list_with_index,
    })
}

fn live_memory_fraction(db_path: &Path) -> Result<f64> {
    let conn = Connection::open(db_path)?;
    conn.query_row(
        "SELECT (SELECT COUNT(*) FROM memory_traces WHERE kind = 'memory' AND (superseded_by IS NULL OR superseded_by = '')) * 1.0 / COUNT(*) FROM memory_traces",
        [],
        |r| r.get(0),
    )
    .map_err(Into::into)
}

const PULSE_LIST_EXPLAIN_SQL: &str =
    "SELECT nat_key, snippet, source, status, kind, lock, seen_count, superseded_by, first_seen, last_seen \
     FROM memory_traces WHERE kind = 'memory' AND (superseded_by IS NULL OR superseded_by = '') \
     ORDER BY time DESC, nat_key ASC LIMIT ?1";

fn explain_pulse_list(db_path: &Path) -> Result<Vec<String>> {
    let conn = Connection::open(db_path)?;
    explain(&conn, PULSE_LIST_EXPLAIN_SQL, params![LIST_LIVE_LIMIT])
}

fn measure_sparse_pulse_list(pristine: &Path) -> Result<(f64, LatencyMs, LatencyMs)> {
    let sparse_path = pristine.with_file_name("pulse_index_sparse.sqlite");
    if sparse_path.exists() {
        std::fs::remove_file(&sparse_path)?;
    }
    std::fs::copy(pristine, &sparse_path)?;
    {
        let conn = Connection::open(&sparse_path)?;
        conn.execute_batch(
            "UPDATE memory_traces SET superseded_by = 'bench/missing/' || (id % 97) \
             WHERE id % 20 != 0 AND (superseded_by IS NULL OR superseded_by = '')",
        )?;
    }
    let sparse_live_fraction = live_memory_fraction(&sparse_path)?;
    let conn = pc::open_ro(&sparse_path)?;
    let sparse_no = bench(|| {
        let _ = ps::list_live(&conn, LIST_LIVE_LIMIT)?;
        Ok(())
    });
    drop(conn);
    pulse_index::apply(
        &sparse_path,
        "laptop-1",
        ApplyOptions {
            dry_run: false,
            analyze: false,
        },
    )?;
    let conn = pc::open_ro(&sparse_path)?;
    let sparse_with = bench(|| {
        let _ = ps::list_live(&conn, LIST_LIVE_LIMIT)?;
        Ok(())
    });
    Ok((sparse_live_fraction, sparse_no, sparse_with))
}

/// Label for benchmark reports: explicit `RETRIEVAL_BENCH_MACHINE` or `unspecified`.
pub fn machine_label() -> String {
    std::env::var("RETRIEVAL_BENCH_MACHINE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unspecified".into())
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
            "SELECT t.nat_key FROM memory_traces t \
             LEFT JOIN memory_traces u ON u.nat_key = t.superseded_by \
             WHERE t.superseded_by IS NOT NULL AND t.superseded_by <> '' AND u.nat_key IS NULL ORDER BY t.nat_key",
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
