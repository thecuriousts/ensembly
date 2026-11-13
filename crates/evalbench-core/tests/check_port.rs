//! Ported evalbench-python check cases (tests/test_evalbench.py) plus parity traps.
mod common;
use common::*;
use evalbench_core::answer::check_answer;
use evalbench_core::check::{evidenced, gaps, gaps_from_flags, parse_pack, Pack};

const A_ALL: [&str; 4] = ["LLM evals", "Python", "RAG", "Observability"];

/// Python: test_full_pack_has_no_gaps (with its proofs-stripped positive control).
#[test]
fn full_pack_has_no_gaps() {
    let (posting, pack) = task("t01-full");
    assert!(
        !pack.evidence.is_empty(),
        "positive control needs evidence items"
    );
    assert_eq!(gaps(&posting, &pack), Vec::<String>::new());
    let mut stripped = pack.clone();
    for e in &mut stripped.evidence {
        e.proof = None;
    }
    let all: Vec<String> = posting.must_haves.iter().map(|m| m.name.clone()).collect();
    assert_eq!(gaps(&posting, &stripped), all);
    assert_eq!(all, names(&A_ALL));
}

/// Python: test_gappy_pack_flags_known_gaps (evals claimed without proof is a gap).
#[test]
fn gappy_pack_flags_known_gaps() {
    let (posting, pack) = task("t02-gappy");
    assert_eq!(
        gaps(&posting, &pack),
        names(&["LLM evals", "RAG", "Observability"])
    );
}

/// P2 at pack level: empty evidence gaps every must-have, and that task cannot pass
/// with an answer that leaves any must-have out.
#[test]
fn empty_evidence_pack_gaps_every_must_have() {
    let empty = Pack { evidence: vec![] };
    for id in ["t01-full", "t09-data-full"] {
        let (posting, _) = task(id);
        let all: Vec<String> = posting.must_haves.iter().map(|m| m.name.clone()).collect();
        assert_eq!(gaps(&posting, &empty), all, "{id}");
        assert!(evidenced(&posting, &empty).iter().all(|b| !b), "{id}");
    }
    let dir = tasks_dir().join("t04-empty-evidence");
    let want = expected(&dir);
    assert_eq!(want, names(&A_ALL));
    assert!(!check_answer(&want, r#"{"gaps":[]}"#));
    assert!(!check_answer(
        &want,
        r#"{"gaps":["LLM evals","Python","RAG"]}"#
    ));
}

#[test]
fn proof_and_keyword_must_be_on_same_item() {
    let (posting, pack) = task("t08-split-item");
    assert_eq!(gaps(&posting, &pack), names(&A_ALL));
}

#[test]
fn empty_string_proof_is_no_proof() {
    let (posting, pack) = task("t07-empty-proof");
    assert_eq!(gaps(&posting, &pack), names(&A_ALL));
}

/// D6: no trim; Python truthiness counts " " as a proof.
#[test]
fn whitespace_proof_counts_as_proof() {
    let (posting, pack) = parity("p05-whitespace-proof");
    assert_eq!(gaps(&posting, &pack), Vec::<String>::new());
}

#[test]
fn keyword_match_is_case_insensitive_substring() {
    let (p, k) = task("t05-case");
    assert_eq!(gaps(&p, &k), names(&["RAG", "Observability"]));
    // "storage" contains "rag": substring semantics, kept for parity.
    let (p, k) = task("t06-substring-trap");
    assert_eq!(
        gaps(&p, &k),
        names(&["LLM evals", "Python", "Observability"])
    );
}

/// D2 (flagged): Python `"" in s` is always true, so an empty keyword matches any
/// proved claim. Kept for parity, not rejected.
#[test]
fn empty_keyword_matches_any_proved_claim() {
    let (p, k) = parity("p01-empty-keyword");
    assert_eq!(gaps(&p, &k), Vec::<String>::new());
}

#[test]
fn empty_keyword_list_is_always_a_gap() {
    let (p, k) = parity("p02-empty-keyword-list");
    assert_eq!(gaps(&p, &k), names(&["Nothing"]));
}

/// D3: zero must-haves give no gaps (pack level). Trace-level empty set scores 0 (P2).
#[test]
fn zero_must_haves_gives_no_gaps() {
    let (p, k) = parity("p03-zero-must-haves");
    assert_eq!(gaps(&p, &k), Vec::<String>::new());
    assert_eq!(evidenced(&p, &k), Vec::<bool>::new());
}

/// D5: Unicode lowercase, like Python `.lower()`. ASCII lowercasing would leave a gap.
#[test]
fn unicode_lowercase_matches_python() {
    let (p, k) = parity("p04-unicode");
    assert_eq!(gaps(&p, &k), Vec::<String>::new());
}

#[test]
fn gaps_keep_posting_order_and_duplicates() {
    let (p, k) = parity("p06-duplicate-names");
    assert_eq!(gaps(&p, &k), names(&["Python", "Python"]));
}

#[test]
fn gaps_from_flags_lists_false_indices() {
    assert_eq!(gaps_from_flags(&[]), Vec::<usize>::new());
    assert_eq!(gaps_from_flags(&[true, false, true, false]), vec![1, 3]);
    assert_eq!(gaps_from_flags(&[false, false]), vec![0, 1]);
}

#[test]
fn gaps_equals_names_of_gaps_from_flags() {
    for dir in case_dirs() {
        let (posting, pack) = load_case(&dir);
        let from_flags: Vec<String> = gaps_from_flags(&evidenced(&posting, &pack))
            .into_iter()
            .map(|i| posting.must_haves[i].name.clone())
            .collect();
        assert_eq!(gaps(&posting, &pack), from_flags, "{}", dir.display());
        assert_eq!(gaps(&posting, &pack), expected(&dir), "{}", dir.display());
    }
}

#[test]
fn missing_claim_is_a_parse_error() {
    assert!(parse_pack(r#"{"evidence":[{"proof":"https://example.org/x"}]}"#).is_err());
    // proof key absent is fine (Python `e.get("proof")`).
    let p = parse_pack(r#"{"evidence":[{"claim":"x"}]}"#).unwrap();
    assert_eq!(p.evidence[0].proof, None);
}
