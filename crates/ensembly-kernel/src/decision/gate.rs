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

use super::normalize::{
    dotted_labels_contain, extend_cue_views, extract_hosts, flag_matches, host_from_url_views,
    host_non_label_forms, junk_dotted_forms, views_with_canon,
};

pub const GATE_REASON: &str = "scam-gate";
/// Fail-closed reason when the fixed-point decode budget is exhausted
/// or a further `%XX` / `&name;` / `&#...` layer remains. This is a
/// hard Fail, not [`GateVerdict::Uncertain`]: Uncertain is an abstain
/// and does not skip the posting, so a budget miss would be treated as
/// safe enough to score.
pub const DEPTH_REASON: &str = "encoding-depth-exceeded";

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

fn fail_depth() -> GateResult {
    GateResult {
        verdict: GateVerdict::Fail,
        reason: Some(DEPTH_REASON.into()),
    }
}

/// Audit a posting. Listed domains (URL host and hosts in the body) and hard
/// text cues fail. Soft cues are uncertain (abstain, not a hard fail).
///
/// Matching uses every normaliser view of the URL and body. A hit on any
/// view is a hit. If the fixed-point decode budget is exhausted, the
/// verdict is Fail with [`DEPTH_REASON`], never Pass: Uncertain would
/// not skip the posting.
pub fn audit(url: &str, posting_text: &str, flags: &FlagList) -> GateResult {
    let blob = format!("{url} {posting_text}");
    let (views, canon) = views_with_canon(&blob);
    let mut hosts = host_from_url_views(url);
    for view in &views {
        hosts.extend(extract_hosts(view));
    }
    // Junk-as-separator host views of the fixed point: an unknown or
    // malformed entity between labels becomes '.' so a flag host
    // hidden as phish&zzzzz;example.test still matches. Not used for
    // the depth-residual rule.
    for src in junk_dotted_forms(&canon, &blob) {
        hosts.extend(extract_hosts(&src));
    }
    // Class-wide host view: any run of non-[a-z0-9-] characters
    // (punctuation, leftover junk, U+FFFD from overlong or invalid
    // percent sequences) becomes a single '.'. Matched as a consecutive
    // label window so a folded sentence still yields the flag.
    let host_folded = host_non_label_forms(&canon, &blob);
    if flags.flags.iter().any(|f| {
        hosts.iter().any(|h| flag_matches(h, &f.domain))
            || host_folded
                .iter()
                .any(|t| dotted_labels_contain(t, &f.domain))
    }) {
        return fail();
    }
    let cue_views = extend_cue_views(&blob, views, &canon);
    if cue_views.iter().any(|v| cue_fail().is_match(v)) {
        return fail();
    }
    if canon.depth_exceeded() {
        return fail_depth();
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

    fn percent_wrap(s: &str) -> String {
        let mut out = String::with_capacity(s.len() * 3);
        for b in s.as_bytes() {
            out.push_str(&format!("%{b:02X}"));
        }
        out
    }

    fn percent_wrap_n(s: &str, n: usize) -> String {
        let mut cur = s.to_string();
        for _ in 0..n {
            cur = percent_wrap(&cur);
        }
        cur
    }

    fn is_hard_fail(g: &super::GateResult) -> bool {
        g.verdict == GateVerdict::Fail
            && (g.reason.as_deref() == Some(GATE_REASON)
                || g.reason.as_deref() == Some(DEPTH_REASON))
    }

    #[test]
    fn exhausted_decode_budget_never_passes() {
        // Review repros: four percent-wraps of a ZWSP-split name, and
        // seven wraps of %26ndash%3B. Either the cue fires or the
        // budget is exceeded; neither may Pass.
        let four = format!("wire{}money today", percent_wrap_n("&n\u{200b}dash;", 4));
        let g = audit(clean_url(), &four, &flags());
        assert!(
            is_hard_fail(&g),
            "4 wraps of ZWSP-split ndash must not Pass, got {:?} {:?}",
            g.verdict,
            g.reason
        );
        assert_ne!(g.verdict, GateVerdict::Pass);
        let seven = format!("wire{}money today", percent_wrap_n("%26ndash%3B", 7));
        let g = audit(clean_url(), &seven, &flags());
        assert!(
            is_hard_fail(&g),
            "7 wraps of %26ndash%3B must not Pass, got {:?} {:?}",
            g.verdict,
            g.reason
        );
        assert_ne!(g.verdict, GateVerdict::Pass);
        // Deep wraps that outrun the cap must still be a hard fail. The
        // leftover `%XX` layers may fire encoding-depth-exceeded, or the
        // junk-as-separator view may recover the cue; neither may Pass.
        let deep = format!("wire{}money today", percent_wrap_n("&ndash;", 12));
        let g = audit(clean_url(), &deep, &flags());
        assert!(
            is_hard_fail(&g),
            "12 wraps must not Pass, got {:?} {:?}",
            g.verdict,
            g.reason
        );
    }

    #[test]
    fn wrapped_entity_depths_never_pass() {
        for depth in 0..=12 {
            for (kind, inner) in [("zwsp-split", "&n\u{200b}dash;"), ("plain", "&ndash;")] {
                let body = format!("wire{}money today", percent_wrap_n(inner, depth));
                let g = audit(clean_url(), &body, &flags());
                assert_ne!(
                    g.verdict,
                    GateVerdict::Pass,
                    "{kind} wrap depth {depth} must not Pass, reason={:?}",
                    g.reason
                );
                assert!(
                    is_hard_fail(&g),
                    "{kind} wrap depth {depth} must Fail on the cue or as encoding-depth-exceeded, got {:?} {:?}",
                    g.verdict,
                    g.reason
                );
            }
        }
    }

    /// Malformed or unknown escapes used as separators between cue
    /// words or flag-host labels.
    fn malformed_escape_templates() -> &'static [&'static str] {
        &[
            "&#xD800;",
            "&#99999999;",
            "&#xZZ;",
            "%2",
            "%G1",
            "&zzzzz;",
            "&NDASH;",
        ]
    }

    #[test]
    fn malformed_escapes_as_separators_still_fail() {
        let templates = malformed_escape_templates();
        let repros = [
            "wire&#xD800;money today",
            "wire&#99999999;money today",
            "wire&#xZZ;money today",
            "wire%2money today",
            "wire%G1money today",
            "wire&zzzzz; money today",
            "wire&NDASH;money today",
        ];
        for body in repros {
            let g = audit(clean_url(), body, &flags());
            assert_eq!(
                g.verdict,
                GateVerdict::Fail,
                "cue still fails on malformed-escape repro {body:?}, got {:?} {:?}",
                g.verdict,
                g.reason
            );
            assert_eq!(g.reason.as_deref(), Some(GATE_REASON));
        }
        let g = audit(clean_url(), "see phish&zzzzz;example.test", &flags());
        assert_eq!(
            g.verdict,
            GateVerdict::Fail,
            "unknown entity between host labels must fail, got {:?} {:?}",
            g.verdict,
            g.reason
        );

        for tmpl in templates {
            let body = format!("wire{tmpl}money today");
            let g = audit(clean_url(), &body, &flags());
            assert_eq!(
                g.verdict,
                GateVerdict::Fail,
                "cue still fails when {tmpl} is inserted between words, reason={:?}",
                g.reason
            );
            let body = format!("see phish{tmpl}example.test");
            let g = audit(clean_url(), &body, &flags());
            assert_eq!(
                g.verdict,
                GateVerdict::Fail,
                "flagged host in the body still fails when {tmpl} is inserted between labels, reason={:?}",
                g.reason
            );
            let url = format!("https://phish{tmpl}example.test/x");
            let g = audit(&url, "a role", &flags());
            assert_eq!(
                g.verdict,
                GateVerdict::Fail,
                "flagged host in the URL still fails when {tmpl} is inserted between labels, reason={:?}",
                g.reason
            );
        }
    }

    #[test]
    fn junk_as_separator_does_not_flag_ordinary_text() {
        for body in [
            "R&D engineer in Stockholm",
            "experience with AT&T systems",
            "Q&A session after the interview",
            "50% off relocation bonus",
            "a && b are both required skills",
            "<p>R&amp;D engineer in Stockholm</p>",
            "wireless money today",
            "rewire money today",
        ] {
            let g = audit(clean_url(), body, &flags());
            assert_eq!(
                g.verdict,
                GateVerdict::Pass,
                "ordinary text must still Pass, body={body:?} got {:?} {:?}",
                g.verdict,
                g.reason
            );
        }
        let g = audit(
            "https://jobs.example.test/posting/1?utm_source=board&utm_medium=email%20list",
            "staff engineer, rust and systems",
            &flags(),
        );
        assert_eq!(
            g.verdict,
            GateVerdict::Pass,
            "utm %20 URL must still Pass, got {:?} {:?}",
            g.verdict,
            g.reason
        );
    }

    fn spacing_variants(left: &str, mid: &str, right: &str) -> [String; 4] {
        [
            format!("{left}{mid}{right}"),
            format!("{left} {mid}{right}"),
            format!("{left}{mid} {right}"),
            format!("{left} {mid} {right}"),
        ]
    }

    fn punctuation_samples() -> Vec<char> {
        let mut chars: Vec<char> = ('\u{21}'..='\u{7e}')
            .filter(|c| !c.is_ascii_alphanumeric())
            .collect();
        // Sample of Unicode punctuation, symbols, separators, format,
        // and the replacement character.
        chars.extend([
            '\u{00A0}', // nbsp
            '\u{00A7}', // section
            '\u{00A9}', // copyright
            '\u{00AD}', // soft hyphen
            '\u{00BF}', // inverted question
            '\u{2013}', // en dash
            '\u{2014}', // em dash
            '\u{2018}', // left quote
            '\u{2026}', // ellipsis
            '\u{2003}', // em space
            '\u{200B}', // zwsp
            '\u{2212}', // minus
            '\u{FEFF}', // bom
            '\u{FFFD}', // replacement
        ]);
        chars
    }

    #[test]
    fn bare_ampersand_and_overlong_utf8_still_fail() {
        // Review repros: a bare `&` used as a cue separator must Fail
        // in every spacing variant. These used to Pass because `&`
        // followed by a space is not an entity-shaped token.
        for body in [
            "wire& money today",
            "wire&money today",
            "wire &money today",
            "wire & money today",
        ] {
            let g = audit(clean_url(), body, &flags());
            assert_eq!(
                g.verdict,
                GateVerdict::Fail,
                "bare-ampersand cue must Fail, body={body:?} got {:?} {:?}",
                g.verdict,
                g.reason
            );
            assert_eq!(g.reason.as_deref(), Some(GATE_REASON));
        }
        // Overlong UTF-8 NUL (`%c0%80`) between host labels must Fail,
        // same as a `%2e` dot encode.
        let g = audit(clean_url(), "see phish%c0%80example.test", &flags());
        assert_eq!(
            g.verdict,
            GateVerdict::Fail,
            "overlong UTF-8 between host labels must Fail, got {:?} {:?}",
            g.verdict,
            g.reason
        );
        let g = audit(clean_url(), "see phish%2eexample.test", &flags());
        assert_eq!(g.verdict, GateVerdict::Fail);
        let g = audit(clean_url(), "see phish%2Eexample.test", &flags());
        assert_eq!(g.verdict, GateVerdict::Fail);
    }

    #[test]
    fn punctuation_as_separator_still_fails() {
        for c in punctuation_samples() {
            for body in spacing_variants("wire", &c.to_string(), "money today") {
                let g = audit(clean_url(), &body, &flags());
                assert_eq!(
                    g.verdict,
                    GateVerdict::Fail,
                    "cue still fails when U+{:04X} is inserted, body={body:?} reason={:?}",
                    c as u32,
                    g.reason
                );
            }
            // ASCII '-' is a valid host-label character and must not
            // become '.'; the hyphenated lookalike stays distinct.
            if c == '-' {
                for body in spacing_variants("phish", "-", "example.test") {
                    // "phish-example.test" (no extra spaces) is the
                    // unflagged hyphenated domain. Spaced variants
                    // (`phish - example.test`) are separators.
                    if body == "phish-example.test" {
                        let g = audit(clean_url(), &body, &flags());
                        assert_eq!(
                            g.verdict,
                            GateVerdict::Pass,
                            "ASCII hyphenated unflagged host must still Pass, body={body:?}"
                        );
                        continue;
                    }
                    let g = audit(clean_url(), &format!("see {body}"), &flags());
                    assert_eq!(
                        g.verdict,
                        GateVerdict::Fail,
                        "spaced hyphen between host labels must Fail, body={body:?} reason={:?}",
                        g.reason
                    );
                }
                continue;
            }
            for host in spacing_variants("phish", &c.to_string(), "example.test") {
                let body = format!("see {host}");
                let g = audit(clean_url(), &body, &flags());
                assert_eq!(
                    g.verdict,
                    GateVerdict::Fail,
                    "flagged host in the body still fails when U+{:04X} is inserted, body={body:?} reason={:?}",
                    c as u32,
                    g.reason
                );
                let url = format!("https://{host}/x");
                // Spaces and URL syntax (`/ ? # @ :`) break the
                // authority, so only the body case is required there.
                // Other no-space insertions in the authority must Fail.
                let url_syntax = ['/', '?', '#', '@', ':', '[', ']'];
                if !host
                    .chars()
                    .any(|ch| ch.is_whitespace() || url_syntax.contains(&ch))
                {
                    let g = audit(&url, "a role", &flags());
                    assert_eq!(
                        g.verdict,
                        GateVerdict::Fail,
                        "flagged host in the URL still fails when U+{:04X} is inserted, url={url:?} reason={:?}",
                        c as u32,
                        g.reason
                    );
                }
            }
        }
    }

    #[test]
    fn one_megabyte_work_is_proportional_to_input() {
        use crate::decision::normalize::{byte_work, reset_byte_work};

        let n = 1024 * 1024;
        let url = "https://jobs.example.test/posting/1";
        let flags = flags();
        // 128 work units per input byte is ample for a handful of
        // linear views and a capped (CANON_PASSES) decode loop.
        let bound = |len: usize| (len as u64).saturating_mul(128).saturating_add(64 * 1024);

        reset_byte_work();
        let _ = audit(url, &"x".repeat(n), &flags);
        let plain = byte_work();
        assert!(
            plain <= bound(n + url.len()),
            "plain 1 MB work {plain} exceeded bound {}",
            bound(n + url.len())
        );

        let mut nested = String::with_capacity(n);
        nested.push_str("&amp;");
        while nested.len() < n {
            nested.push_str("%26amp%3B");
        }
        nested.truncate(n);
        reset_byte_work();
        let _ = audit(url, &nested, &flags);
        let work = byte_work();
        assert!(
            work <= bound(n + url.len()),
            "nested 1 MB work {work} exceeded bound {}",
            bound(n + url.len())
        );
    }
}
