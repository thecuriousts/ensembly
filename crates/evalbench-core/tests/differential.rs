//! Differential test, gap lists only: vendored Python reference vs Rust `gaps()` on
//! every synthetic fixture. python3 missing is a loud failure (D14), not a skip.
mod common;
use common::*;
use evalbench_core::check::gaps;
use std::path::Path;
use std::process::Command;

/// sha256 of evalbench-python/evalbench/check.py at the time of the port.
const REFERENCE_SHA256: &str = "986881450914238a6bfa8a432449a38ea76b6d3eaccbeea6a620ea26db697491";

fn reference() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference/check.py")
}

#[test]
fn reference_check_py_is_byte_identical_to_source() {
    let bytes = std::fs::read(reference()).unwrap();
    assert_eq!(sha256_hex(&bytes), REFERENCE_SHA256);
}

#[test]
fn rust_gaps_match_python_on_every_fixture() {
    let mut compared = 0;
    for dir in case_dirs() {
        let out = Command::new("python3")
            .args(["-I", "-X", "utf8"])
            .arg(reference())
            .arg(dir.join("posting.json"))
            .arg(dir.join("pack.json"))
            .output()
            .unwrap_or_else(|e| panic!("python3 not found: differential test cannot run ({e})"));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("{}: python output not JSON: {e}", dir.display()));
        let py: Vec<String> = v["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g.as_str().unwrap().to_string())
            .collect();
        let py_code = out.status.code().unwrap();
        assert_eq!(
            py_code,
            if py.is_empty() { 0 } else { 1 },
            "{}",
            dir.display()
        );

        let (posting, pack) = load_case(&dir);
        assert_eq!(
            gaps(&posting, &pack),
            py,
            "gap lists differ: {}",
            dir.display()
        );
        compared += 1;
    }
    assert!(compared >= 18, "only {compared} fixtures compared");
}
