//! Hard scam-gate. Applied before the decider and never learned.
//!
//! Soft cues return [`GateVerdict::Uncertain`]. That is an abstain, not a
//! hard fail: the posting is not skipped by the gate, and the model defers
//! to the operator. Hard cues and listed flag domains return
//! [`GateVerdict::Fail`] and skip.
//!
//! Cue matching and host extraction share [`super::normalize`]. Every check
//! runs on every view; a hit on any view is a hit.

use ensembly_schema::GateVerdict;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use super::normalize::{cue_views, extract_hosts, flag_matches, host_from_url_views, views};

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

fn cue_fail() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| {
        // Linear-time (regex crate). Alternation of fixed phrases, no nested stars.
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

fn fail() -> GateResult {
    GateResult {
        verdict: GateVerdict::Fail,
        reason: Some(GATE_REASON.into()),
    }
}

/// Audit a posting. Listed domains (URL host and hosts in the body) and hard
/// text cues fail. Soft cues are uncertain (abstain, not a hard fail).
///
/// Matching uses every normaliser view of the URL and body. A hit on any
/// view is a hit.
pub fn audit(url: &str, posting_text: &str, flags: &FlagList) -> GateResult {
    let blob = format!("{url} {posting_text}");
    let views = views(&blob);
    let mut hosts = host_from_url_views(url);
    for view in &views {
        hosts.extend(extract_hosts(view));
    }
    if flags
        .flags
        .iter()
        .any(|f| hosts.iter().any(|h| flag_matches(h, &f.domain)))
    {
        return fail();
    }
    let cue_views = cue_views(&blob);
    if cue_views.iter().any(|v| cue_fail().is_match(v)) {
        return fail();
    }
    if cue_views.iter().any(|v| cue_uncertain().is_match(v)) {
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

    fn flags() -> FlagList {
        FlagList::default()
    }

    fn clean_url() -> &'static str {
        "https://jobs.example.test/1"
    }

    #[test]
    fn listed_domain_fails() {
        let g = audit("https://phish.example.test/job/1", "a role", &flags());
        assert_eq!(g.verdict, GateVerdict::Fail);
        assert_eq!(g.reason.as_deref(), Some(GATE_REASON));
    }

    #[test]
    fn ordinary_https_passes() {
        let g = audit(
            "https://jobs.example.test/posting/1",
            "Rust engineer in Stockholm",
            &flags(),
        );
        assert_eq!(g.verdict, GateVerdict::Pass);
    }

    #[test]
    fn listed_domain_in_email_body_fails() {
        let g = audit(clean_url(), "contact hiring@phish.example.test", &flags());
        assert_eq!(g.verdict, GateVerdict::Fail);
        assert_eq!(g.reason.as_deref(), Some(GATE_REASON));
    }

    #[test]
    fn listed_domain_in_body_is_case_insensitive_and_strips_a_trailing_dot() {
        let g = audit(
            clean_url(),
            "Apply at PHISH.EXAMPLE.TEST. for details",
            &flags(),
        );
        assert_eq!(g.verdict, GateVerdict::Fail);
    }

    #[test]
    fn listed_domain_subdomain_in_body_fails() {
        let g = audit(
            clean_url(),
            "visit https://careers.phish.example.test/apply",
            &flags(),
        );
        assert_eq!(g.verdict, GateVerdict::Fail);
    }

    #[test]
    fn similar_but_distinct_host_does_not_match_a_flag() {
        let g = audit(
            clean_url(),
            "see notphish.example.test and phish.example.test.evil.example",
            &flags(),
        );
        assert_eq!(g.verdict, GateVerdict::Pass);
    }

    #[test]
    fn zero_width_space_inside_wire_still_fails() {
        let g = audit(clean_url(), "W\u{200b}IRE MONEY today", &flags());
        assert_eq!(g.verdict, GateVerdict::Fail);
    }

    #[test]
    fn repeated_spaces_in_hard_cue_still_fail() {
        let g = audit(clean_url(), "wire   money   today", &flags());
        assert_eq!(g.verdict, GateVerdict::Fail);
    }

    #[test]
    fn hard_cue_whitespace_variants_still_fail() {
        for text in [
            "wire\tmoney\ttoday",
            "wire\nmoney\ntoday",
            "wire\r\nmoney\r\ntoday",
            "wire\u{00a0}money\u{00a0}today", // NBSP
        ] {
            let g = audit(clean_url(), text, &flags());
            assert_eq!(g.verdict, GateVerdict::Fail, "{text:?}");
        }
    }

    #[test]
    fn lookalike_letter_in_hard_cue_still_fails() {
        // Ukrainian і (U+0456) in "wire"
        let g = audit(clean_url(), "w\u{0456}re money today", &flags());
        assert_eq!(g.verdict, GateVerdict::Fail);
    }

    #[test]
    fn soft_cue_is_uncertain_not_a_hard_fail() {
        let g = audit(clean_url(), "interviews are whatsapp only", &flags());
        assert_eq!(g.verdict, GateVerdict::Uncertain);
        assert_eq!(g.reason.as_deref(), Some("scam-gate-uncertain"));
    }

    #[test]
    fn every_separator_class_codepoint_still_fails() {
        use super::super::normalize::{
            is_non_ascii_dash, is_period_lookalike, separator_class_codepoints,
        };
        for &c in separator_class_codepoints() {
            let body = format!("wire{c}money today");
            let g = audit(clean_url(), &body, &flags());
            assert_eq!(
                g.verdict,
                GateVerdict::Fail,
                "cue still fails when U+{:04X} is substituted",
                c as u32
            );

            if is_non_ascii_dash(c) || is_period_lookalike(c) {
                let body = format!("contact phish{c}example.test");
                let g = audit(clean_url(), &body, &flags());
                assert_eq!(
                    g.verdict,
                    GateVerdict::Fail,
                    "flagged host in the body still fails when U+{:04X} is substituted",
                    c as u32
                );
                let url = format!("https://phish{c}example.test/x");
                let g = audit(&url, "a role", &flags());
                assert_eq!(
                    g.verdict,
                    GateVerdict::Fail,
                    "flagged host in the URL still fails when U+{:04X} is substituted",
                    c as u32
                );
            }
        }
    }

    #[test]
    fn every_folded_class_named_entity_still_fails() {
        use super::super::normalize::{
            html5_named_entities, is_folded_class_char, is_non_ascii_dash, is_period_lookalike,
        };
        let mut n = 0;
        for &(name, repl) in html5_named_entities() {
            if !repl.chars().any(is_folded_class_char) {
                continue;
            }
            n += 1;
            for entity in [format!("&{name};"), format!("&{name}")] {
                let body = format!("wire{entity}money today");
                let g = audit(clean_url(), &body, &flags());
                assert_eq!(
                    g.verdict,
                    GateVerdict::Fail,
                    "cue still fails when {entity} (U+{}) is substituted",
                    repl.chars()
                        .map(|c| format!("{:04X}", c as u32))
                        .collect::<Vec<_>>()
                        .join("+")
                );

                if repl
                    .chars()
                    .any(|c| is_non_ascii_dash(c) || is_period_lookalike(c) || c == '.')
                {
                    let body = format!("contact phish{entity}example.test");
                    let g = audit(clean_url(), &body, &flags());
                    assert_eq!(
                        g.verdict,
                        GateVerdict::Fail,
                        "flagged host in the body still fails when {entity} is substituted"
                    );
                    let url = format!("https://phish{entity}example.test/x");
                    let g = audit(&url, "a role", &flags());
                    assert_eq!(
                        g.verdict,
                        GateVerdict::Fail,
                        "flagged host in the URL still fails when {entity} is substituted"
                    );
                }
            }
        }
        assert!(
            n >= 20,
            "expected many folded-class named entities, got {n}"
        );
    }

    #[test]
    fn format_chars_inside_named_entities_still_fail() {
        use super::super::normalize::format_class_codepoints;
        let entity = "&ndash;";
        let entity_chars: Vec<char> = entity.chars().collect();
        let host_entity = "&period;";
        let host_chars: Vec<char> = host_entity.chars().collect();
        for c in format_class_codepoints() {
            for i in 0..=entity_chars.len() {
                let mut body = String::from("wire");
                body.extend(entity_chars[..i].iter().copied());
                body.push(c);
                body.extend(entity_chars[i..].iter().copied());
                body.push_str("money today");
                let g = audit(clean_url(), &body, &flags());
                assert_eq!(
                    g.verdict,
                    GateVerdict::Fail,
                    "cue still fails when U+{:04X} is inserted at index {i} of &ndash;",
                    c as u32
                );
            }
            for i in 0..=host_chars.len() {
                let mut mid = String::new();
                mid.extend(host_chars[..i].iter().copied());
                mid.push(c);
                mid.extend(host_chars[i..].iter().copied());
                let body = format!("contact phish{mid}example.test");
                let g = audit(clean_url(), &body, &flags());
                assert_eq!(
                    g.verdict,
                    GateVerdict::Fail,
                    "flagged host in the body still fails when U+{:04X} is inserted at index {i} of &period;",
                    c as u32
                );
            }
        }
    }

    #[test]
    fn encoding_order_combinations_still_fail() {
        // format-in-entity, entity-in-percent, percent-in-entity, double-encoded
        let cues = [
            ("format-in-entity", "wire&n\u{200b}dash;money today"),
            ("format-before-name", "wire&\u{200b}ndash;money today"),
            ("entity-in-percent", "wire%26ndash%3Bmoney today"),
            ("percent-in-entity", "wire&n%E2%80%8Bdash;money today"),
            ("double-encoded-entity", "wire&amp;ndash;money today"),
            ("double-encoded-percent", "wire%2526ndash%253Bmoney today"),
            ("numeric-no-semicolon", "wire&#8211money today"),
            ("hex-numeric-no-semicolon", "wire&#x2013money today"),
        ];
        for (name, body) in cues {
            let g = audit(clean_url(), body, &flags());
            assert_eq!(
                g.verdict,
                GateVerdict::Fail,
                "cue combination {name} should fail, body={body:?}"
            );
        }
        let hosts = [
            (
                "format-in-entity-host",
                "see phish&per\u{200b}iod;example.test",
            ),
            (
                "entity-in-percent-host",
                "see phish%26period%3Bexample.test",
            ),
            (
                "percent-in-entity-host",
                "see phish&per%E2%80%8Biod;example.test",
            ),
            (
                "double-encoded-entity-host",
                "see phish&amp;period;example.test",
            ),
        ];
        for (name, body) in hosts {
            let g = audit(clean_url(), body, &flags());
            assert_eq!(
                g.verdict,
                GateVerdict::Fail,
                "host combination {name} should fail, body={body:?}"
            );
        }
    }
}
