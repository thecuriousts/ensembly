//! Proof: default `ensembly-schema` root exports omit opportunity-finder types.

#[test]
fn lib_rs_default_section_omits_opportunity_types() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));
    let of_idx = src
        .find("#[cfg(feature = \"opportunity-finder\")]")
        .expect("feature gate");
    let head = &src[..of_idx];
    for bad in [
        "LedgerRow",
        "GateStatus",
        "CompareReport",
        "map_ledger_status",
        "Verdict",
    ] {
        assert!(
            !head.contains(bad),
            "{bad} leaked into default exports section"
        );
    }
}
