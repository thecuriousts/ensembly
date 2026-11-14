use chrono::{TimeZone, Utc};
use ensembly_bundle::{
    export_bundle, verify_bundle, BackupSource, BackupSourceKind, BundleManifest,
    EnsemblyStorePaths, ExportOpts, DEFAULT_MAX_FILE_MB,
};
use std::fs;
use tempfile::tempdir;

fn synthetic_ops_bytes() -> Vec<u8> {
    br#"{"format":"ensembly-ops-bundle-v1","schema_version":1,"exported_at":"2026-01-01T00:00:00Z","kv":[]}"#.to_vec()
}

fn export_with_source(
    dir: &tempfile::TempDir,
    force_include: Vec<String>,
    setup: impl FnOnce(&std::path::Path),
) -> (std::path::PathBuf, BundleManifest) {
    let ops_tmp = dir.path().join("ops.json");
    fs::write(&ops_tmp, synthetic_ops_bytes()).unwrap();
    let shared = dir.path().join("vault");
    fs::create_dir_all(&shared).unwrap();
    setup(&shared);
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
            force_include,
            line: 1,
        }],
    })
    .unwrap();
    let manifest: BundleManifest =
        serde_json::from_slice(&fs::read(out.join("MANIFEST.json")).unwrap()).unwrap();
    (out, manifest)
}

#[test]
fn force_include_exports_token_named_file() {
    let dir = tempdir().unwrap();
    let (out, manifest) = export_with_source(&dir, vec!["api-token.txt".into()], |shared| {
        fs::write(shared.join("api-token.txt"), b"fixture-token-value").unwrap();
    });
    let v = verify_bundle(&out);
    assert!(v.ok, "{:?}", v.errors);
    let bundled = out.join("shared/vault/api-token.txt");
    assert!(bundled.is_file());
    let entry = manifest
        .files
        .iter()
        .find(|f| f.path == "shared/vault/api-token.txt")
        .expect("manifest entry");
    assert!(entry.excluded.is_none());
}

#[test]
fn token_named_file_excluded_without_force_include() {
    let dir = tempdir().unwrap();
    let (out, manifest) = export_with_source(&dir, vec![], |shared| {
        fs::write(shared.join("api-token.txt"), b"fixture-token-value").unwrap();
    });
    assert!(
        !out.join("shared/vault/api-token.txt").exists(),
        "default *token* exclude must not copy file"
    );
    let entry = manifest
        .files
        .iter()
        .find(|f| f.path == "shared/vault/api-token.txt")
        .expect("manifest should list excluded token file");
    assert_eq!(entry.excluded, Some(true));
    assert!(
        entry.exclude_reason.as_deref() == Some("default-exclude-glob"),
        "{:?}",
        entry.exclude_reason
    );
}

#[test]
fn force_include_does_not_bypass_secret_content_scan() {
    let dir = tempdir().unwrap();
    let (out, manifest) = export_with_source(&dir, vec!["notes.txt".into()], |shared| {
        fs::write(
            shared.join("notes.txt"),
            b"ignore this ghp_abcdefghijklmnopqrstuvwxyz1234567890",
        )
        .unwrap();
    });
    assert!(!out.join("shared/vault/notes.txt").exists());
    let entry = manifest
        .files
        .iter()
        .find(|f| f.path == "shared/vault/notes.txt")
        .expect("manifest entry");
    assert_eq!(entry.excluded, Some(true));
    assert!(
        entry
            .exclude_reason
            .as_deref()
            .is_some_and(|r| r.contains("secret pattern")),
        "{:?}",
        entry.exclude_reason
    );

    let dir2 = tempdir().unwrap();
    let (_out2, manifest2) = export_with_source(&dir2, vec!["private.pem".into()], |shared| {
        fs::write(
            shared.join("private.pem"),
            b"-----BEGIN PRIVATE KEY-----\nfixture\n",
        )
        .unwrap();
    });
    let entry2 = manifest2
        .files
        .iter()
        .find(|f| f.path == "shared/vault/private.pem")
        .expect("manifest entry");
    assert_eq!(entry2.excluded, Some(true));
    assert!(
        entry2
            .exclude_reason
            .as_deref()
            .is_some_and(|r| r.contains("secret pattern")),
        "{:?}",
        entry2.exclude_reason
    );
}
