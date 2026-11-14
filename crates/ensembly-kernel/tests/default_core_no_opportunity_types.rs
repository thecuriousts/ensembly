//! Default kernel/schema builds must not expose opportunity-finder type names.

#[test]
fn default_ensembly_schema_has_no_ledger_row_symbol() {
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ensembly-schema/src/lib.rs"
    ));
    let of_idx = src
        .find("#[cfg(feature = \"opportunity-finder\")]")
        .expect("feature gate block");
    let default_exports = &src[..of_idx];
    assert!(
        !default_exports.contains("LedgerRow"),
        "LedgerRow must not be exported without opportunity-finder"
    );
    assert!(
        !default_exports.contains("GateStatus"),
        "GateStatus must not be exported without opportunity-finder"
    );
    assert!(
        !default_exports.contains("compare_ledger"),
        "compare_ledger must not be exported without opportunity-finder"
    );
}
