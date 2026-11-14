use chrono::Utc;
use ensembly_bundle::{verify_bundle, BundleManifest, BUNDLE_FORMAT};
use std::fs;
use tempfile::tempdir;

#[test]
fn verify_fails_on_extra_file() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("bundle");
    fs::create_dir_all(&root).unwrap();
    let m = BundleManifest {
        bundle_format: BUNDLE_FORMAT.to_string(),
        ensembly_version: "t".into(),
        created_at: Utc::now(),
        sources: vec![],
        files: vec![],
    };
    m.write(&root).unwrap();
    fs::write(root.join("stray.txt"), b"extra").unwrap();
    let v = verify_bundle(&root);
    assert!(!v.ok);
    assert!(v.errors.iter().any(|e| e.contains("extra file")));
}
