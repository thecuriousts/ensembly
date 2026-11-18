use crate::hlc::HlcWire;
use crate::url;
use crate::{Checked, SCHEMA_VERSION, SCHEMA_VERSION_LEGACY};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(pub ulid::Ulid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct AggregateId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct IdempotencyKey(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct Version(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct Rfc3339Time(String);

impl Rfc3339Time {
    pub fn new(raw: impl Into<String>) -> Result<Self, String> {
        let s = raw.into();
        url::check_submit_time(&s)?;
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct TextForm(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hash(#[serde(with = "hash_hex")] pub [u8; 32]);

#[cfg(test)]
impl schemars::JsonSchema for EventId {
    fn schema_name() -> String {
        "EventId".into()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(gen)
    }
}

#[cfg(test)]
impl schemars::JsonSchema for Hash {
    fn schema_name() -> String {
        "Hash".into()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(gen)
    }
}

mod hash_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8; 32], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut hex = String::with_capacity(64);
        for b in bytes {
            use std::fmt::Write;
            write!(hex, "{b:02x}").map_err(serde::ser::Error::custom)?;
        }
        s.serialize_str(&hex)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(d)?;
        if s.len() != 64 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(serde::de::Error::custom("hash must be 64 hex digits"));
        }
        let mut out = [0u8; 32];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            out[i] = u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    Owner,
    CollabFinder,
    CosCopy,
    Gmail,
    Linear,
    Ats,
    Human,
    Document,
    LegacyUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub source: Source,
    pub at: Rfc3339Time,
    pub text: TextForm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SubmitTime {
    Known { at: Rfc3339Time },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum Status {
    Intended,
    Submitted { submit: SubmitTime },
    Confirmed { evidence: Evidence },
    Rejected { evidence: Evidence },
    NextStage { n: u8, evidence: Evidence },
    Silent { since: Rfc3339Time },
    Expired { at: Rfc3339Time },
    Withdrawn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Aggregate {
    pub id: AggregateId,
    pub status: Status,
    pub version: Version,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_after: Option<Rfc3339Time>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum EventKind {
    Submit,
    Confirm,
    Reject,
    Advance,
    Silence,
    Withdraw,
    Expire,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum ConflictState {
    Detected,
    Explained,
    ResolvedByOwner,
    WaivedByHuman,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IllegalTransition {
    pub from: Status,
    pub kind: EventKind,
    pub reason: &'static str,
}

impl fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "illegal {:?} from {:?}: {}",
            self.kind, self.from, self.reason
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum EventTimePrec {
    Us,
    Ms,
    S,
    Min,
    Day,
    Unknown,
}

/// Wall-clock sync metadata on a v2 line. `src` is an open string
/// (`chrony`, `sntp`, `peer:<alias>`, `none`, `roughtime`, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ClockInfo {
    pub off_us: i64,
    pub err_us: u64,
    pub src: String,
    pub age_s: u32,
}

/// Optional v2 bus fields — extend **here** (with `EventEnvelope`) before ship.
/// Fleet field list still open (`ingress`, `settle`, `supersedes`, …).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(default)]
pub struct EventEnvelopeV2Optional {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<ClockInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<EventId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[cfg_attr(test, schemars(skip))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<EnvelopeExt>,
}

/// Optional envelope extension bag; unknown keys are preserved and ignored on read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EnvelopeExt(#[serde(with = "ext_map")] pub BTreeMap<String, serde_json::Value>);

mod ext_map {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S>(map: &BTreeMap<String, serde_json::Value>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        map.serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<BTreeMap<String, serde_json::Value>, D::Error>
    where
        D: Deserializer<'de>,
    {
        BTreeMap::deserialize(d)
    }
}

/// Record event line (schema_v 1 legacy or schema_v 2 bus envelope).
///
/// Top-level `deny_unknown_fields` lives only on this struct so new v2 keys land in
/// one place (`EventEnvelope` + `EventEnvelopeV2Optional`) without a schema_v 3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope<P> {
    pub id: EventId,
    pub aggregate: AggregateId,
    pub key: IdempotencyKey,
    pub version: Version,
    pub event_time: Rfc3339Time,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_time_prec: Option<EventTimePrec>,
    pub record_time: Rfc3339Time,
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<Hash>,
    pub schema_v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hlc: Option<HlcWire>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writer: Option<String>,
    #[serde(flatten, default)]
    pub v2_optional: EventEnvelopeV2Optional,
    pub payload: P,
}

impl<P> EventEnvelope<P> {
    #[allow(clippy::too_many_arguments)]
    pub fn with_payload(
        id: EventId,
        aggregate: AggregateId,
        key: IdempotencyKey,
        version: Version,
        event_time: Rfc3339Time,
        record_time: Rfc3339Time,
        source: Source,
        schema_v: u32,
        payload: P,
    ) -> Self {
        Self {
            id,
            aggregate,
            key,
            version,
            event_time,
            record_time,
            source,
            schema_v,
            payload,
            event_time_prec: None,
            prev_hash: None,
            node: None,
            seq: None,
            hlc: None,
            writer: None,
            v2_optional: EventEnvelopeV2Optional::default(),
        }
    }
}

pub fn is_host_alias(s: &str) -> bool {
    crate::identity::is_host_alias(s)
}

impl<P> crate::wire::WireDecode for EventEnvelope<P>
where
    P: Checked + for<'de> serde::Deserialize<'de>,
{
    fn max_wire_version() -> u32 {
        crate::SCHEMA_VERSION
    }
}

impl<P: Checked> Checked for EventEnvelope<P> {
    fn check(&self) -> Result<(), String> {
        match self.schema_v {
            SCHEMA_VERSION_LEGACY | SCHEMA_VERSION => {}
            other => {
                return Err(format!(
                    "unsupported schema_v {other}; supported: {SCHEMA_VERSION_LEGACY} or {SCHEMA_VERSION}"
                ));
            }
        }
        crate::idempotency::clamp_key(self.key.0.as_str())?;
        if self.schema_v == SCHEMA_VERSION {
            check_utc_z_time(self.event_time.as_str(), "event_time")?;
            check_utc_z_time(self.record_time.as_str(), "record_time")?;
            let Some(prec) = self.event_time_prec else {
                return Err("event_time_prec is required on schema_v 2".into());
            };
            if prec == EventTimePrec::Unknown {
                return Err("event_time_prec must not be unknown on schema_v 2 writes".into());
            }
            let node = self
                .node
                .as_deref()
                .ok_or_else(|| "node is required on schema_v 2".to_string())?;
            if !is_host_alias(node) {
                return Err("node is not a host alias".into());
            }
            self.seq
                .ok_or_else(|| "seq is required on schema_v 2".to_string())?;
            self.hlc
                .ok_or_else(|| "hlc is required on schema_v 2".to_string())?;
            let writer = self
                .writer
                .as_deref()
                .ok_or_else(|| "writer is required on schema_v 2".to_string())?;
            crate::identity::validate_writer_identity(writer)?;
            self.v2_optional.depth.unwrap_or(0);
        } else {
            url::check_submit_time(self.event_time.as_str())?;
            url::check_submit_time(self.record_time.as_str())?;
        }
        self.payload.check()?;
        Ok(())
    }
}

/// RFC3339 UTC with `Z` suffix; `record_time` must carry exactly six fraction digits on v2.
pub fn check_utc_z_time(t: &str, field: &str) -> Result<(), String> {
    let dt = chrono::DateTime::parse_from_rfc3339(t)
        .map_err(|e| format!("{field} {t:?} is not RFC 3339: {e}"))?;
    if !t.ends_with('Z') && !t.ends_with('z') {
        return Err(format!("{field} must be UTC Z"));
    }
    if field == "record_time" {
        let frac = t.split('.').nth(1).and_then(|s| s.strip_suffix('Z'));
        match frac {
            Some(f) if f.len() == 6 => {}
            _ => return Err("record_time must have exactly six fraction digits".into()),
        }
    }
    let _ = dt;
    Ok(())
}

impl Status {
    pub fn check_invariants(&self) -> Result<(), String> {
        match self {
            Status::Confirmed { evidence } | Status::Rejected { evidence } => {
                url::check_submit_time(evidence.at.as_str())?;
            }
            Status::Submitted { submit } => match submit {
                SubmitTime::Known { at } => url::check_submit_time(at.as_str())?,
                SubmitTime::Unknown => {}
            },
            Status::Silent { since } => url::check_submit_time(since.as_str())?,
            Status::Expired { at } => url::check_submit_time(at.as_str())?,
            Status::NextStage { evidence, .. } => {
                url::check_submit_time(evidence.at.as_str())?;
            }
            Status::Intended | Status::Withdrawn => {}
        }
        Ok(())
    }
}

impl Checked for Status {
    fn check(&self) -> Result<(), String> {
        self.check_invariants()
    }
}

pub fn transition(
    from: Status,
    kind: EventKind,
    ev: Option<Evidence>,
    submit: Option<SubmitTime>,
    at: Option<Rfc3339Time>,
) -> Result<Status, IllegalTransition> {
    use EventKind::*;
    let err = |reason: &'static str| IllegalTransition {
        from: from.clone(),
        kind,
        reason,
    };
    match (&from, kind) {
        (Status::Intended, Submit) => {
            let Some(sub) = submit else {
                return Err(err("submit needs SubmitTime"));
            };
            Ok(Status::Submitted { submit: sub })
        }
        (Status::Intended, Withdraw) => Ok(Status::Withdrawn),
        (Status::Intended, Expire) => {
            let Some(t) = at else {
                return Err(err("expire needs Rfc3339Time"));
            };
            Ok(Status::Expired { at: t })
        }
        (Status::Submitted { .. }, Confirm) => {
            let Some(e) = ev else {
                return Err(err("confirm needs Evidence"));
            };
            Ok(Status::Confirmed { evidence: e })
        }
        (Status::Submitted { .. }, Reject) => {
            let Some(e) = ev else {
                return Err(err("reject needs Evidence"));
            };
            Ok(Status::Rejected { evidence: e })
        }
        (Status::Submitted { .. }, Advance) => {
            let Some(e) = ev else {
                return Err(err("advance needs Evidence"));
            };
            Ok(Status::NextStage { n: 1, evidence: e })
        }
        (Status::Submitted { .. }, Silence) => {
            let Some(t) = at else {
                return Err(err("silence needs since time"));
            };
            Ok(Status::Silent { since: t })
        }
        (Status::Submitted { .. }, Withdraw) => Ok(Status::Withdrawn),
        (Status::Submitted { .. }, Expire) => {
            let Some(t) = at else {
                return Err(err("expire needs Rfc3339Time"));
            };
            Ok(Status::Expired { at: t })
        }
        (Status::Confirmed { .. }, Advance) => {
            let Some(e) = ev else {
                return Err(err("advance needs Evidence"));
            };
            Ok(Status::NextStage { n: 1, evidence: e })
        }
        (Status::Confirmed { .. }, Reject) => {
            let Some(e) = ev else {
                return Err(err("reject needs Evidence"));
            };
            Ok(Status::Rejected { evidence: e })
        }
        (Status::Confirmed { .. }, Silence) => {
            let Some(t) = at else {
                return Err(err("silence needs since time"));
            };
            Ok(Status::Silent { since: t })
        }
        (Status::Confirmed { .. }, Withdraw) => Ok(Status::Withdrawn),
        (Status::NextStage { n, .. }, Advance) => {
            let Some(e) = ev else {
                return Err(err("advance needs Evidence"));
            };
            Ok(Status::NextStage {
                n: n.saturating_add(1),
                evidence: e,
            })
        }
        (Status::NextStage { .. }, Confirm) => {
            let Some(e) = ev else {
                return Err(err("confirm needs Evidence"));
            };
            Ok(Status::Confirmed { evidence: e })
        }
        (Status::NextStage { .. }, Reject) => {
            let Some(e) = ev else {
                return Err(err("reject needs Evidence"));
            };
            Ok(Status::Rejected { evidence: e })
        }
        (Status::NextStage { .. }, Silence) => {
            let Some(t) = at else {
                return Err(err("silence needs since time"));
            };
            Ok(Status::Silent { since: t })
        }
        (Status::NextStage { .. }, Withdraw) => Ok(Status::Withdrawn),
        (
            Status::Rejected { .. }
            | Status::Silent { .. }
            | Status::Expired { .. }
            | Status::Withdrawn,
            _,
        ) => Err(err("terminal status")),
        (Status::Intended, Confirm | Reject | Advance | Silence) => Err(err("never acted")),
        (Status::Submitted { .. }, Submit) => Err(err("already submitted")),
        (Status::Confirmed { .. }, Submit | Confirm | Expire) => Err(err("already past submit")),
        (Status::NextStage { .. }, Submit | Expire) => Err(err("already past submit")),
    }
}

#[cfg(feature = "opportunity-finder")]
impl From<crate::ledger::Status> for Status {
    fn from(s: crate::ledger::Status) -> Self {
        use crate::ledger::Status as L;
        fn ledger_map_evidence(label: &str) -> Evidence {
            Evidence {
                source: Source::LegacyUnknown,
                at: Rfc3339Time::new("2026-01-01T00:00:00+01:00").expect("fixture time"),
                text: TextForm(format!("from-ledger-{label}")),
            }
        }
        match s {
            L::Prepared => Status::Intended,
            L::Submitted => Status::Submitted {
                submit: SubmitTime::Unknown,
            },
            L::Interview => Status::NextStage {
                n: 1,
                evidence: ledger_map_evidence("interview-stage"),
            },
            L::Rejected => Status::Rejected {
                evidence: ledger_map_evidence("rejected"),
            },
            L::Withdrawn => Status::Withdrawn,
            L::SilentClosed => Status::Silent {
                since: Rfc3339Time::new("2026-01-01T00:00:00+01:00").expect("fixture time"),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RecordEventPayload {
    pub host: String,
    pub company: String,
}

impl Checked for RecordEventPayload {
    fn check(&self) -> Result<(), String> {
        if self.host.trim().is_empty() {
            return Err("host must not be empty".into());
        }
        if self.company.trim().is_empty() {
            return Err("company must not be empty".into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecordEventLine {
    #[serde(flatten)]
    body: EventEnvelope<RecordEventPayload>,
    v: crate::version_one::VersionOne,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_line, encode_line};
    use serde_json::json;

    type LegalEdgeArgs = fn() -> (Option<Evidence>, Option<SubmitTime>, Option<Rfc3339Time>);
    type LegalEdge = (Status, EventKind, LegalEdgeArgs);

    const FIXTURE: &str = "fixtures/record-events-v1.jsonl";

    fn t() -> Rfc3339Time {
        Rfc3339Time::new("2026-10-26T18:59:00+01:00").unwrap()
    }

    fn ev(label: &str) -> Evidence {
        Evidence {
            source: Source::Gmail,
            at: t(),
            text: TextForm(label.into()),
        }
    }

    fn sample_status(status: Status) -> Status {
        status.check_invariants().unwrap();
        status
    }

    #[test]
    fn unknown_top_level_field_refused() {
        let line = r#"{"id":"01JCV1Q8Z00000000000000000","aggregate":"https://jobs.example.test/acme/1","key":"acme-1-submit","version":1,"event_time":"2026-10-26T18:59:00+01:00","record_time":"2026-10-26T18:59:00+01:00","source":"owner","schema_v":1,"payload":{"host":"laptop-1","company":"Acme Fictional"},"extra":1,"v":1}"#;
        assert!(decode_line::<EventEnvelope<RecordEventPayload>>(line).is_err());
    }

    #[test]
    fn schema_v_two_body_without_bus_fields_fails_check() {
        let line = r#"{"id":"01JCV1Q8Z00000000000000000","aggregate":"https://jobs.example.test/acme/1","key":"acme-1-submit","version":1,"event_time":"2026-10-26T18:59:00Z","record_time":"2026-10-26T18:59:00.000000Z","source":"owner","schema_v":2,"payload":{"host":"laptop-1","company":"Acme Fictional"},"v":1}"#;
        assert!(decode_line::<EventEnvelope<RecordEventPayload>>(line).is_err());
    }

    #[test]
    fn version_two_wire_reads_v1_body() {
        let line =
            std::fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), FIXTURE)).unwrap();
        let first = line.lines().find(|l| !l.trim().is_empty()).unwrap();
        let wire2 = first.replacen("\"v\":1", "\"v\":2", 1);
        assert!(decode_line::<EventEnvelope<RecordEventPayload>>(&wire2).is_ok());
    }

    #[test]
    fn confirmed_rejected_without_evidence_refused_in_json() {
        let bad_confirmed = json!({"status":"confirmed"});
        assert!(serde_json::from_value::<Status>(bad_confirmed).is_err());
        let bad_rejected = json!({"status":"rejected"});
        assert!(serde_json::from_value::<Status>(bad_rejected).is_err());
    }

    #[test]
    fn illegal_transitions_refused() {
        let intended = sample_status(Status::Intended);
        for k in [
            EventKind::Confirm,
            EventKind::Reject,
            EventKind::Advance,
            EventKind::Silence,
        ] {
            assert!(transition(intended.clone(), k, Some(ev("x")), None, None).is_err());
        }
        let terminal = [
            sample_status(Status::Rejected {
                evidence: ev("done"),
            }),
            sample_status(Status::Silent { since: t() }),
            sample_status(Status::Expired { at: t() }),
            sample_status(Status::Withdrawn),
        ];
        for s in terminal {
            for k in [EventKind::Submit, EventKind::Confirm, EventKind::Withdraw] {
                assert!(transition(s.clone(), k, Some(ev("x")), None, None).is_err());
            }
        }
        let submitted = sample_status(Status::Submitted {
            submit: SubmitTime::Unknown,
        });
        assert!(transition(submitted, EventKind::Confirm, None, None, None).is_err());
    }

    #[test]
    fn status_variants_round_trip() {
        let cases = [
            Status::Intended,
            Status::Submitted {
                submit: SubmitTime::Known { at: t() },
            },
            Status::Submitted {
                submit: SubmitTime::Unknown,
            },
            Status::Confirmed { evidence: ev("ok") },
            Status::Rejected { evidence: ev("no") },
            Status::NextStage {
                n: 2,
                evidence: ev("stage"),
            },
            Status::Silent { since: t() },
            Status::Expired { at: t() },
            Status::Withdrawn,
        ];
        for s in cases {
            let j = serde_json::to_value(&s).unwrap();
            let back: Status = serde_json::from_value(j).unwrap();
            assert_eq!(back, s);
        }
    }

    #[cfg(feature = "opportunity-finder")]
    #[test]
    fn from_ledger_status_exhaustive() {
        use crate::ledger::Status as L;
        let _ = Status::from(L::Prepared);
        let _ = Status::from(L::Submitted);
        let _ = Status::from(L::Interview);
        let _ = Status::from(L::Rejected);
        let _ = Status::from(L::Withdrawn);
        let _ = Status::from(L::SilentClosed);
    }

    #[cfg(feature = "opportunity-finder")]
    #[test]
    fn from_ledger_maps_names() {
        use crate::ledger::Status as L;
        assert!(matches!(Status::from(L::Prepared), Status::Intended));
        assert!(matches!(
            Status::from(L::Submitted),
            Status::Submitted { .. }
        ));
        assert!(matches!(
            Status::from(L::Interview),
            Status::NextStage { n: 1, .. }
        ));
        assert!(matches!(
            Status::from(L::SilentClosed),
            Status::Silent { .. }
        ));
    }

    #[test]
    fn legal_transition_edges_match_statechart() {
        let edges: &[LegalEdge] = &[
            (Status::Intended, EventKind::Submit, || {
                (None, Some(SubmitTime::Unknown), None)
            }),
            (Status::Intended, EventKind::Withdraw, || (None, None, None)),
            (Status::Intended, EventKind::Expire, || {
                (None, None, Some(t()))
            }),
            (
                Status::Submitted {
                    submit: SubmitTime::Unknown,
                },
                EventKind::Confirm,
                || (Some(ev("c")), None, None),
            ),
            (
                Status::Submitted {
                    submit: SubmitTime::Unknown,
                },
                EventKind::Reject,
                || (Some(ev("r")), None, None),
            ),
            (
                Status::Submitted {
                    submit: SubmitTime::Unknown,
                },
                EventKind::Advance,
                || (Some(ev("a")), None, None),
            ),
            (
                Status::Submitted {
                    submit: SubmitTime::Unknown,
                },
                EventKind::Silence,
                || (None, None, Some(t())),
            ),
            (
                Status::Submitted {
                    submit: SubmitTime::Unknown,
                },
                EventKind::Withdraw,
                || (None, None, None),
            ),
            (
                Status::Submitted {
                    submit: SubmitTime::Unknown,
                },
                EventKind::Expire,
                || (None, None, Some(t())),
            ),
            (
                Status::Confirmed {
                    evidence: ev("base"),
                },
                EventKind::Advance,
                || (Some(ev("a2")), None, None),
            ),
            (
                Status::Confirmed {
                    evidence: ev("base"),
                },
                EventKind::Reject,
                || (Some(ev("late")), None, None),
            ),
            (
                Status::Confirmed {
                    evidence: ev("base"),
                },
                EventKind::Silence,
                || (None, None, Some(t())),
            ),
            (
                Status::Confirmed {
                    evidence: ev("base"),
                },
                EventKind::Withdraw,
                || (None, None, None),
            ),
            (
                Status::NextStage {
                    n: 1,
                    evidence: ev("s1"),
                },
                EventKind::Advance,
                || (Some(ev("s2")), None, None),
            ),
            (
                Status::NextStage {
                    n: 1,
                    evidence: ev("s1"),
                },
                EventKind::Confirm,
                || (Some(ev("ok")), None, None),
            ),
            (
                Status::NextStage {
                    n: 1,
                    evidence: ev("s1"),
                },
                EventKind::Reject,
                || (Some(ev("no")), None, None),
            ),
            (
                Status::NextStage {
                    n: 1,
                    evidence: ev("s1"),
                },
                EventKind::Silence,
                || (None, None, Some(t())),
            ),
            (
                Status::NextStage {
                    n: 1,
                    evidence: ev("s1"),
                },
                EventKind::Withdraw,
                || (None, None, None),
            ),
        ];
        assert_eq!(edges.len(), 18);
        for (from, kind, args) in edges {
            let (ev, sub, at) = args();
            transition(from.clone(), *kind, ev, sub, at)
                .unwrap_or_else(|e| panic!("legal edge {:?} from {:?}: {e}", kind, from));
        }
    }

    #[test]
    fn fixture_lines_round_trip() {
        let text =
            std::fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), FIXTURE)).unwrap();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let d = decode_line::<EventEnvelope<RecordEventPayload>>(line).unwrap();
            assert_eq!(d.version, SCHEMA_VERSION_LEGACY);
            assert_eq!(encode_line(d.version, &d.value).unwrap(), line);
            let row = RecordEventLine {
                body: d.value.clone(),
                v: crate::version_one::VersionOne,
            };
            assert_eq!(row.body.schema_v, SCHEMA_VERSION_LEGACY);
            assert!(matches!(
                row.body.payload.host.as_str(),
                "laptop-1" | "laptop-2" | "mac-mini"
            ));
            let _ = row.v;
        }
    }

    #[test]
    fn checked_in_record_schema_matches_types() {
        let path = format!(
            "{}/schemas/record-event.v1.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let generated =
            serde_json::to_string_pretty(&schemars::schema_for!(RecordEventLine)).unwrap() + "\n";
        if std::env::var("WRITE_SCHEMA").ok().as_deref() == Some("1") {
            std::fs::write(&path, &generated).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), generated);
    }
}
