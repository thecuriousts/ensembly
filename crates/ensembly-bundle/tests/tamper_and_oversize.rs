use chrono::{TimeZone, Utc};
use ensembly_bundle::{
    export_bundle, verify_bundle, BackupSource, BackupSourceKind, EnsemblyStorePaths, ExportOpts,
    DEFAULT_MAX_FILE_MB,
};
use std::fs::{self, OpenOptions};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn tampered_file_fails_verify() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("shared-fix");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.md"), b"ok").unwrap();
    let out = dir.path().join("bundle");
    export_bundle(ExportOpts {
        out: out.clone(),
        force_new_subdir: false,
        max_file_mb: DEFAULT_MAX_FILE_MB,
        ensembly_version: "test".into(),
        created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        stores: EnsemblyStorePaths::default(),
        user_sources: vec![BackupSource {
            name: "docs".into(),
            section: "shared".into(),
            path: src,
            kind: BackupSourceKind::Dir,
            include: vec!["**/*".into()],
            exclude: vec![],
            line: 1,
        }],
    })
    .unwrap();
    let p = out.join("shared/docs/a.md");
    fs::write(&p, b"tampered").unwrap();
    let v = verify_bundle(&out);
    assert!(!v.ok);
}

#[test]
fn oversize_file_skipped_in_manifest() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("big");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("small.txt"), b"x").unwrap();
    let big = src.join("huge.bin");
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .open(&big)
        .unwrap();
    let chunk = vec![0u8; 1024 * 1024];
    for _ in 0..3 {
        f.write_all(&chunk).unwrap();
    }
    drop(f);
    let out = dir.path().join("bundle");
    export_bundle(ExportOpts {
        out: out.clone(),
        force_new_subdir: false,
        max_file_mb: 2,
        ensembly_version: "test".into(),
        created_at: Utc::now(),
        stores: EnsemblyStorePaths::default(),
        user_sources: vec![BackupSource {
            name: "blobs".into(),
            section: "shared".into(),
            path: src,
            kind: BackupSourceKind::Dir,
            include: vec!["**/*".into()],
            exclude: vec![],
            line: 1,
        }],
    })
    .unwrap();
    assert!(!out.join("shared/blobs/huge.bin").exists());
    let m = ensembly_bundle::BundleManifest::read(&out).unwrap();
    assert!(m
        .files
        .iter()
        .any(|f| f.exclude_reason.as_deref() == Some("oversize>2mb")));
}