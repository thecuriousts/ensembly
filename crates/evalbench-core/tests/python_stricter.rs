//! The three documented cases where Rust is stricter than the Python reference.
//! Each input is one Python `check()` accepts (its `any()` short-circuits before it
//! touches the missing or odd field); Rust rejects it at parse time. Both halves are
//! asserted: Rust's error here, and Python's answer via the vendored `check.py`.
mod common;
use evalbench_core::check::{parse_pack, parse_posting};
use std::path::{Path, PathBuf};
use std::process::Command;

const ONE_MUST_HAVE: &str = r#"{"must_haves":[{"name":"A","keywords":["a"]}]}"#;

fn reference() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference/check.py")
}

/// Runs the vendored Python reference on (posting, pack). Returns (exit code, gaps).
/// python3 missing is a loud failure (D14), as in the differential test.
fn python(case: &str, posting: &str, pack: &str) -> (i32, Vec<String>) {
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
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "{case}: Python did not accept the input: {e}\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let gaps = v["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g.as_str().unwrap().to_string())
        .collect();
    (out.status.code().unwrap(), gaps)
}

/// Stricter 1: an evidence item with no `claim` and no proof. Python never reads the
/// claim (the falsy proof short-circuits) and reports the gap; Rust requires `claim`.
#[test]
fn stricter_missing_claim_without_proof() {
    let pack = r#"{"evidence":[{"proof":""}]}"#;
    assert_eq!(
        python("s1", ONE_MUST_HAVE, pack),
        (1, vec!["A".to_string()])
    );
    let err = parse_pack(pack).unwrap_err().to_string();
    assert!(err.contains("missing field `claim`"), "{err}");
}

/// Stricter 2: zero must-haves and a pack with no `evidence` key. Python never reads
/// `pack["evidence"]` when there is nothing to check; Rust requires `evidence`.
#[test]
fn stricter_zero_must_haves_pack_without_evidence() {
    let (posting, pack) = (r#"{"must_haves":[]}"#, "{}");
    assert_eq!(python("s2", posting, pack), (0, Vec::<String>::new()));
    assert!(parse_posting(posting).is_ok());
    let err = parse_pack(pack).unwrap_err().to_string();
    assert!(err.contains("missing field `evidence`"), "{err}");
}

/// Stricter 3: a truthy non-string proof. Python's truthiness counts `true` as a proof;
/// Rust requires `proof` to be a string or null.
#[test]
fn stricter_non_string_proof() {
    let pack = r#"{"evidence":[{"claim":"a","proof":true}]}"#;
    assert_eq!(python("s3", ONE_MUST_HAVE, pack), (0, Vec::<String>::new()));
    let err = parse_pack(pack).unwrap_err().to_string();
    assert!(err.contains("invalid type: boolean"), "{err}");
}
