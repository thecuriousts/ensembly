//! Adversarial regression matrix for the E3 scam-gate.
//!
//! Single source of truth for scam-gate adversarial probes: reviews of
//! 7672d81, 6d90827 and 1e85825, plus userinfo, IDN dots, leet, HTML
//! cues, spaced brackets and negative cases. Fixtures are synthetic
//! (`*.example.test` only).

use ensembly_kernel::decision::{audit, FlagList, GATE_REASON};
use ensembly_schema::GateVerdict;
use std::time::{Duration, Instant};

fn flags() -> FlagList {
    FlagList::default()
}

fn clean_url() -> &'static str {
    "https://jobs.example.test/1"
}

struct Case {
    name: &'static str,
    url: &'static str,
    body: &'static str,
    want: GateVerdict,
}

fn matrix() -> Vec<Case> {
    let fail = GateVerdict::Fail;
    let pass = GateVerdict::Pass;
    let u = clean_url();
    vec![
        // --- first review (7672d81) ---
        Case {
            name: "flag host only in email body",
            url: u,
            body: "contact hiring@phish.example.test",
            want: fail,
        },
        Case {
            name: "ZWSP inside wire",
            url: u,
            body: "W\u{200b}IRE MONEY today",
            want: fail,
        },
        Case {
            name: "repeated ASCII spaces in hard cue",
            url: u,
            body: "wire   money   today",
            want: fail,
        },
        Case {
            name: "tabs in hard cue",
            url: u,
            body: "wire\tmoney\ttoday",
            want: fail,
        },
        Case {
            name: "newlines in hard cue",
            url: u,
            body: "wire\nmoney\ntoday",
            want: fail,
        },
        Case {
            name: "NBSP in hard cue",
            url: u,
            body: "wire\u{00a0}money\u{00a0}today",
            want: fail,
        },
        Case {
            name: "flag domain on the posting URL",
            url: "https://phish.example.test/job/1",
            body: "a role",
            want: fail,
        },
        Case {
            name: "ordinary https posting passes",
            url: "https://jobs.example.test/posting/1",
            body: "Rust engineer in Stockholm",
            want: pass,
        },
        Case {
            name: "flag domain in body, case and trailing dot",
            url: u,
            body: "Apply at PHISH.EXAMPLE.TEST. for details",
            want: fail,
        },
        Case {
            name: "flag subdomain in body URL",
            url: u,
            body: "visit https://careers.phish.example.test/apply",
            want: fail,
        },
        Case {
            name: "similar but distinct hosts do not match",
            url: u,
            body: "see notphish.example.test and phish.example.test.evil.example",
            want: pass,
        },
        Case {
            name: "Ukrainian lookalike in wire",
            url: u,
            body: "w\u{0456}re money today",
            want: fail,
        },
        // --- second review (6d90827) ---
        Case {
            name: "soft hyphen between cue words",
            url: u,
            body: "wire\u{00AD}money today",
            want: fail,
        },
        Case {
            name: "BOM between cue words",
            url: u,
            body: "wire\u{FEFF}money today",
            want: fail,
        },
        Case {
            name: "ALM between cue words",
            url: u,
            body: "wire\u{061C}money today",
            want: fail,
        },
        Case {
            name: "RLM between cue words",
            url: u,
            body: "wire\u{200F}money today",
            want: fail,
        },
        Case {
            name: "ZWSP between fee and to",
            url: u,
            body: "pay a fee\u{200B}to apply",
            want: fail,
        },
        Case {
            name: "bracket-dot host in body",
            url: u,
            body: "contact phish[.]example.test",
            want: fail,
        },
        Case {
            name: "paren-dot host in body",
            url: u,
            body: "contact phish(.)example.test",
            want: fail,
        },
        Case {
            name: "brace-dot host in body",
            url: u,
            body: "contact phish{.}example.test",
            want: fail,
        },
        Case {
            name: "spelled-dot host in body",
            url: u,
            body: "contact phish dot example dot test",
            want: fail,
        },
        Case {
            name: "bracketed-dot word in body",
            url: u,
            body: "contact phish [dot] example.test",
            want: fail,
        },
        Case {
            name: "bracketed-at email in body",
            url: u,
            body: "write hiring[at]phish.example.test",
            want: fail,
        },
        Case {
            name: "spelled-at email in body",
            url: u,
            body: "write hiring at phish.example.test",
            want: fail,
        },
        Case {
            name: "percent-encoded flag host in URL",
            url: "https://ph%69sh.example.test/job",
            body: "a role",
            want: fail,
        },
        Case {
            name: "Cyrillic p lookalike in body host label",
            url: u,
            body: "see \u{0440}hish.example.test",
            want: fail,
        },
        Case {
            name: "Ukrainian i lookalike in body host label",
            url: u,
            body: "see ph\u{0456}sh.example.test",
            want: fail,
        },
        // --- HTML numeric entities and combining marks ---
        Case {
            name: "HTML decimal period entity in host",
            url: u,
            body: "see phish&#46;example.test",
            want: fail,
        },
        Case {
            name: "HTML hex period entity in host",
            url: u,
            body: "see phish&#x2e;example.test",
            want: fail,
        },
        Case {
            name: "HTML named period entity in host",
            url: u,
            body: "see phish&period;example.test",
            want: fail,
        },
        Case {
            name: "combining acute on cue letter",
            url: u,
            body: "w\u{0069}\u{0301}re money today",
            want: fail,
        },
        // --- third review (1e85825) and SHOULDs ---
        Case {
            name: "userinfo URL hides flag host after at",
            url: "https://jobs.example.test@phish.example.test/job",
            body: "a role",
            want: fail,
        },
        Case {
            name: "IDEOGRAPHIC FULL STOP U+3002 in body host",
            url: u,
            body: "contact phish\u{3002}example\u{3002}test",
            want: fail,
        },
        Case {
            name: "IDEOGRAPHIC FULL STOP U+3002 in URL host",
            url: "https://phish\u{3002}example\u{3002}test/x",
            body: "a role",
            want: fail,
        },
        Case {
            name: "HALFWIDTH IDEOGRAPHIC FULL STOP U+FF61 in body host",
            url: u,
            body: "contact phish\u{FF61}example\u{FF61}test",
            want: fail,
        },
        Case {
            name: "FULLWIDTH FULL STOP U+FF0E in body host",
            url: u,
            body: "contact phish\u{FF0E}example\u{FF0E}test",
            want: fail,
        },
        Case {
            name: "spaces inside bracket-dot",
            url: u,
            body: "contact phish [ dot ] example.test",
            want: fail,
        },
        Case {
            name: "spaces inside paren-dot",
            url: u,
            body: "contact phish ( . ) example.test",
            want: fail,
        },
        Case {
            name: "hyphen between cue words",
            url: u,
            body: "wire-money today",
            want: fail,
        },
        Case {
            name: "dotted single letters join to wire",
            url: u,
            body: "w.i.r.e money today",
            want: fail,
        },
        Case {
            name: "slash between cue words",
            url: u,
            body: "wire/money today",
            want: fail,
        },
        Case {
            name: "leet w1re money today",
            url: u,
            body: "w1re money today",
            want: fail,
        },
        Case {
            name: "HTML tags split a hard cue",
            url: u,
            body: "wire <b>money</b> today",
            want: fail,
        },
        Case {
            name: "punycode ACE label of a confusable flag host",
            url: u,
            body: "see xn--hish-f6d.example.test",
            want: fail,
        },
        Case {
            name: "punycode ACE flag host in the URL",
            url: "https://xn--hish-f6d.example.test/x",
            body: "a role",
            want: fail,
        },
        Case {
            name: "flag host in the URL path",
            url: "https://jobs.example.test/go/phish.example.test",
            body: "a role",
            want: fail,
        },
        // negatives: punct/leet views must not fail ordinary postings
        Case {
            name: "dotted wip is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "w.i.p. engineer in Stockholm",
            want: pass,
        },
        Case {
            name: "digits in a normal posting are not leet cues",
            url: "https://jobs.example.test/posting/1",
            body: "Stage 1 rust interview, 7 years, 10 people",
            want: pass,
        },
        Case {
            name: "re-wire a board today is not wire money today",
            url: "https://jobs.example.test/posting/1",
            body: "re-wire a board today",
            want: pass,
        },
    ]
}

#[test]
fn adversarial_matrix_matches_expected_verdicts() {
    for case in matrix() {
        let g = audit(case.url, case.body, &flags());
        assert_eq!(
            g.verdict, case.want,
            "case {} url={:?} body={:?}",
            case.name, case.url, case.body
        );
        if case.want == GateVerdict::Fail {
            assert_eq!(
                g.reason.as_deref(),
                Some(GATE_REASON),
                "case {} should be a hard fail",
                case.name
            );
        }
    }
}

#[test]
fn one_megabyte_input_finishes_fast() {
    let mut body = "plain text ".repeat(100_000);
    body.truncate(1_000_000);
    assert_eq!(body.len(), 1_000_000);
    let start = Instant::now();
    let g = audit(clean_url(), &body, &flags());
    let elapsed = start.elapsed();
    assert_eq!(g.verdict, GateVerdict::Pass);
    assert!(
        elapsed < Duration::from_secs(3),
        "1 MB audit took {elapsed:?}, expected linear-time"
    );

    let mut with_cue = body;
    with_cue.truncate(1_000_000 - 17);
    with_cue.push_str(" wire money today");
    assert!(with_cue.len() >= 1_000_000);
    let start = Instant::now();
    let g = audit(clean_url(), &with_cue, &flags());
    let elapsed = start.elapsed();
    assert_eq!(g.verdict, GateVerdict::Fail);
    assert!(
        elapsed < Duration::from_secs(3),
        "1 MB cue audit took {elapsed:?}, expected linear-time"
    );
}
