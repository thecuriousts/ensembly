//! Hard scam-gate. Applied before the decider and never learned.
//!
//! Soft cues return [`GateVerdict::Uncertain`]. That is an abstain, not a
//! hard fail: the posting is not skipped by the gate, and the model defers
//! to the operator. Hard cues and listed flag domains return
//! [`GateVerdict::Fail`] and skip.

use ensembly_schema::GateVerdict;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

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

/// Canonical host: lowercase, no brackets, no trailing dots, no trailing port.
fn canonicalize_host(raw: &str) -> String {
    let mut h = raw
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    while h.ends_with('.') {
        h.pop();
    }
    if let Some((name, port)) = h.rsplit_once(':') {
        if !name.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            h = name.to_string();
        }
    }
    h
}

fn host(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let authority = rest.split('/').next().unwrap_or("");
    let after_user = authority.split('@').next_back().unwrap_or("");
    canonicalize_host(after_user)
}

fn flag_matches(candidate: &str, flag: &str) -> bool {
    let h = canonicalize_host(candidate);
    let f = canonicalize_host(flag);
    !h.is_empty() && !f.is_empty() && (h == f || h.ends_with(&format!(".{f}")))
}

fn host_extract() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| {
        Regex::new(
            r"(?ix)
            (?:https?://|ftps?://)(?:[^/@\s]+@)?([^/\s:?\#]+)
            | \b[A-Z0-9._%+\-]+@([^/\s:?\#]+)
            | \b((?:[A-Z0-9](?:[A-Z0-9\-]{0,61}[A-Z0-9])?\.)+[A-Z]{2,})\.?
            ",
        )
        .expect("host extract regex")
    })
}

/// Hosts mentioned in `text`: URL hosts, email hosts, and bare domain tokens.
fn extract_hosts(text: &str) -> Vec<String> {
    host_extract()
        .captures_iter(text)
        .filter_map(|caps| {
            caps.get(1)
                .or_else(|| caps.get(2))
                .or_else(|| caps.get(3))
                .map(|m| canonicalize_host(m.as_str()))
        })
        .filter(|h| !h.is_empty())
        .collect()
}

/// Map common Latin lookalikes (Cyrillic, Greek, and a few specials) after lowercase.
fn map_lookalike(c: char) -> char {
    match c {
        '\u{0430}' | '\u{03B1}' => 'a',                           // а, α
        '\u{0441}' | '\u{03F2}' => 'c',                           // с, ϲ
        '\u{0501}' => 'd',                                        // ԁ
        '\u{0435}' | '\u{03B5}' | '\u{03F5}' => 'e',              // е, ε, ϵ
        '\u{04BB}' => 'h',                                        // һ
        '\u{0456}' | '\u{03B9}' | '\u{0131}' | '\u{04CF}' => 'i', // і, ι, ı, ӏ
        '\u{043A}' | '\u{03BA}' => 'k',                           // к, κ
        '\u{043C}' => 'm',                                        // м
        '\u{03B7}' => 'n',                                        // η
        '\u{043E}' | '\u{03BF}' => 'o',                           // о, ο
        '\u{0440}' | '\u{03C1}' => 'p',                           // р, ρ
        '\u{051B}' => 'q',                                        // ԛ
        '\u{0455}' => 's',                                        // ѕ
        '\u{0442}' | '\u{03C4}' => 't',                           // т, τ
        '\u{03C5}' => 'u',                                        // υ
        '\u{03BD}' => 'v',                                        // ν
        '\u{051D}' => 'w',                                        // ԝ
        '\u{0445}' | '\u{03C7}' => 'x',                           // х, χ
        '\u{0443}' => 'y',                                        // у
        _ => c,
    }
}

fn is_format_or_zero_width(c: char) -> bool {
    // Cf / zero-width / format characters. Do not use `is_control()` alone:
    // on rustc 1.99, U+200B is neither control nor whitespace.
    matches!(
        c,
        '\u{00AD}' // soft hyphen
            | '\u{034F}' // combining grapheme joiner
            | '\u{061C}' // arabic letter mark
            | '\u{180E}' // mongolian vowel separator
            | '\u{200B}'..='\u{200F}' // ZWSP, ZWNJ, ZWJ, LRM, RLM
            | '\u{202A}'..='\u{202E}' // bidi embeddings
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FEFF}' // BOM / ZWNBSP
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{110BD}'
            | '\u{110CD}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0001}'
            | '\u{E0020}'..='\u{E007F}'
    ) || ((c as u32) >= 0xE0100 && (c as u32) <= 0xE01EF)
        || (c.is_control() && !c.is_whitespace())
}

/// NFKC, strip format/zero-width, collapse whitespace, lowercase, map lookalikes.
fn normalize_for_cues(text: &str) -> String {
    let nfkc: String = text.nfkc().collect();
    let stripped: String = nfkc
        .chars()
        .filter(|c| !is_format_or_zero_width(*c))
        .collect();
    let lowered = stripped.to_lowercase();
    let mapped: String = lowered.chars().map(map_lookalike).collect();
    collapse_whitespace(&mapped)
}

fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            in_space = false;
            out.push(c);
        }
    }
    out.trim().to_string()
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

fn fail() -> GateResult {
    GateResult {
        verdict: GateVerdict::Fail,
        reason: Some(GATE_REASON.into()),
    }
}

/// Audit a posting. Listed domains (URL host and hosts in the body) and hard
/// text cues fail. Soft cues are uncertain (abstain, not a hard fail).
pub fn audit(url: &str, posting_text: &str, flags: &FlagList) -> GateResult {
    let blob = format!("{url} {posting_text}");
    let mut hosts = vec![host(url)];
    hosts.extend(extract_hosts(&blob));
    if flags
        .flags
        .iter()
        .any(|f| hosts.iter().any(|h| flag_matches(h, &f.domain)))
    {
        return fail();
    }
    let normalised = normalize_for_cues(&blob);
    if cue_fail().is_match(&normalised) {
        return fail();
    }
    if cue_uncertain().is_match(&normalised) {
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
}
