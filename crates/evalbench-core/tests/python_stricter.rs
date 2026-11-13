//! Examples, not a complete list: inputs Python `check()` accepts and Rust rejects.
//! The general rule (see `src/check.rs`): Rust's schema is strict and requires every
//! field with its type; Python reads only the fields its short-circuiting `any()`
//! reaches and treats any truthy `proof` as a proof. Parity holds on well-formed input,
//! which is every frozen fixture. Each example asserts both halves: Python's answer via
//! the vendored `check.py`, and Rust's parse error.
mod common;
use evalbench_core::check::{parse_pack, parse_posting};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

const ONE_MUST_HAVE: &str = r#"{"must_haves":[{"name":"A","keywords":["a"]}]}"#;

fn reference() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference/check.py")
}

/// Runs the vendored Python reference on (posting, pack). Returns (exit code, gaps).
/// python3 missing is a loud failure (D14), as in the differential test.
fn python(case: &str, posting: &str, pack: &str) -> (i32, Value) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("python-stricter-{case}"));
    std::fs::create_dir_all(&dir).unwrap();
    let (po, pa) = (dir.join("posting.json"), dir.join("pack.json"));
    std::fs::write(&po, posting).unwrap();
    std::fs::write(&pa, pack).unwrap();
    let out = Command::new("python3")
        .args(["-I", "-X", "utf8"])
        .arg(reference())
        .args([&po, &pa])
        .output()
        .unwrap_or_else(|e| panic!("python3 not found: cannot run the reference ({e})"));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "{case}: Python did not accept the input: {e}\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out.status.code().unwrap(), v["gaps"].clone())
}

/// Rust rejects (posting, pack) at parse time with an error containing `want`.
fn rust_rejects(case: &str, posting: &str, pack: &str, want: &str) {
    let err = match (parse_posting(posting), parse_pack(pack)) {
        (Err(e), _) | (Ok(_), Err(e)) => e.to_string(),
        (Ok(_), Ok(_)) => panic!("{case}: Rust accepted the input"),
    };
    assert!(err.contains(want), "{case}: {err}");
}

/// Example: an evidence item with no `claim` and no proof. Python never reads the
/// claim (the falsy proof short-circuits) and reports the gap.
#[test]
fn stricter_missing_claim_without_proof() {
    let pack = r#"{"evidence":[{"proof":""}]}"#;
    assert_eq!(python("s1", ONE_MUST_HAVE, pack), (1, json!(["A"])));
    rust_rejects("s1", ONE_MUST_HAVE, pack, "missing field `claim`");
}

/// Example: zero must-haves and a pack with no `evidence` key. Python never reads
/// `pack["evidence"]` when there is nothing to check.
#[test]
fn stricter_zero_must_haves_pack_without_evidence() {
    let (posting, pack) = (r#"{"must_haves":[]}"#, "{}");
    assert_eq!(python("s2", posting, pack), (0, json!([])));
    rust_rejects("s2", posting, pack, "missing field `evidence`");
}

/// Example: a truthy non-string proof (`true`, `1`). Python's truthiness counts it.
#[test]
fn stricter_non_string_proof() {
    for (i, proof) in ["true", "1"].iter().enumerate() {
        let case = format!("s3-{i}");
        let pack = format!(r#"{{"evidence":[{{"claim":"a","proof":{proof}}}]}}"#);
        assert_eq!(python(&case, ONE_MUST_HAVE, &pack), (0, json!([])));
        rust_rejects(&case, ONE_MUST_HAVE, &pack, "invalid type");
    }
}

/// More examples of the same class, found in review of 5f27a86.
#[test]
fn stricter_more_examples() {
    let no_keywords = r#"{"must_haves":[{"name":"A","keywords":[]}]}"#;
    let cases: [(&str, &str, &str, i32, Value, &str); 8] = [
        // Non-string claim behind a falsy proof: Python never lowercases it.
        (
            "claim-int",
            ONE_MUST_HAVE,
            r#"{"evidence":[{"claim":5,"proof":""}]}"#,
            1,
            json!(["A"]),
            "invalid type: integer",
        ),
        // Null claim, no proof.
        (
            "claim-null",
            ONE_MUST_HAVE,
            r#"{"evidence":[{"claim":null}]}"#,
            1,
            json!(["A"]),
            "invalid type: null",
        ),
        // Falsy non-string proofs.
        (
            "proof-zero",
            ONE_MUST_HAVE,
            r#"{"evidence":[{"claim":"a","proof":0}]}"#,
            1,
            json!(["A"]),
            "invalid type",
        ),
        (
            "proof-false",
            ONE_MUST_HAVE,
            r#"{"evidence":[{"claim":"a","proof":false}]}"#,
            1,
            json!(["A"]),
            "invalid type",
        ),
        // Empty keyword list: Python's inner any() is empty, so the claim is never read.
        (
            "proof-no-claim-no-keywords",
            no_keywords,
            r#"{"evidence":[{"proof":"https://example.org/x"}]}"#,
            1,
            json!(["A"]),
            "missing field `claim`",
        ),
        // Posting side: an evidenced must-have with no name; Python only reads the name
        // of a gap.
        (
            "name-missing",
            r#"{"must_haves":[{"keywords":["a"]}]}"#,
            r#"{"evidence":[{"claim":"a","proof":"https://example.org/x"}]}"#,
            0,
            json!([]),
            "missing field `name`",
        ),
        // Non-string name on a gap: Python echoes it.
        (
            "name-int",
            r#"{"must_haves":[{"name":5,"keywords":["a"]}]}"#,
            r#"{"evidence":[]}"#,
            1,
            json!([5]),
            "invalid type: integer",
        ),
        // Zero must-haves: Python never reads the evidence items.
        (
            "zero-must-haves-empty-item",
            r#"{"must_haves":[]}"#,
            r#"{"evidence":[{}]}"#,
            0,
            json!([]),
            "missing field `claim`",
        ),
    ];
    for (case, posting, pack, code, gaps, want) in cases {
        assert_eq!(python(case, posting, pack), (code, gaps), "{case}");
        rust_rejects(case, posting, pack, want);
    }
}
