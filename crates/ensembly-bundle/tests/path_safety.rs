use ensembly_bundle::{
    restore_bundle, verify_bundle, BundleManifest, FileEntry, RestoreOpts, BUNDLE_FORMAT,
};
use std::fs;
use std::os::unix::fs::symlink;
use tempfile::tempdir;

fn minimal_manifest(files: Vec<FileEntry>) -> BundleManifest {
    BundleManifest {
        bundle_format: BUNDLE_FORMAT.to_string(),
        ensembly_version: "test".into(),
        created_at: chrono::Utc::now(),
        sources: vec![],
        files,
    }
}

#[test]
fn verify_rejects_traversal_in_manifest() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("bundle");
    fs::create_dir_all(root.join("ensembly/ops")).unwrap();
    fs::write(root.join("ensembly/ops/ops-bundle.json"), b"{}").unwrap();
    let m = minimal_manifest(vec![FileEntry {
        path: "../outside/secret".into(),
        sha256: String::new(),
        bytes: 0,
        row_count: None,
        section: "shared".into(),
        source: "x".into(),
        excluded: None,
        exclude_reason: None,
        view: None,
    }]);
    m.write(&root).unwrap();
    let v = verify_bundle(&root);
    assert!(!v.ok, "{:?}", v.errors);
    assert!(v
        .errors
        .iter()
        .any(|e| e.contains("..") || e.contains("unsafe")));
}

#[test]
fn verify_rejects_absolute_manifest_path() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("bundle");
    fs::create_dir_all(&root).unwrap();
    let m = minimal_manifest(vec![FileEntry {
        path: "/etc/passwd".into(),
        sha256: String::new(),
        bytes: 0,
        row_count: None,
        section: "shared".into(),
        source: "x".into(),
        excluded: None,
        exclude_reason: None,
        view: None,
    }]);
    m.write(&root).unwrap();
    assert!(!verify_bundle(&root).ok);
}

#[test]
fn restore_refuses_symlink_escape() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("bundle");
    let outside = dir.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("owned.txt"), b"outside").unwrap();
    fs::create_dir_all(root.join("shared/docs")).unwrap();
    fs::write(root.join("shared/docs/ok.txt"), b"ok").unwrap();
    let entry = FileEntry {
        path: "shared/docs/ok.txt".into(),
        sha256: String::new(),
        bytes: 2,
        row_count: None,
        section: "shared".into(),
        source: "docs".into(),
        excluded: None,
        exclude_reason: None,
        view: None,
    };
    let mut m = minimal_manifest(vec![entry.clone()]);
    m.write(&root).unwrap();
    // fix checksum by running verify would fail - skip hash by marking excluded false and use real hash from file - simpler: hand compute not needed; restore calls verify which fails on checksum.
    // Build manifest via export is heavy; patch manifest after computing hash
    let bytes = fs::read(root.join("shared/docs/ok.txt")).unwrap();
    let hash = ensembly_bundle::hash_bytes(&bytes);
    m.files[0].sha256 = hash.sha256;
    m.files[0].bytes = hash.bytes;
    m.write(&root).unwrap();
    assert!(verify_bundle(&root).ok);
    let target = dir.path().join("restore");
    fs::create_dir_all(target.join("shared")).unwrap();
    symlink(&outside, target.join("shared/docs")).unwrap();
    let err = restore_bundle(
        &root,
        &RestoreOpts {
            into_ops: None,
            into_memory: None,
            sources_to: Some(target),
            merge: false,
            dry_run: false,
        },
    )
    .unwrap_err();
    assert!(err.contains("symlink"), "{err}");
}
