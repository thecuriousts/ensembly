//! Hard scam-gate. Applied before the decider and never learned.

use ensembly_schema::GateVerdict;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const GATE_REASON: &str = "scam-gate";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flag {
    pub domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlagList {
    pub version: String,
    pub flags: Vec<Flag>,
}

impl Default for FlagList {
    fn default() -> Self {
        Self {
            version: "synthetic-v1".into(),
            flags: vec![
                Flag {
                    domain: "phish.example.test".into(),
                    note: Some("synthetic fixture only".into()),
                },
                Flag {
                    domain: "wire-now.example.test".into(),
                    note: Some("synthetic fixture only".into()),
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateResult {
    pub verdict: GateVerdict,
    pub reason: Option<String>,
}

fn host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase()
}

fn cue_fail() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?:wire (?:money|funds) (?:today|now)|pay a (?:fee|deposit) to apply|crypto payment required|urgent hiring fee)\b",
        )
        .expect("gate fail regex")
    })
}

fn cue_uncertain() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| {
        Regex::new(r"(?i)\b(?:whatsapp only|telegram only for (?:hiring|interviews)|personal gmail for payroll)\b")
            .expect("gate uncertain regex")
    })
}

/// Audit a posting. Listed domains and hard text cues fail. Soft cues are uncertain.
pub fn audit(url: &str, posting_text: &str, flags: &FlagList) -> GateResult {
    let h = host(url);
    if flags
        .flags
        .iter()
        .any(|f| h == f.domain || h.ends_with(&format!(".{}", f.domain)))
    {
        return GateResult {
            verdict: GateVerdict::Fail,
            reason: Some(GATE_REASON.into()),
        };
    }
    let blob = format!("{url} {posting_text}");
    if cue_fail().is_match(&blob) {
        return GateResult {
            verdict: GateVerdict::Fail,
            reason: Some(GATE_REASON.into()),
        };
    }
    if cue_uncertain().is_match(&blob) {
        return GateResult {
            verdict: GateVerdict::Uncertain,
            reason: Some("scam-gate-uncertain".into()),
        };
    }
    GateResult {
        verdict: GateVerdict::Pass,
        reason: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_domain_fails() {
        let g = audit(
            "https://phish.example.test/job/1",
            "a role",
            &FlagList::default(),
        );
        assert_eq!(g.verdict, GateVerdict::Fail);
        assert_eq!(g.reason.as_deref(), Some(GATE_REASON));
    }

    #[test]
    fn ordinary_https_passes() {
        let g = audit(
            "https://jobs.example.test/posting/1",
            "Rust engineer in Stockholm",
            &FlagList::default(),
        );
        assert_eq!(g.verdict, GateVerdict::Pass);
    }
}
