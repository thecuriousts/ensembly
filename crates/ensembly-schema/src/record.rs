use crate::ledger;
use crate::{Checked, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
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
pub struct StockholmTime(String);

impl StockholmTime {
    pub fn new(raw: impl Into<String>) -> Result<Self, String> {
        let s = raw.into();
        ledger::check_submit_time(&s)?;
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub source: Source,
    pub at: StockholmTime,
    pub text: TextForm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SubmitTime {
    Known { at: StockholmTime },
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
    Silent { since: StockholmTime },
    Expired { at: StockholmTime },
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
    pub stale_after: Option<StockholmTime>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope<P> {
    pub id: EventId,
    pub aggregate: AggregateId,
    pub key: IdempotencyKey,
    pub version: Version,
    pub event_time: StockholmTime,
    pub record_time: StockholmTime,
    pub source: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<Hash>,
    pub schema_v: u32,
    pub payload: P,
}

impl<P: Checked> Checked for EventEnvelope<P> {
    fn check(&self) -> Result<(), String> {
        if self.schema_v != SCHEMA_VERSION {
            return Err(format!(
                "unsupported schema_v {}; supported: {SCHEMA_VERSION}",
                self.schema_v
            ));
        }
        ledger::check_submit_time(self.event_time.as_str())?;
        ledger::check_submit_time(self.record_time.as_str())?;
        self.payload.check()?;
        Ok(())
    }
}

impl Status {
    pub fn check_invariants(&self) -> Result<(), String> {
        match self {
            Status::Confirmed { evidence } | Status::Rejected { evidence } => {
                ledger::check_submit_time(evidence.at.as_str())?;
            }
            Status::Submitted { submit } => match submit {
                SubmitTime::Known { at } => ledger::check_submit_time(at.as_str())?,
                SubmitTime::Unknown => {}
            },
            Status::Silent { since } => ledger::check_submit_time(since.as_str())?,
            Status::Expired { at } => ledger::check_submit_time(at.as_str())?,
            Status::NextStage { evidence, .. } => {
                ledger::check_submit_time(evidence.at.as_str())?;
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
    at: Option<StockholmTime>,
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
                return Err(err("expire needs StockholmTime"));
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
                return Err(err("expire needs StockholmTime"));
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

impl From<ledger::Status> for Status {
    fn from(s: ledger::Status) -> Self {
        use ledger::Status as L;
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
                since: StockholmTime::new("2026-01-01T00:00:00+01:00").expect("fixture time"),
            },
        }
    }
}

fn ledger_map_evidence(label: &str) -> Evidence {
    Evidence {
        source: Source::Human,
        at: StockholmTime::new("2026-01-01T00:00:00+01:00").expect("fixture time"),
        text: TextForm(format!("from-ledger-{label}")),
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
    use crate::{decode_line, encode_line, SCHEMA_VERSION};
    use serde_json::json;

    type LegalEdgeArgs = fn() -> (Option<Evidence>, Option<SubmitTime>, Option<StockholmTime>);
    type LegalEdge = (Status, EventKind, LegalEdgeArgs);

    const FIXTURE: &str = "fixtures/record-events-v1.jsonl";

    fn t() -> StockholmTime {
        StockholmTime::new("2026-10-26T18:59:00+01:00").unwrap()
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
    fn unknown_field_on_envelope_refused() {
        let line = r#"{"id":"01JCV1Q8Z00000000000000000","aggregate":"https://jobs.example.test/acme/1","key":"acme-1-submit","version":1,"event_time":"2026-10-26T18:59:00+01:00","record_time":"2026-10-26T18:59:00+01:00","source":"owner","schema_v":1,"payload":{"host":"laptop-1","company":"Acme Fictional"},"extra":1,"v":1}"#;
        assert!(decode_line::<EventEnvelope<RecordEventPayload>>(line).is_err());
    }

    #[test]
    fn schema_v_two_on_envelope_refused() {
        let line = r#"{"id":"01JCV1Q8Z00000000000000000","aggregate":"https://jobs.example.test/acme/1","key":"acme-1-submit","version":1,"event_time":"2026-10-26T18:59:00+01:00","record_time":"2026-10-26T18:59:00+01:00","source":"owner","schema_v":2,"payload":{"host":"laptop-1","company":"Acme Fictional"},"v":1}"#;
        let err = decode_line::<EventEnvelope<RecordEventPayload>>(line).unwrap_err();
        assert!(err.contains("schema_v 2"), "{err}");
    }

    #[test]
    fn version_two_line_refused() {
        let line =
            std::fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), FIXTURE)).unwrap();
        let first = line.lines().find(|l| !l.trim().is_empty()).unwrap();
        let bad = first.replacen("\"v\":1", "\"v\":2", 1);
        assert!(decode_line::<EventEnvelope<RecordEventPayload>>(&bad).is_err());
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

    #[test]
    fn from_ledger_status_exhaustive() {
        use ledger::Status as L;
        let _ = Status::from(L::Prepared);
        let _ = Status::from(L::Submitted);
        let _ = Status::from(L::Interview);
        let _ = Status::from(L::Rejected);
        let _ = Status::from(L::Withdrawn);
        let _ = Status::from(L::SilentClosed);
    }

    #[test]
    fn from_ledger_maps_names() {
        assert!(matches!(
            Status::from(ledger::Status::Prepared),
            Status::Intended
        ));
        assert!(matches!(
            Status::from(ledger::Status::Submitted),
            Status::Submitted { .. }
        ));
        assert!(matches!(
            Status::from(ledger::Status::Interview),
            Status::NextStage { n: 1, .. }
        ));
        assert!(matches!(
            Status::from(ledger::Status::SilentClosed),
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
            assert_eq!(d.version, SCHEMA_VERSION);
            assert_eq!(encode_line(SCHEMA_VERSION, &d.value).unwrap(), line);
            let row = RecordEventLine {
                body: d.value.clone(),
                v: crate::version_one::VersionOne,
            };
            assert_eq!(row.body.schema_v, SCHEMA_VERSION);
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
