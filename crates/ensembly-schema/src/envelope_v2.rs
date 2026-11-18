//! Optional v2 bus fields (ingress, supersedes, parsed, subjects, …).

use super::ClockInfo;
use super::{EnvelopeExt, EventId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const SUPERSEDES_MAX: usize = 64;
pub const SUBJECTS_MAX: usize = 32;
pub const SUBJECT_MAX_CHARS: usize = 200;
pub const PARSED_SUMMARY_MAX: usize = 2_000;
pub const RAW_REF_MAX_CHARS: usize = 512;
pub const INGRESS_STRING_MAX: usize = 256;
pub const PARSER_MAX_CHARS: usize = 128;

/// How the event entered the log. When `ingress` is present, `path` is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum IngressPath {
    Push,
    Reconcile,
}

/// Provider ingress metadata. `sig` holds the provider signature **header value only** — never a
/// secret, API key, or shared signing material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct IngressInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<String>,
    /// Signature header value from the provider (public wire copy), not a secret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sig_ok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recv_ts_us: Option<i64>,
    pub path: IngressPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum ParsedState {
    Recognized,
    Decoded,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ParsedInfo {
    pub state: ParsedState,
    /// Versioned parser id, e.g. `gmail-thread@1`.
    pub parser: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Prior event id(s) this line supersedes (pulse-style `superseded_by` on the old row is out of band).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Supersedes(pub Vec<EventId>);

impl Serialize for Supersedes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Supersedes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            One(EventId),
            Many(Vec<EventId>),
        }
        match Wire::deserialize(deserializer)? {
            Wire::One(id) => Ok(Supersedes(vec![id])),
            Wire::Many(ids) => Ok(Supersedes(ids)),
        }
    }
}

#[cfg(test)]
impl schemars::JsonSchema for Supersedes {
    fn schema_name() -> String {
        "Supersedes".into()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        Vec::<EventId>::json_schema(gen)
    }
}

pub fn validate_event_id_str(s: &str) -> Result<(), String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("event id is empty".into());
    }
    t.parse::<ulid::Ulid>()
        .map_err(|e| format!("invalid event id {t:?}: {e}"))?;
    Ok(())
}

pub fn validate_supersedes(s: &Supersedes) -> Result<(), String> {
    if s.0.is_empty() {
        return Err("supersedes must not be empty when present".into());
    }
    if s.0.len() > SUPERSEDES_MAX {
        return Err(format!("supersedes longer than {SUPERSEDES_MAX} ids"));
    }
    for id in &s.0 {
        validate_event_id_str(&id.0.to_string())?;
    }
    Ok(())
}

fn bound_str(field: &str, max: usize, v: &str) -> Result<(), String> {
    if v.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if v.chars().count() > max {
        return Err(format!("{field} longer than {max} chars"));
    }
    Ok(())
}

fn bound_opt_str(field: &str, max: usize, v: &Option<String>) -> Result<(), String> {
    if let Some(s) = v {
        bound_str(field, max, s)?;
    }
    Ok(())
}

pub fn validate_ingress(i: &IngressInfo) -> Result<(), String> {
    bound_opt_str("ingress.via", INGRESS_STRING_MAX, &i.via)?;
    bound_opt_str("ingress.provider", INGRESS_STRING_MAX, &i.provider)?;
    bound_opt_str("ingress.msg_id", INGRESS_STRING_MAX, &i.msg_id)?;
    bound_opt_str("ingress.sig", INGRESS_STRING_MAX, &i.sig)?;
    let _ = i.path;
    Ok(())
}

pub fn validate_parser_id(parser: &str) -> Result<(), String> {
    bound_str("parser", PARSER_MAX_CHARS, parser)?;
    let Some((name, ver)) = parser.rsplit_once('@') else {
        return Err("parser must be versioned (name@N)".into());
    };
    if name.trim().is_empty() {
        return Err("parser name is empty".into());
    }
    if ver.is_empty() || !ver.chars().all(|c| c.is_ascii_digit()) {
        return Err("parser version must be a positive integer after @".into());
    }
    Ok(())
}

pub fn validate_parsed(p: &ParsedInfo) -> Result<(), String> {
    validate_parser_id(&p.parser)?;
    bound_opt_str("parsed.type", INGRESS_STRING_MAX, &p.r#type)?;
    if let Some(summary) = &p.summary {
        if summary.chars().count() > PARSED_SUMMARY_MAX {
            return Err(format!(
                "parsed.summary longer than {PARSED_SUMMARY_MAX} chars"
            ));
        }
    }
    Ok(())
}

pub fn validate_raw_sha256(s: &str) -> Result<(), String> {
    if s.len() != 64
        || !s
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err("raw_sha256 must be 64 lowercase hex digits".into());
    }
    Ok(())
}

pub fn validate_raw_ref(s: &str) -> Result<(), String> {
    bound_str("raw_ref", RAW_REF_MAX_CHARS, s)
}

pub fn validate_subjects(subjects: &[String]) -> Result<(), String> {
    if subjects.is_empty() {
        return Err("subjects must not be empty when present".into());
    }
    if subjects.len() > SUBJECTS_MAX {
        return Err(format!("subjects longer than {SUBJECTS_MAX} entries"));
    }
    for (i, s) in subjects.iter().enumerate() {
        bound_str(&format!("subjects[{i}]"), SUBJECT_MAX_CHARS, s)?;
    }
    Ok(())
}

/// Optional v2 bus fields on [`crate::record::EventEnvelope`].
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingress: Option<IngressInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<Supersedes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parsed: Option<ParsedInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subjects: Option<Vec<String>>,
    #[cfg_attr(test, schemars(skip))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<EnvelopeExt>,
}

impl EventEnvelopeV2Optional {
    /// Event ids this line supersedes; for pulse/record readers (same role as `superseded_by` on the old row).
    pub fn supersedes_ids(&self) -> Option<&[EventId]> {
        self.supersedes.as_ref().map(|s| s.0.as_slice())
    }

    pub fn check(&self) -> Result<(), String> {
        if let Some(i) = &self.ingress {
            validate_ingress(i)?;
        }
        if let Some(s) = &self.supersedes {
            validate_supersedes(s)?;
        }
        if let Some(p) = &self.parsed {
            validate_parsed(p)?;
        }
        if let Some(r) = &self.raw_ref {
            validate_raw_ref(r)?;
        }
        if let Some(h) = &self.raw_sha256 {
            validate_raw_sha256(h)?;
        }
        if let Some(subj) = &self.subjects {
            validate_subjects(subj)?;
        }
        Ok(())
    }
}
