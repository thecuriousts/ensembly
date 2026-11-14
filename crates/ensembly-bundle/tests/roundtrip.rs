use chrono::{TimeZone, Utc};
use ensembly_bundle::{
    export_bundle, restore_bundle, verify_bundle, BackupSource, BackupSourceKind,
    EnsemblyStorePaths, ExportOpts, RestoreOpts, DEFAULT_MAX_FILE_MB,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn roundtrip_sources_to_preserves_section_layout() {
    let dir = tempdir().unwrap();
    let shared = dir.path().join("shared-fix");
    fs::create_dir_all(&shared).unwrap();
    fs::write(shared.join("page.html"), b"<html>fixture</html>").unwrap();
    let out = dir.path().join("bundle");
    export_bundle(ExportOpts {
        out: out.clone(),
        force_new_subdir: false,
        max_file_mb: DEFAULT_MAX_FILE_MB,
        ensembly_version: "test".into(),
        created_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
        stores: EnsemblyStorePaths::default(),
        user_sources: vec![BackupSource {
            name: "pages".into(),
            section: "life".into(),
            path: shared,
            kind: BackupSourceKind::Dir,
            include: vec!["**/*".into()],
            exclude: vec![],
            force_include: vec![],
            line: 1,
        }],
    })
    .unwrap();
    let v = verify_bundle(&out);
    assert!(v.ok, "{:?}", v.errors);
    let dest = dir.path().join("restored");
    let plan = restore_bundle(
        &out,
        &RestoreOpts {
            into_ops: None,
            into_memory: None,
            sources_to: Some(dest.clone()),
            merge: false,
            dry_run: false,
            assume_verified: false,
        },
    )
    .unwrap();
    assert!(!plan.copies.is_empty());
    assert!(dest.join("life/pages/page.html").is_file());
}
