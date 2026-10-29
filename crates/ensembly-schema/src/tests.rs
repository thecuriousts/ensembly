use super::audit::AuditLine;
use super::ledger::LedgerRowLine;
use super::pack::PackRefLine;
use super::*;
use serde_json::json;
use std::path::Path;

const LEDGER: &str = "fixtures/ledger-v0.jsonl";
const AUDIT: &str = "fixtures/audit-v0.jsonl";
const PACK: &str = "fixtures/pack-ref-v0.jsonl";
const LEDGER_V1: &str = "fixtures/ledger-v1.jsonl";
const AUDIT_V1: &str = "fixtures/audit-v1.jsonl";
const PACK_V1: &str = "fixtures/pack-ref-v1.jsonl";

fn text(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

fn first(rel: &str) -> String {
    text(rel)
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap()
        .into()
}

fn set(rel: &str, key: &str, val: serde_json::Value) -> String {
    let mut obj: serde_json::Value = serde_json::from_str(&first(rel)).unwrap();
    obj[key] = val;
    obj.to_string()
}

fn trip<T>(rel: &str)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug + Checked,
{
    for line in text(rel).lines().filter(|l| !l.trim().is_empty()) {
        let d = decode_line::<T>(line).unwrap();
        assert_eq!(d.version, 0);
        assert_eq!(encode_line(0, &d.value).unwrap(), line);
    }
}

#[test]
fn fixture_lines_round_trip() {
    trip::<LedgerRow>(LEDGER);
    trip::<Audit>(AUDIT);
    trip::<PackRef>(PACK);
    for line in text(LEDGER).lines().filter(|l| !l.trim().is_empty()) {
        check_row_submit_time(&decode_line::<LedgerRow>(line).unwrap().value).unwrap();
    }
}

#[test]
fn missing_v_reads_as_zero() {
    for rel in [LEDGER, AUDIT, PACK] {
        assert!(serde_json::from_str::<serde_json::Value>(&first(rel))
            .unwrap()
            .get("v")
            .is_none());
    }
    assert_eq!(decode_line::<LedgerRow>(&first(LEDGER)).unwrap().version, 0);
    assert_eq!(decode_line::<Audit>(&first(AUDIT)).unwrap().version, 0);
    assert_eq!(decode_line::<PackRef>(&first(PACK)).unwrap().version, 0);
}

#[test]
fn version_two_is_refused() {
    for n in [json!(0), json!(2), json!("1")] {
        assert!(decode_line::<LedgerRow>(&set(LEDGER, "v", n.clone())).is_err());
        assert!(decode_line::<Audit>(&set(AUDIT, "v", n.clone())).is_err());
        assert!(decode_line::<PackRef>(&set(PACK, "v", n)).is_err());
    }
}

#[test]
fn unsupported_version_names_the_value() {
    let err = decode_line::<LedgerRow>(&set(LEDGER, "v", json!(2))).unwrap_err();
    assert!(err.contains("unsupported v 2"), "{err}");
    assert!(err.contains("omitted or 1"), "{err}");
    assert!(err.contains("explicit v 0 is refused"), "{err}");
    let zero = decode_line::<LedgerRow>(&set(LEDGER, "v", json!(0))).unwrap_err();
    assert!(zero.contains("unsupported v 0"), "{zero}");
    assert!(zero.contains("explicit v 0 is refused"), "{zero}");
}

#[test]
fn unknown_field_rejected() {
    let extra = json!(1);
    assert!(decode_line::<LedgerRow>(&set(LEDGER, "extra", extra.clone())).is_err());
    assert!(decode_line::<Audit>(&set(AUDIT, "extra", extra.clone())).is_err());
    assert!(decode_line::<PackRef>(&set(PACK, "extra", extra)).is_err());
}

#[test]
fn unknown_field_inside_check_is_refused() {
    let mut v: serde_json::Value = serde_json::from_str(&first(AUDIT)).unwrap();
    v["checks"][0]["extra"] = json!(1);
    let err = decode_line::<Audit>(&v.to_string()).unwrap_err();
    assert!(err.contains("extra") || err.contains("unknown"), "{err}");
}

#[test]
fn duplicate_keys_are_refused() {
    let row = first(LEDGER);
    let dup_v = format!("{{\"v\":2,\"v\":1,{}", &row[1..]);
    assert!(decode_line::<LedgerRow>(&dup_v).is_err(), "{dup_v}");
    let dup_role = row.replacen("\"role\":", "\"role\":\"Other Fictional\",\"role\":", 1);
    assert!(decode_line::<LedgerRow>(&dup_role).is_err(), "{dup_role}");
}

#[test]
fn unknown_status_or_verdict_refused() {
    assert!(decode_line::<LedgerRow>(&set(LEDGER, "status", json!("ghosted"))).is_err());
    assert!(decode_line::<Audit>(&set(AUDIT, "verdict", json!("maybe"))).is_err());
}

#[test]
fn version_one_round_trips() {
    let row = decode_line::<LedgerRow>(&first(LEDGER)).unwrap();
    let again =
        decode_line::<LedgerRow>(&encode_line(SCHEMA_VERSION, &row.value).unwrap()).unwrap();
    assert_eq!((again.version, again.value), (SCHEMA_VERSION, row.value));
}

#[test]
fn encode_line_writes_v1_and_never_v2() {
    let row = decode_line::<LedgerRow>(&first(LEDGER)).unwrap().value;
    let line = encode_line(SCHEMA_VERSION, &row).unwrap();
    assert!(line.contains("\"v\":1"), "{line}");
    assert!(!line.contains("\"v\":2"), "{line}");
    assert!(encode_line(2, &row).is_err());
}

#[test]
fn submitted_null_submit_time_is_refused() {
    let line = r#"{"company":"Acme Fictional","role":"Widget Engineer","link":"https://jobs.example.test/acme/9","status":"submitted","submit_time":null,"cv_version":"cv-sample-1"}"#;
    let err = decode_line::<LedgerRow>(line).unwrap_err();
    assert!(err.contains("submit_time"), "{err}");
}

#[test]
fn submit_time_link_and_domain() {
    assert!(check_submit_time("2026-10-26T18:59:00Z").is_err());
    assert!(check_submit_time("2026-10-26T18:59:00+01:00").is_ok());
    assert!(check_submit_time("2026-07-01T10:00:00+02:00").is_ok());
    assert_eq!(
        link_key("https://t.example/j/application/"),
        "https://t.example/j"
    );
    assert_eq!(
        canonical_domain(" HTTPS://x@www.Acme.Example.test:443/jobs?x=1#f "),
        "acme.example.test"
    );
}

#[test]
fn exit_code_contract() {
    assert_eq!(audit_exit_code(GateStatus::Pass), 0);
    assert_eq!(audit_exit_code(GateStatus::Flag), 3);
    assert_eq!(audit_exit_code(GateStatus::Block), 4);
    assert_eq!(audit_exit_code(GateStatus::Unaudited), 5);
    assert_eq!(audit_exit_code(GateStatus::Stale), 5);
}

#[test]
fn exit_constants_do_not_collide() {
    let codes = [
        EXIT_PASS,
        EXIT_ERROR,
        EXIT_USAGE,
        EXIT_FLAG,
        EXIT_BLOCK,
        EXIT_UNAUDITED_OR_STALE,
        EXIT_PULSE_REFUSED_WRITE,
    ];
    assert_eq!(codes, [0, 1, 2, 3, 4, 5, 7]);
    let mut uniq = codes.to_vec();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(uniq.len(), codes.len());
}

#[test]
fn exit_code_reverse_map() {
    assert_eq!(audit_exit_status(0), Some(&[GateStatus::Pass][..]));
    assert_eq!(audit_exit_status(3), Some(&[GateStatus::Flag][..]));
    assert_eq!(audit_exit_status(4), Some(&[GateStatus::Block][..]));
    assert_eq!(
        audit_exit_status(5),
        Some(&[GateStatus::Unaudited, GateStatus::Stale][..])
    );
    for c in [1, 2, 6, 255, EXIT_PULSE_REFUSED_WRITE] {
        assert_eq!(audit_exit_status(c), None, "{c}");
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
    match_schema(
        "schemas/ledger-row.v1.json",
        schemars::schema_for!(LedgerRowLine),
    );
    match_schema(
        "schemas/company-audit.v1.json",
        schemars::schema_for!(AuditLine),
    );
    match_schema(
        "schemas/pack-ref.v1.json",
        schemars::schema_for!(PackRefLine),
    );
}

fn match_schema(rel: &str, schema: schemars::schema::RootSchema) {
    let generated = serde_json::to_string_pretty(&schema).unwrap() + "\n";
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    if std::env::var("WRITE_SCHEMA").ok().as_deref() == Some("1") {
        std::fs::write(&path, &generated).unwrap();
    }
    assert_eq!(text(rel), generated, "{rel}");
}

#[test]
fn version_two_fails_line_schema() {
    for (rel, schema) in [
        (
            LEDGER_V1,
            schema_value(schemars::schema_for!(LedgerRowLine)),
        ),
        (AUDIT_V1, schema_value(schemars::schema_for!(AuditLine))),
        (PACK_V1, schema_value(schemars::schema_for!(PackRefLine))),
    ] {
        assert_eq!(schema["properties"]["v"]["const"], json!(1), "{rel}");
        let line = first(rel).replacen("\"v\":1", "\"v\":2", 1);
        let instance: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert!(validate(&schema, &schema, &instance).is_err(), "{rel}");
    }
}

fn schema_value(schema: schemars::schema::RootSchema) -> serde_json::Value {
    serde_json::to_value(schema).unwrap()
}

#[test]
fn line_fields_match_body_types() {
    same_fields(
        schema_value(schemars::schema_for!(LedgerRow)),
        schema_value(schemars::schema_for!(LedgerRowLine)),
    );
    same_fields(
        schema_value(schemars::schema_for!(Audit)),
        schema_value(schemars::schema_for!(AuditLine)),
    );
    same_fields(
        schema_value(schemars::schema_for!(PackRef)),
        schema_value(schemars::schema_for!(PackRefLine)),
    );
}

fn property_names(schema: &serde_json::Value) -> std::collections::BTreeSet<String> {
    schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

fn same_fields(body: serde_json::Value, line: serde_json::Value) {
    let body = property_names(&body);
    let mut line = property_names(&line);
    assert!(line.remove("v"));
    assert_eq!(line, body);
}

#[test]
fn byte_order_mark_is_named() {
    let line = format!("\u{feff}{}", first(LEDGER));
    let err = decode_line::<LedgerRow>(&line).unwrap_err();
    assert!(err.contains("byte-order mark"), "{err}");
}

#[test]
fn v1_fixtures_match_checked_in_schema() {
    check_lines::<LedgerRow>(LEDGER_V1, "schemas/ledger-row.v1.json");
    check_lines::<Audit>(AUDIT_V1, "schemas/company-audit.v1.json");
    check_lines::<PackRef>(PACK_V1, "schemas/pack-ref.v1.json");
}

fn check_lines<T>(lines: &str, schema_rel: &str)
where
    T: serde::de::DeserializeOwned + Checked,
{
    let schema: serde_json::Value = serde_json::from_str(&text(schema_rel)).unwrap();
    for line in text(lines).lines().filter(|l| !l.trim().is_empty()) {
        let instance: serde_json::Value = serde_json::from_str(line).unwrap();
        validate(&schema, &schema, &instance).unwrap();
        assert_eq!(decode_line::<T>(line).unwrap().version, 1);
    }
}

fn validate(
    doc: &serde_json::Value,
    schema: &serde_json::Value,
    instance: &serde_json::Value,
) -> Result<(), String> {
    if let Some(refer) = schema.get("$ref").and_then(|v| v.as_str()) {
        let name = refer.rsplit('/').next().unwrap();
        let def = doc
            .get("definitions")
            .and_then(|d| d.get(name))
            .ok_or_else(|| format!("missing {name}"))?;
        return validate(doc, def, instance);
    }
    check_const(schema, instance)?;
    check_type(schema, instance)?;
    check_enum(schema, instance)?;
    check_object(doc, schema, instance)?;
    check_array(doc, schema, instance)
}

fn check_const(schema: &serde_json::Value, instance: &serde_json::Value) -> Result<(), String> {
    let Some(expected) = schema.get("const") else {
        return Ok(());
    };
    if expected == instance {
        Ok(())
    } else {
        Err(format!("const {instance}"))
    }
}

fn check_type(schema: &serde_json::Value, instance: &serde_json::Value) -> Result<(), String> {
    let Some(ty) = schema.get("type") else {
        return Ok(());
    };
    let ok = match ty {
        serde_json::Value::String(s) => type_matches(s, instance),
        serde_json::Value::Array(xs) => xs
            .iter()
            .any(|t| t.as_str().is_some_and(|s| type_matches(s, instance))),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(format!("type {instance}"))
    }
}

fn type_matches(ty: &str, instance: &serde_json::Value) -> bool {
    match ty {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "boolean" => instance.is_boolean(),
        "null" => instance.is_null(),
        "integer" => instance.as_i64().is_some() || instance.as_u64().is_some(),
        "number" => instance.is_number(),
        _ => false,
    }
}

fn check_enum(schema: &serde_json::Value, instance: &serde_json::Value) -> Result<(), String> {
    let Some(enu) = schema.get("enum").and_then(|v| v.as_array()) else {
        return Ok(());
    };
    if enu.contains(instance) {
        Ok(())
    } else {
        Err(format!("enum {instance}"))
    }
}

fn check_object(
    doc: &serde_json::Value,
    schema: &serde_json::Value,
    instance: &serde_json::Value,
) -> Result<(), String> {
    let Some(obj) = instance.as_object() else {
        return Ok(());
    };
    let props = schema.get("properties").and_then(|p| p.as_object());
    reject_unknown(schema, props, obj)?;
    require_keys(schema, obj)?;
    walk_props(doc, props, obj)
}

fn reject_unknown(
    schema: &serde_json::Value,
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    if schema.get("additionalProperties").and_then(|v| v.as_bool()) != Some(false) {
        return Ok(());
    }
    let Some(props) = props else {
        return Ok(());
    };
    for k in obj.keys() {
        if !props.contains_key(k) {
            return Err(format!("unknown property {k}"));
        }
    }
    Ok(())
}

fn require_keys(
    schema: &serde_json::Value,
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let Some(req) = schema.get("required").and_then(|r| r.as_array()) else {
        return Ok(());
    };
    for k in req {
        let name = k.as_str().unwrap();
        if !obj.contains_key(name) {
            return Err(format!("missing {name}"));
        }
    }
    Ok(())
}

fn walk_props(
    doc: &serde_json::Value,
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let Some(props) = props else {
        return Ok(());
    };
    for (k, v) in obj {
        if let Some(ps) = props.get(k) {
            validate(doc, ps, v)?;
        }
    }
    Ok(())
}

fn check_array(
    doc: &serde_json::Value,
    schema: &serde_json::Value,
    instance: &serde_json::Value,
) -> Result<(), String> {
    let Some(items) = schema.get("items") else {
        return Ok(());
    };
    let Some(arr) = instance.as_array() else {
        return Ok(());
    };
    for el in arr {
        validate(doc, items, el)?;
    }
    Ok(())
}
