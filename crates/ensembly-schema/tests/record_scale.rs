//! Scaling checks for append commit and version-scoped feed (run with `--release`).
//!
//! Set `RECORD_SCALE_BENCH=1` to print 1k / 10k / 100k timings in the test log.

use ensembly_schema::{
    commit_json, feed_json, snapshot_json, AggregateId, CommitFault, EventEnvelope, EventId,
    IdempotencyKey, JsonPayload, Rfc3339Time, Source, UnitOfWork, Version, SCHEMA_VERSION,
};
use std::path::PathBuf;
use std::time::Instant;

fn temp_log() -> (PathBuf, PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("record-scale-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.clone(), dir.join("events.jsonl"))
}

fn fixture_event_id(n: u64) -> EventId {
    const BASES: [&str; 4] = [
        "01JCV1Q8Z00000000000000000",
        "01JCV1Q8Z10000000000000000",
        "01JCV1Q8Z20000000000000000",
        "01JCV1Q8Z30000000000000000",
    ];
    EventId(BASES[(n as usize) % BASES.len()].parse().unwrap())
}

fn synth_event(n: u64) -> EventEnvelope<JsonPayload> {
    EventEnvelope::with_payload(
        fixture_event_id(n),
        AggregateId(format!("agg://example.test/scale/{n}")),
        IdempotencyKey(format!("scale-key-{n}")),
        Version(n),
        Rfc3339Time::new("2026-10-26T18:59:00+01:00").unwrap(),
        Rfc3339Time::new("2026-10-26T19:00:00+01:00").unwrap(),
        Source::Owner,
        ensembly_schema::SCHEMA_VERSION_LEGACY,
        JsonPayload(serde_json::json!({
            "writer": "laptop-1",
            "note": format!("n={n}")
        })),
    )
}

fn seed_log(log: &PathBuf, count: usize) {
    std::env::set_var("ENSEMBLY_WRITER", "laptop-1");
    for n in 1..=count {
        let unit = UnitOfWork {
            events: vec![synth_event(n as u64)],
        };
        let json = serde_json::to_string(&unit.events).unwrap();
        commit_json(log, &json, &CommitFault::default()).unwrap();
    }
}

#[test]
fn record_log_scaling_commit_and_feed() {
    if std::env::var_os("RECORD_SCALE_BENCH").is_none() {
        // Smoke size only in default CI.
        let (_dir, log) = temp_log();
        seed_log(&log, 50);
        let snap = snapshot_json(&log, None).unwrap();
        assert_eq!(snap.facts.len(), 50);
        let feed = feed_json(&log, snap.at.0.saturating_sub(10)).unwrap();
        assert_eq!(feed.events.len(), 10);
        return;
    }

    eprintln!("record scale bench (release build recommended)");
    for count in [1_000usize, 10_000, 100_000] {
        let (_dir, log) = temp_log();
        seed_log(&log, count.saturating_sub(1));
        let unit = UnitOfWork {
            events: vec![synth_event(count as u64)],
        };
        let json = serde_json::to_string(&unit.events).unwrap();
        let t0 = Instant::now();
        commit_json(&log, &json, &CommitFault::default()).unwrap();
        let commit_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let t1 = Instant::now();
        let _ = snapshot_json(&log, None).unwrap();
        let snap_ms = t1.elapsed().as_secs_f64() * 1000.0;

        let since = (count as u64).saturating_sub(10);
        let t2 = Instant::now();
        let feed = feed_json(&log, since).unwrap();
        let feed_ms = t2.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(feed.events.len(), 10);

        eprintln!(
            "events={count} commit_ms={commit_ms:.2} snapshot_ms={snap_ms:.2} feed_last_10_ms={feed_ms:.2}"
        );
    }
}

#[test]
fn old_pre_slice2_fixture_loads_unchanged() {
    let fixture = include_str!("../fixtures/scenarios/pre-slice2-a-happy.jsonl");
    let (_dir, log) = temp_log();
    std::fs::write(&log, fixture).unwrap();
    let snap = snapshot_json(&log, None).unwrap();
    assert_eq!(snap.facts.len(), 2);
    assert_eq!(snap.at.0, 2);
}