use chrono::{TimeZone, Utc};
use ensembly_bundle::{
    export_bundle, verify_bundle, BackupSource, BackupSourceKind, EnsemblyStorePaths, ExportOpts,
    DEFAULT_MAX_FILE_MB,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn force_include_exports_token_named_file() {
    let dir = tempdir().unwrap();
    let ops_tmp = dir.path().join("ops.json");
    fs::write(
        &ops_tmp,
        br#"{"format":"ensembly-ops-bundle-v1","schema_version":1,"exported_at":"2026-01-01T00:00:00Z","kv":[]}"#,
    )
    .unwrap();
    let shared = dir.path().join("vault");
    fs::create_dir_all(&shared).unwrap();
    fs::write(shared.join("api-token.txt"), b"fixture-token-value").unwrap();
    let out = dir.path().join("bundle");
    export_bundle(ExportOpts {
        out: out.clone(),
        force_new_subdir: false,
        max_file_mb: DEFAULT_MAX_FILE_MB,
        ensembly_version: "test".into(),
        created_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
        stores: EnsemblyStorePaths {
            ops_json: Some(ops_tmp),
            memory_json: None,
            ledger_jsonl: None,
        },
        user_sources: vec![BackupSource {
            name: "vault".into(),
            section: "shared".into(),
            path: shared,
            kind: BackupSourceKind::Dir,
            include: vec!["**/*".into()],
            exclude: vec![],
            force_include: vec!["api-token.txt".into()],
            line: 1,
        }],
    })
    .unwrap();
    let v = verify_bundle(&out);
    assert!(v.ok, "{:?}", v.errors);
    let bundled = out.join("shared/vault/api-token.txt");
    assert!(bundled.is_file());
}
