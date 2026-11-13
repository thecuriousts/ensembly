//! Kani gate for P1/P2 (D13). cargo-kani is not installed on laptop-1 today, so this is
//! `#[ignore]` and CI stays green; the bounded exhaustive versions run in src/proofs.rs.
//! Run: cargo test -p evalbench-core --test kani_gate -- --ignored
//! Today it fails loudly: "cargo-kani not installed: P1/P2 Kani proofs NOT RUN".
use std::process::Command;

fn kani(extra: &[&str]) -> (bool, String) {
    let out = Command::new("cargo")
        .args(["kani", "-p", "evalbench-core"])
        .args(extra)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run cargo kani");
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

#[test]
#[ignore = "needs cargo-kani: cargo test -p evalbench-core --test kani_gate -- --ignored"]
fn kani_proofs_pass_and_broken_variants_fail() {
    let installed = Command::new("cargo")
        .args(["kani", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    assert!(
        installed,
        "cargo-kani not installed: P1/P2 Kani proofs NOT RUN"
    );

    for h in [
        "p1_percent_floor_in_range",
        "p1_score_from_passes_in_range",
        "p2_no_pass_scores_zero",
        "p2_all_false_flags_gap_everything",
    ] {
        let (ok, text) = kani(&["--harness", h]);
        assert!(
            ok && text.contains("VERIFICATION:- SUCCESSFUL"),
            "{h} must verify:\n{text}"
        );
    }
    for h in ["p1_broken_percent_floor", "p2_broken_empty_scores_hundred"] {
        let (ok, text) = kani(&["--features", "broken-proof-check", "--harness", h]);
        assert!(
            !ok && text.contains("VERIFICATION:- FAILED"),
            "{h} must FAIL:\n{text}"
        );
    }
}
