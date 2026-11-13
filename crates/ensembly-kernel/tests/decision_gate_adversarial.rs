//! Adversarial regression matrix for the E3 scam-gate.
//!
//! Covers every probe from the same-family reviews of 7672d81 and
//! 6d90827, plus HTML numeric entities and combining marks. Fixtures
//! are synthetic (`*.example.test` only).

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
