//! Domain-agnostic sync: offline merge, TTL expiry, clock-skew bounds.

use crate::record::{AggregateId, EventEnvelope, IdempotencyKey, Rfc3339Time, Status};
use crate::{encode_line, Checked, SCHEMA_VERSION};
use chrono::{DateTime, FixedOffset, TimeDelta};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncConfig {
    /// When [`Rfc3339Time`] is past `stale_after` on a fact, status becomes [`Status::Expired`].
    pub ttl_grace: TimeDelta,
    /// Maximum allowed gap between `event_time` and `record_time` on one envelope.
    pub max_clock_skew: TimeDelta,
    /// Reference "now" for TTL and skew checks (Europe/Stockholm RFC3339).
    pub now: Rfc3339Time,
}

impl SyncConfig {
    pub fn new(now: Rfc3339Time, ttl_grace: TimeDelta, max_clock_skew: TimeDelta) -> Self {
        Self {
            ttl_grace,
            max_clock_skew,
            now,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    ClockSkew {
        aggregate: AggregateId,
        key: IdempotencyKey,
    },
    MergeConflict {
        aggregate: AggregateId,
        key: IdempotencyKey,
    },
    TimeParse(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::ClockSkew { aggregate, key } => {
                write!(f, "clock skew on {} / {}", aggregate.0, key.0)
            }
            SyncError::MergeConflict { aggregate, key } => {
                write!(f, "merge conflict on {} / {}", aggregate.0, key.0)
            }
            SyncError::TimeParse(m) => write!(f, "time parse: {m}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeOutcome<P> {
    pub events: Vec<EventEnvelope<P>>,
}

/// Merge offline append events into the owner log after reconnect. Idempotent on same key+payload;
/// refuses conflicting payloads and excessive clock skew (no silent last-write-wins).
pub fn merge_offline<P>(
    owner: &[EventEnvelope<P>],
    offline: &[EventEnvelope<P>],
    config: &SyncConfig,
) -> Result<MergeOutcome<P>, SyncError>
where
    P: Checked + Serialize + Clone,
{
    let mut index: BTreeMap<(String, String), String> = BTreeMap::new();
    for e in owner {
        check_clock_skew(e, config)?;
        let line = encode_line(SCHEMA_VERSION, e).map_err(SyncError::TimeParse)?;
        index.insert((e.aggregate.0.clone(), e.key.0.clone()), line);
    }
    let mut merged = owner.to_vec();
    for e in offline {
        check_clock_skew(e, config)?;
        let line = encode_line(SCHEMA_VERSION, e).map_err(SyncError::TimeParse)?;
        let idem = (e.aggregate.0.clone(), e.key.0.clone());
        if let Some(prev) = index.get(&idem) {
            if prev == &line {
                continue;
            }
            return Err(SyncError::MergeConflict {
                aggregate: e.aggregate.clone(),
                key: e.key.clone(),
            });
        }
        index.insert(idem, line);
        merged.push(e.clone());
    }
    merged.sort_by(|a, b| {
        a.version
            .0
            .cmp(&b.version.0)
            .then_with(|| a.aggregate.0.cmp(&b.aggregate.0))
    });
    Ok(MergeOutcome { events: merged })
}

/// If `stale_after` is before `config.now` (plus optional grace), return `Expired`.
pub fn expire_if_stale(
    status: Status,
    stale_after: Option<Rfc3339Time>,
    config: &SyncConfig,
) -> Result<Status, SyncError> {
    let Some(deadline) = stale_after else {
        return Ok(status);
    };
    let now = parse_stockholm(config.now.as_str())?;
    let deadline = parse_stockholm(deadline.as_str())?;
    if now > deadline + config.ttl_grace {
        return Ok(Status::Expired {
            at: config.now.clone(),
        });
    }
    Ok(status)
}

fn check_clock_skew<P>(e: &EventEnvelope<P>, config: &SyncConfig) -> Result<(), SyncError> {
    let event_time = parse_stockholm(e.event_time.as_str())?;
    let record_time = parse_stockholm(e.record_time.as_str())?;
    let skew = (record_time - event_time).abs();
    if skew > config.max_clock_skew {
        return Err(SyncError::ClockSkew {
            aggregate: e.aggregate.clone(),
            key: e.key.clone(),
        });
    }
    Ok(())
}

fn parse_stockholm(raw: &str) -> Result<DateTime<FixedOffset>, SyncError> {
    DateTime::parse_from_rfc3339(raw).map_err(|e| SyncError::TimeParse(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{EventId, Source, Version};
    use serde::Deserialize;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ScenarioPayload {
        writer: String,
        note: String,
    }

    impl Checked for ScenarioPayload {
        fn check(&self) -> Result<(), String> {
            crate::validate_writer_identity(&self.writer)?;
            Ok(())
        }
    }

    fn t(s: &str) -> Rfc3339Time {
        Rfc3339Time::new(s).unwrap()
    }

    fn cfg(now: &str) -> SyncConfig {
        SyncConfig::new(t(now), TimeDelta::days(7), TimeDelta::minutes(15))
    }

    fn envelope(
        n: u64,
        writer: &str,
        aggregate: &str,
        key: &str,
        event_time: &str,
        record_time: &str,
    ) -> EventEnvelope<ScenarioPayload> {
        EventEnvelope {
            id: EventId(ulid::Ulid::from_parts(1_700_000_000 + n, n as u128)),
            aggregate: AggregateId(aggregate.into()),
            key: IdempotencyKey(key.into()),
            version: Version(n),
            event_time: t(event_time),
            record_time: t(record_time),
            source: Source::Owner,
            prev_hash: None,
            schema_v: SCHEMA_VERSION,
            payload: ScenarioPayload {
                writer: writer.into(),
                note: "sync".into(),
            },
        }
    }

    #[test]
    fn offline_append_then_merge_when_skew_ok() {
        #[derive(Deserialize)]
        struct ScenarioE {
            owner: Vec<EventEnvelope<ScenarioPayload>>,
            offline: Vec<EventEnvelope<ScenarioPayload>>,
            now: String,
        }
        let sc: ScenarioE =
            serde_json::from_str(include_str!("../fixtures/scenarios/e-offline-merge.json"))
                .expect("scenario e");
        let config = cfg(&sc.now);
        let out = merge_offline(&sc.owner, &sc.offline, &config).unwrap();
        assert!(out.events.len() > sc.owner.len());
    }

    #[test]
    fn stale_fact_becomes_expired() {
        let config = cfg("2026-11-12T12:00:00+01:00");
        let stale_after = t("2026-11-01T00:00:00+01:00");
        let next = expire_if_stale(Status::Intended, Some(stale_after), &config).unwrap();
        assert!(matches!(next, Status::Expired { .. }));
    }

    #[test]
    fn clock_skew_is_refused() {
        let config = cfg("2026-11-05T12:00:00+01:00");
        let bad = envelope(
            1,
            "laptop-1",
            "agg://example.test/sync/1",
            "skew",
            "2026-11-05T10:00:00+01:00",
            "2026-11-05T12:00:00+01:00",
        );
        assert!(matches!(
            merge_offline(&[], &[bad], &config),
            Err(SyncError::ClockSkew { .. })
        ));
    }

    #[test]
    fn merge_refuses_conflicting_payload() {
        let config = cfg("2026-11-05T12:00:00+01:00");
        let owner = vec![envelope(
            1,
            "mac-mini",
            "agg://example.test/sync/1",
            "k1",
            "2026-11-05T11:00:00+01:00",
            "2026-11-05T11:01:00+01:00",
        )];
        let mut offline = owner[0].clone();
        offline.payload.note = "different".into();
        assert!(matches!(
            merge_offline(&owner, std::slice::from_ref(&offline), &config),
            Err(SyncError::MergeConflict { .. })
        ));
    }
}
