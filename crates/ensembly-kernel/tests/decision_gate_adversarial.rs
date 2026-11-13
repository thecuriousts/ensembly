//! Adversarial regression matrix for the E3 scam-gate.
//!
//! Single source of truth for scam-gate adversarial probes: reviews of
//! 7672d81, 6d90827, 1e85825, a30eab9, 11b68e4 and 9ae7e04, plus userinfo,
//! IDN dots, leet, HTML cues, spaced brackets, Unicode dashes, named HTML
//! entities (including ndash/mdash/minus/hyphen/nbsp, with and without a
//! semicolon, ZWSP-split names, and double-encoded forms) and negative
//! cases. ZWJ (U+200D) and ZWNJ (U+200C) are format characters and are
//! documented in the matrix below. `&NDASH;` is not a WHATWG name
//! (the decoder stays case-sensitive and will not fold it to `&ndash;`)
//! but the junk-as-separator cue view treats unknown names as
//! separators, so `wire&NDASH;money today` is a hard fail.
//! Fixtures are synthetic (`*.example.test` only).

use ensembly_kernel::decision::{
    audit, byte_work, reset_byte_work, FlagList, DEPTH_REASON, GATE_REASON,
};
use ensembly_schema::GateVerdict;
use std::time::{Duration, Instant};

fn wall_limit() -> Duration {
    // Debug builds and shared CI runners vary; keep a generous debug
    // bound and a tight one only in release. Linear-time is also
    // checked with a byte-work counter in the unit tests.
    if cfg!(debug_assertions) {
        Duration::from_secs(10)
    } else {
        Duration::from_secs(3)
    }
}

fn work_bound(input_len: usize) -> u64 {
    // Ample for a handful of linear views and a capped decode loop.
    (input_len as u64)
        .saturating_mul(128)
        .saturating_add(64 * 1024)
}

fn assert_work_linear(input_len: usize) {
    let work = byte_work();
    let bound = work_bound(input_len);
    assert!(
        work <= bound,
        "byte work {work} exceeded bound {bound} for {input_len} input bytes"
    );
}

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
        // ZWJ (U+200D) and ZWNJ (U+200C) are format characters: one view
        // strips them, the other replaces them with a space, so a hard
        // cue split by either still fails. Named-entity forms (&zwj;,
        // &zwnj;) are covered by the folded-class entity property test.
        Case {
            name: "ZWJ between cue words",
            url: u,
            body: "wire\u{200D}money today",
            want: fail,
        },
        Case {
            name: "ZWNJ between cue words",
            url: u,
            body: "wire\u{200C}money today",
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
            name: "HTML named ndash entity between cue words",
            url: u,
            body: "wire&ndash;money today",
            want: fail,
        },
        Case {
            name: "HTML named mdash entity between cue words",
            url: u,
            body: "wire&mdash;money today",
            want: fail,
        },
        Case {
            name: "HTML named minus entity between cue words",
            url: u,
            body: "wire&minus;money today",
            want: fail,
        },
        Case {
            name: "HTML named hyphen entity between cue words",
            url: u,
            body: "wire&hyphen;money today",
            want: fail,
        },
        Case {
            name: "HTML named nbsp entity between cue words",
            url: u,
            body: "wire&nbsp;money today",
            want: fail,
        },
        Case {
            name: "HTML named ndash entity as a flag-host dot in the body",
            url: u,
            body: "see phish&ndash;example.test",
            want: fail,
        },
        Case {
            name: "HTML named mdash entity as a flag-host dot in the body",
            url: u,
            body: "see phish&mdash;example.test",
            want: fail,
        },
        Case {
            name: "HTML named minus entity as a flag-host dot in the body",
            url: u,
            body: "see phish&minus;example.test",
            want: fail,
        },
        Case {
            name: "HTML named hyphen entity as a flag-host dot in the body",
            url: u,
            body: "see phish&hyphen;example.test",
            want: fail,
        },
        // Fail-closed: known names are accepted without a trailing semicolon.
        Case {
            name: "HTML named ndash entity without semicolon between cue words",
            url: u,
            body: "wire&ndashmoney today",
            want: fail,
        },
        Case {
            name: "HTML named mdash entity without semicolon between cue words",
            url: u,
            body: "wire&mdashmoney today",
            want: fail,
        },
        Case {
            name: "HTML named minus entity without semicolon between cue words",
            url: u,
            body: "wire&minusmoney today",
            want: fail,
        },
        Case {
            name: "HTML named hyphen entity without semicolon between cue words",
            url: u,
            body: "wire&hyphenmoney today",
            want: fail,
        },
        Case {
            name: "HTML named nbsp entity without semicolon between cue words",
            url: u,
            body: "wire&nbspmoney today",
            want: fail,
        },
        Case {
            name: "HTML named ndash entity without semicolon as a flag-host dot",
            url: u,
            body: "see phish&ndashexample.test",
            want: fail,
        },
        // ZWSP-split names: a format character inside a named entity must
        // be stripped before decode, otherwise `&n` + ZWSP + `dash;` is
        // not a name and the cue would pass.
        Case {
            name: "ZWSP inside ndash entity name",
            url: u,
            body: "wire&n\u{200b}dash;money today",
            want: fail,
        },
        Case {
            name: "ZWSP between ampersand and ndash name",
            url: u,
            body: "wire&\u{200b}ndash;money today",
            want: fail,
        },
        Case {
            name: "numeric en-dash entity without semicolon between cue words",
            url: u,
            body: "wire&#8211money today",
            want: fail,
        },
        Case {
            name: "percent-encoded ndash entity between cue words",
            url: u,
            body: "wire%26ndash%3Bmoney today",
            want: fail,
        },
        Case {
            name: "percent-encoded ZWSP inside ndash entity name",
            url: u,
            body: "wire&n%E2%80%8Bdash;money today",
            want: fail,
        },
        Case {
            name: "double-encoded ndash entity between cue words",
            url: u,
            body: "wire&amp;ndash;money today",
            want: fail,
        },
        Case {
            name: "double-percent-encoded ndash entity between cue words",
            url: u,
            body: "wire%2526ndash%253Bmoney today",
            want: fail,
        },
        Case {
            name: "double-encoded period entity as a flag-host dot",
            url: u,
            body: "see phish&amp;period;example.test",
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
        // --- fourth review (a30eab9): unicode dashes and tech negatives ---
        Case {
            name: "non-breaking hyphen between cue words",
            url: u,
            body: "wire\u{2011}money today",
            want: fail,
        },
        Case {
            name: "en-dash between cue words",
            url: u,
            body: "wire\u{2013}money today",
            want: fail,
        },
        Case {
            name: "minus sign between cue words",
            url: u,
            body: "wire\u{2212}money today",
            want: fail,
        },
        Case {
            name: "non-breaking hyphen as a flag-host dot in the body",
            url: u,
            body: "contact phish\u{2011}example.test",
            want: fail,
        },
        Case {
            name: "en-dash as a flag-host dot in the URL",
            url: "https://phish\u{2013}example.test/x",
            body: "a role",
            want: fail,
        },
        Case {
            name: "C3.ai is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "we use C3.ai in production",
            want: pass,
        },
        Case {
            name: "v1.0 release is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "v1.0 release next week",
            want: pass,
        },
        Case {
            name: "S3 buckets is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "experience with S3 buckets",
            want: pass,
        },
        Case {
            name: "k8s is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "k8s and rust experience",
            want: pass,
        },
        Case {
            name: "pay a fee for lunch is not pay a fee to apply",
            url: "https://jobs.example.test/posting/1",
            body: "pay a fee for lunch",
            want: pass,
        },
        Case {
            name: "ASCII hyphenated unflagged domain stays distinct",
            url: "https://phish-example.test/job",
            body: "see also jobs-board.example.test",
            want: pass,
        },
        // &NDASH; is not a WHATWG name (the decoder stays case-sensitive
        // and will not fold it to &ndash;). The junk-as-separator view
        // still treats the unknown name as a cue separator, so this is
        // a hard fail.
        Case {
            name: "uppercase NDASH entity is a junk-as-separator hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wire&NDASH;money today",
            want: fail,
        },
        Case {
            name: "surrogate numeric entity is a junk-as-separator hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wire&#xD800;money today",
            want: fail,
        },
        Case {
            name: "overflow numeric entity is a junk-as-separator hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wire&#99999999;money today",
            want: fail,
        },
        Case {
            name: "non-hex numeric entity is a junk-as-separator hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wire&#xZZ;money today",
            want: fail,
        },
        Case {
            name: "truncated percent escape is a junk-as-separator hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wire%2money today",
            want: fail,
        },
        Case {
            name: "non-hex percent escape is a junk-as-separator hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wire%G1money today",
            want: fail,
        },
        Case {
            name: "unknown named entity is a junk-as-separator hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wire&zzzzz; money today",
            want: fail,
        },
        Case {
            name: "unknown named entity between flag-host labels fails",
            url: "https://jobs.example.test/posting/1",
            body: "see phish&zzzzz;example.test",
            want: fail,
        },
        Case {
            name: "bare ampersand before space is a token-stream hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wire& money today",
            want: fail,
        },
        Case {
            name: "bare ampersand glued between cue words is a token-stream hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wire&money today",
            want: fail,
        },
        Case {
            name: "bare ampersand after space is a token-stream hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wire &money today",
            want: fail,
        },
        Case {
            name: "bare ampersand with spaces around it is a token-stream hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wire & money today",
            want: fail,
        },
        Case {
            name: "overlong UTF-8 between flag-host labels fails",
            url: "https://jobs.example.test/posting/1",
            body: "see phish%c0%80example.test",
            want: fail,
        },
        Case {
            name: "percent-2e between flag-host labels fails",
            url: "https://jobs.example.test/posting/1",
            body: "see phish%2eexample.test",
            want: fail,
        },
        Case {
            name: "R&D is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "R&D engineer in Stockholm",
            want: pass,
        },
        Case {
            name: "AT&T is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "experience with AT&T systems",
            want: pass,
        },
        Case {
            name: "Q&A is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "Q&A session after the interview",
            want: pass,
        },
        Case {
            name: "copyright entity is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "role starts in 2026 &copy; the company",
            want: pass,
        },
        Case {
            name: "ordinary job-ad HTML with R&amp;D is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "<p>R&amp;D engineer in Stockholm</p>",
            want: pass,
        },
        Case {
            name: "50% off is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "50% off relocation bonus",
            want: pass,
        },
        Case {
            name: "a && b is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "a && b are both required skills",
            want: pass,
        },
        Case {
            name: "utm percent-20 URL is not a flagged host",
            url: "https://jobs.example.test/posting/1?utm_source=board&utm_medium=email%20list",
            body: "staff engineer, rust and systems",
            want: pass,
        },
        Case {
            name: "wire inside wireless is not a hard cue",
            url: "https://jobs.example.test/posting/1",
            body: "wireless money today",
            want: pass,
        },
    ]
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

fn is_hard_fail(g: &ensembly_kernel::decision::GateResult) -> bool {
    g.verdict == GateVerdict::Fail
        && (g.reason.as_deref() == Some(GATE_REASON) || g.reason.as_deref() == Some(DEPTH_REASON))
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
fn exhausted_decode_budget_rows_never_pass() {
    let u = clean_url();
    let four = format!("wire{}money today", percent_wrap_n("&n\u{200b}dash;", 4));
    let g = audit(u, &four, &flags());
    assert!(
        is_hard_fail(&g),
        "4 percent-wraps of ZWSP-split ndash must not Pass, got {:?} {:?}",
        g.verdict,
        g.reason
    );

    let seven = format!("wire{}money today", percent_wrap_n("%26ndash%3B", 7));
    let g = audit(u, &seven, &flags());
    assert!(
        is_hard_fail(&g),
        "7 wraps of %26ndash%3B must not Pass, got {:?} {:?}",
        g.verdict,
        g.reason
    );
}

#[test]
fn one_megabyte_input_finishes_fast() {
    let mut body = "plain text ".repeat(100_000);
    body.truncate(1_000_000);
    assert_eq!(body.len(), 1_000_000);
    reset_byte_work();
    let start = Instant::now();
    let g = audit(clean_url(), &body, &flags());
    let elapsed = start.elapsed();
    assert_eq!(g.verdict, GateVerdict::Pass);
    assert!(
        elapsed < wall_limit(),
        "1 MB audit took {elapsed:?}, expected linear-time"
    );
    assert_work_linear(body.len() + clean_url().len());

    let mut with_cue = body;
    with_cue.truncate(1_000_000 - 17);
    with_cue.push_str(" wire money today");
    assert!(with_cue.len() >= 1_000_000);
    reset_byte_work();
    let start = Instant::now();
    let g = audit(clean_url(), &with_cue, &flags());
    let elapsed = start.elapsed();
    assert_eq!(g.verdict, GateVerdict::Fail);
    assert!(
        elapsed < wall_limit(),
        "1 MB cue audit took {elapsed:?}, expected linear-time"
    );
    assert_work_linear(with_cue.len() + clean_url().len());

    // Pathological nested encoding: percent-encoded ZWSP inside a named
    // entity, repeated to 1 MB, with a nested-encoded cue at the end.
    let unit = "%26n%E2%80%8Bdash%3B";
    let mut nested = unit.repeat(1_000_000 / unit.len() + 1);
    nested.truncate(1_000_000);
    assert_eq!(nested.len(), 1_000_000);
    reset_byte_work();
    let start = Instant::now();
    let g = audit(clean_url(), &nested, &flags());
    let elapsed = start.elapsed();
    assert_eq!(g.verdict, GateVerdict::Pass);
    assert!(
        elapsed < wall_limit(),
        "1 MB nested-encoding audit took {elapsed:?}, expected linear-time"
    );
    assert_work_linear(nested.len() + clean_url().len());

    let cue = " wire%26n%E2%80%8Bdash%3Bmoney today";
    nested.truncate(1_000_000 - cue.len());
    nested.push_str(cue);
    assert!(nested.len() >= 1_000_000);
    reset_byte_work();
    let start = Instant::now();
    let g = audit(clean_url(), &nested, &flags());
    let elapsed = start.elapsed();
    assert_eq!(g.verdict, GateVerdict::Fail);
    assert!(
        elapsed < wall_limit(),
        "1 MB nested-encoding cue audit took {elapsed:?}, expected linear-time"
    );
    assert_work_linear(nested.len() + clean_url().len());
}
