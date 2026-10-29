pub const SCHEMA_VERSION: u32 = 1;
mod audit;
mod ledger;
mod wire;
pub use audit::*;
pub use ledger::*;
pub use wire::{decode_line, encode_line, Decoded};
#[cfg(test)]
#[rustfmt::skip]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;
    const LEDGER: &str = "fixtures/ledger-v0.jsonl";
    const AUDIT: &str = "fixtures/audit-v0.jsonl";
    fn text(rel: &str) -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
    }
    fn first(rel: &str) -> String {
        text(rel).lines().find(|l| !l.trim().is_empty()).unwrap().into()
    }
    fn set(rel: &str, key: &str, val: serde_json::Value) -> String {
        let mut obj: serde_json::Value = serde_json::from_str(&first(rel)).unwrap();
        obj[key] = val;
        obj.to_string()
    }
    fn trip<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(rel: &str) {
        for line in text(rel).lines().filter(|l| !l.trim().is_empty()) {
            let d = decode_line::<T>(line).unwrap();
            assert_eq!(d.version, 0);
            assert_eq!(encode_line(0, &d.value).unwrap(), line);
        }
    }
    #[test]
    fn fixture_lines_round_trip() {
        trip::<Row>(LEDGER);
        trip::<Audit>(AUDIT);
        for line in text(LEDGER).lines().filter(|l| !l.trim().is_empty()) {
            check_row_submit_time(&decode_line::<Row>(line).unwrap().value).unwrap();
        }
    }
    #[test]
    fn missing_v_reads_as_zero() {
        for rel in [LEDGER, AUDIT] {
            assert!(serde_json::from_str::<serde_json::Value>(&first(rel)).unwrap().get("v").is_none());
        }
        assert_eq!(decode_line::<Row>(&first(LEDGER)).unwrap().version, 0);
        assert_eq!(decode_line::<Audit>(&first(AUDIT)).unwrap().version, 0);
    }
    #[test]
    fn version_two_is_refused() {
        for n in [json!(0), json!(2), json!("1")] {
            assert!(decode_line::<Row>(&set(LEDGER, "v", n.clone())).is_err());
            assert!(decode_line::<Audit>(&set(AUDIT, "v", n)).is_err());
        }
    }
    #[test]
    fn unknown_field_rejected() {
        let extra = json!(1);
        assert!(decode_line::<Row>(&set(LEDGER, "extra", extra.clone())).is_err());
        assert!(decode_line::<Audit>(&set(AUDIT, "extra", extra)).is_err());
    }
    #[test]
    fn unknown_status_or_verdict_refused() {
        assert!(decode_line::<Row>(&set(LEDGER, "status", json!("ghosted"))).is_err());
        assert!(decode_line::<Audit>(&set(AUDIT, "verdict", json!("maybe"))).is_err());
    }
    #[test]
    fn version_one_round_trips() {
        let row = decode_line::<Row>(&first(LEDGER)).unwrap();
        let again = decode_line::<Row>(&encode_line(SCHEMA_VERSION, &row.value).unwrap()).unwrap();
        assert_eq!((again.version, again.value), (SCHEMA_VERSION, row.value));
    }
    #[test]
    fn submit_time_link_and_domain() {
        assert!(check_submit_time("2026-10-26T18:59:00Z").is_err());
        assert!(check_submit_time("2026-10-26T18:59:00+01:00").is_ok());
        assert!(check_submit_time("2026-07-01T10:00:00+02:00").is_ok());
        assert_eq!(link_key("https://t.example/j/application/"), "https://t.example/j");
        assert_eq!(canonical_domain(" HTTPS://x@www.Acme.Example.test:443/jobs?x=1#f "), "acme.example.test");
    }
    #[test]
    fn exit_code_contract() {
        assert_eq!(exit_code(GateStatus::Pass), 0);
        assert_eq!(exit_code(GateStatus::Flag), 3);
        assert_eq!(exit_code(GateStatus::Block), 4);
        assert_eq!(exit_code(GateStatus::Unaudited), 5);
        assert_eq!(exit_code(GateStatus::Stale), 5);
    }
    #[test]
    fn exit_constants_do_not_collide() {
        let codes = [EXIT_PASS, EXIT_ERROR, EXIT_USAGE, EXIT_FLAG, EXIT_BLOCK, EXIT_UNAUDITED_OR_STALE, EXIT_PULSE_REFUSED_WRITE];
        assert_eq!(codes, [0, 1, 2, 3, 4, 5, 7]);
        let mut uniq = codes.to_vec();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), codes.len());
    }
    #[test]
    fn exit_code_reverse_map() {
        use std::collections::BTreeMap;
        let mut rev: BTreeMap<i32, Vec<GateStatus>> = BTreeMap::new();
        for s in [GateStatus::Pass, GateStatus::Flag, GateStatus::Block, GateStatus::Unaudited, GateStatus::Stale] {
            rev.entry(exit_code(s)).or_default().push(s);
        }
        assert_eq!(rev[&0], vec![GateStatus::Pass]);
        assert_eq!(rev[&3], vec![GateStatus::Flag]);
        assert_eq!(rev[&4], vec![GateStatus::Block]);
        assert_eq!(rev[&5], vec![GateStatus::Unaudited, GateStatus::Stale]);
        for c in [EXIT_ERROR, EXIT_USAGE, EXIT_PULSE_REFUSED_WRITE] {
            assert!(!rev.contains_key(&c), "{c}");
        }
    }
    #[test]
    fn fixtures_exclude_personal_markers() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let mut n = 0;
        for ent in std::fs::read_dir(dir).unwrap().map(|e| e.unwrap()) {
            let body = std::fs::read_to_string(ent.path()).unwrap();
            assert!(!body.is_empty());
            for bad in ["@", "+46", "/home/"] {
                assert!(!body.contains(bad), "{bad}");
            }
            n += 1;
        }
        assert!(n >= 2);
    }
    #[test]
    fn checked_in_schema_matches_types() {
        match_schema("schemas/ledger-row.v1.json", schemars::schema_for!(Row));
        match_schema("schemas/company-audit.v1.json", schemars::schema_for!(Audit));
    }
    fn match_schema(rel: &str, schema: schemars::schema::RootSchema) {
        assert_eq!(text(rel).trim_end(), serde_json::to_string(&schema).unwrap());
    }
}
