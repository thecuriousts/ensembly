//! P1: score stays in 0..=100 (no panic, no overflow).
//! P2: empty evidence gives 0 — an empty task set, or no first-attempt pass, scores 0;
//!     an all-false flag vector makes every must-have a gap.
//!
//! Each proof runs twice: as a bounded exhaustive `cargo test` (always), and as a Kani
//! harness (`cargo kani -p evalbench-core`, only where cargo-kani is installed). Each
//! proof is shown failing on a deliberately broken variant: in `cargo test` the same
//! checker must find a counterexample in the broken fn; under Kani the broken harnesses
//! (feature `broken-proof-check`) must report `VERIFICATION:- FAILED`.
//!
//! The broken variants are test-only and never ship.

#[cfg(any(test, kani))]
#[allow(dead_code)]
mod broken {
    /// P1 broken: no clamp of `passed` to `total`, so (2, 1) gives 200.
    pub fn percent_floor_broken(passed: u32, total: u32) -> u8 {
        if total == 0 {
            return 0;
        }
        (100u64 * passed as u64 / total as u64) as u8
    }

    /// P2 broken: the superseded D-E3 rule "nothing to check scores 100".
    pub fn score_from_passes_broken(passes: &[bool]) -> u8 {
        if passes.is_empty() {
            return 100;
        }
        let p = passes.iter().filter(|b| **b).count() as u64;
        (100 * p / passes.len() as u64) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::broken::*;
    use crate::check::gaps_from_flags;
    use crate::score::{percent_floor, score_from_passes};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    const MAX_LEN: usize = 8;
    const EDGES: [u32; 5] = [0, 1, 1 << 31, u32::MAX - 1, u32::MAX];

    /// Every bool vector of length 0..=MAX_LEN.
    fn all_vectors() -> Vec<Vec<bool>> {
        let mut out = Vec::new();
        for len in 0..=MAX_LEN {
            for bits in 0u32..(1 << len) {
                out.push((0..len).map(|i| bits & (1 << i) != 0).collect());
            }
        }
        out
    }

    /// First (passed, total, result) where `f` panics or exceeds 100. `None` = holds.
    fn p1_counterexample(f: fn(u32, u32) -> u8) -> Option<(u32, u32, String)> {
        let small: Vec<u32> = (0..=128).collect();
        for &total in small.iter().chain(EDGES.iter()) {
            for &passed in small.iter().chain(EDGES.iter()) {
                match catch_unwind(AssertUnwindSafe(|| f(passed, total))) {
                    Ok(v) if v <= 100 => {}
                    Ok(v) => return Some((passed, total, v.to_string())),
                    Err(_) => return Some((passed, total, "panic".into())),
                }
            }
        }
        None
    }

    fn p1_vec_counterexample(f: fn(&[bool]) -> u8) -> Option<(Vec<bool>, String)> {
        for v in all_vectors() {
            match catch_unwind(AssertUnwindSafe(|| f(&v))) {
                Ok(s) if s <= 100 => {}
                Ok(s) => return Some((v, s.to_string())),
                Err(_) => return Some((v, "panic".into())),
            }
        }
        None
    }

    /// First empty or all-false vector where `f` does not score 0.
    fn p2_counterexample(f: fn(&[bool]) -> u8) -> Option<(Vec<bool>, String)> {
        for len in 0..=MAX_LEN {
            let v = vec![false; len];
            match catch_unwind(AssertUnwindSafe(|| f(&v))) {
                Ok(0) => {}
                Ok(s) => return Some((v, s.to_string())),
                Err(_) => return Some((v, "panic".into())),
            }
        }
        None
    }

    #[test]
    fn p1_score_in_range_and_broken_variant_is_caught() {
        let broken = p1_counterexample(percent_floor_broken);
        println!("P1 broken percent_floor counterexample: {broken:?}");
        assert!(
            broken.is_some(),
            "P1 checker is toothless: the broken variant passed"
        );

        let real = p1_counterexample(percent_floor);
        assert_eq!(real, None, "P1 fails on percent_floor");
        let real_vec = p1_vec_counterexample(score_from_passes);
        assert_eq!(real_vec, None, "P1 fails on score_from_passes");
    }

    #[test]
    fn p2_no_pass_scores_zero_and_broken_variant_is_caught() {
        let broken = p2_counterexample(score_from_passes_broken);
        println!("P2 broken score_from_passes counterexample: {broken:?}");
        assert_eq!(
            broken.as_ref().map(|(v, s)| (v.len(), s.as_str())),
            Some((0, "100")),
            "P2 checker must catch the empty -> 100 rule"
        );

        assert_eq!(
            p2_counterexample(score_from_passes),
            None,
            "P2 fails on score_from_passes"
        );
        for len in 0..=MAX_LEN {
            assert_eq!(
                gaps_from_flags(&vec![false; len]),
                (0..len).collect::<Vec<_>>(),
                "all-false flags must gap every must-have (len {len})"
            );
        }
    }
}

#[cfg(kani)]
mod harnesses {
    use crate::check::gaps_from_flags;
    use crate::score::{percent_floor, score_from_passes};

    const N: usize = 8;

    fn any_prefix(buf: &[bool; N]) -> &[bool] {
        let n: usize = kani::any();
        kani::assume(n <= N);
        &buf[..n]
    }

    #[kani::proof]
    fn p1_percent_floor_in_range() {
        let passed: u32 = kani::any();
        let total: u32 = kani::any();
        assert!(percent_floor(passed, total) <= 100);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn p1_score_from_passes_in_range() {
        let buf: [bool; N] = kani::any();
        assert!(score_from_passes(any_prefix(&buf)) <= 100);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn p2_no_pass_scores_zero() {
        let n: usize = kani::any();
        kani::assume(n <= N);
        let buf = [false; N];
        assert_eq!(score_from_passes(&buf[..n]), 0);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn p2_all_false_flags_gap_everything() {
        let n: usize = kani::any();
        kani::assume(n <= N);
        let buf = [false; N];
        assert_eq!(gaps_from_flags(&buf[..n]).len(), n);
    }

    #[cfg(feature = "broken-proof-check")]
    #[kani::proof]
    fn p1_broken_percent_floor() {
        let passed: u32 = kani::any();
        let total: u32 = kani::any();
        assert!(super::broken::percent_floor_broken(passed, total) <= 100);
    }

    #[cfg(feature = "broken-proof-check")]
    #[kani::proof]
    #[kani::unwind(10)]
    fn p2_broken_empty_scores_hundred() {
        let n: usize = kani::any();
        kani::assume(n <= N);
        let buf = [false; N];
        assert_eq!(super::broken::score_from_passes_broken(&buf[..n]), 0);
    }
}
