use chrono::{TimeZone, Utc};
use ensembly_bundle::{
    build_evil_tar, build_symlink_tar, join_bundle, pack_bundle, unpack_tar_to_bundle, JoinOpts,
    PackOpts,
};
use ensembly_bundle::{
    export_bundle, verify_bundle, BackupSource, BackupSourceKind, EnsemblyStorePaths, ExportOpts,
    PartsManifest, DEFAULT_MAX_FILE_MB,
};
use std::fs;
use tempfile::tempdir;

fn synthetic_ops_bytes() -> Vec<u8> {
    let life = serde_json::json!({"graph": {"nodes": {}, "edges": []}});
    let bundle = serde_json::json!({
        "format": "ensembly-ops-bundle-v1",
        "schema_version": 1,
        "exported_at": "2026-01-01T00:00:00Z",
        "kv": [{"key": "life_state", "value": life.to_string(), "updated_at": "2026-01-01T00:00:00Z"}]
    });
    serde_json::to_vec(&bundle).unwrap()
}

fn export_min_bundle(
    dir: &std::path::Path,
    name: &str,
    extra: impl FnOnce(&std::path::Path),
) -> std::path::PathBuf {
    let ops_tmp = dir.join("ops.json");
    fs::write(&ops_tmp, synthetic_ops_bytes()).unwrap();
    let shared = dir.join("shared-src");
    fs::create_dir_all(&shared).unwrap();
    fs::write(shared.join("note.md"), b"hello").unwrap();
    extra(&shared);
    let out = dir.join(name);
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
            name: "docs".into(),
            section: "shared".into(),
            path: shared,
            kind: BackupSourceKind::Dir,
            include: vec!["**/*".into()],
            exclude: vec![],
            line: 1,
        }],
    })
    .unwrap();
    out
}

#[test]
fn pack_join_roundtrip() {
    let dir = tempdir().unwrap();
    let bundle = export_min_bundle(dir.path(), "bundle-fixture", |_| {});
    let parts_dir = dir.path().join("parts");
    let joined = dir.path().join("joined");
    pack_bundle(PackOpts {
        bundle_dir: bundle.clone(),
        out: parts_dir.clone(),
        split_mb: 16,
    })
    .unwrap();
    join_bundle(JoinOpts {
        parts_json: parts_dir.join("bundle-fixture.parts.json"),
        out: joined.clone(),
    })
    .unwrap();
    assert!(verify_bundle(&joined).ok);
    let a = fs::read(bundle.join("MANIFEST.json")).unwrap();
    let b = fs::read(joined.join("MANIFEST.json")).unwrap();
    assert_eq!(a, b);
}

#[test]
fn each_part_within_split_limit() {
    let dir = tempdir().unwrap();
    let bundle = export_min_bundle(dir.path(), "big-bundle", |shared| {
        let blob = vec![b'x'; 2 * 1024 * 1024 + 512 * 1024];
        fs::write(shared.join("blob.bin"), blob).unwrap();
    });
    let parts_dir = dir.path().join("parts");
    let report = pack_bundle(PackOpts {
        bundle_dir: bundle,
        out: parts_dir.clone(),
        split_mb: 1,
    })
    .unwrap();
    assert!(report.part_count >= 3);
    let manifest = PartsManifest::read(&report.parts_json).unwrap();
    let limit = 1024 * 1024;
    for part in &manifest.parts {
        assert!(part.bytes <= limit, "part {} too large", part.file);
    }
}

#[test]
fn join_refuses_tampered_part() {
    let dir = tempdir().unwrap();
    let bundle = export_min_bundle(dir.path(), "bundle-fixture", |_| {});
    let parts_dir = dir.path().join("parts");
    pack_bundle(PackOpts {
        bundle_dir: bundle,
        out: parts_dir.clone(),
        split_mb: 16,
    })
    .unwrap();
    let part = parts_dir.join("bundle-fixture.tar.part001");
    fs::write(&part, b"tamper").unwrap();
    let err = join_bundle(JoinOpts {
        parts_json: parts_dir.join("bundle-fixture.parts.json"),
        out: dir.path().join("joined"),
    })
    .unwrap_err();
    assert!(
        err.contains("checksum") || err.contains("mismatch"),
        "{err}"
    );
    assert!(!dir.path().join("joined/MANIFEST.json").exists());
}

#[test]
fn join_refuses_missing_part() {
    let dir = tempdir().unwrap();
    let bundle = export_min_bundle(dir.path(), "bundle-fixture", |shared| {
        fs::write(shared.join("pad.bin"), vec![b'z'; 2 * 1024 * 1024]).unwrap();
    });
    let parts_dir = dir.path().join("parts");
    let packed = pack_bundle(PackOpts {
        bundle_dir: bundle,
        out: parts_dir.clone(),
        split_mb: 1,
    })
    .unwrap();
    assert!(packed.part_count >= 2);
    let _ = fs::remove_file(parts_dir.join("bundle-fixture.tar.part002"));
    let err = join_bundle(JoinOpts {
        parts_json: parts_dir.join("bundle-fixture.parts.json"),
        out: dir.path().join("joined"),
    })
    .unwrap_err();
    assert!(err.contains("missing"), "{err}");
}

#[test]
fn join_refuses_extra_part() {
    let dir = tempdir().unwrap();
    let bundle = export_min_bundle(dir.path(), "bundle-fixture", |_| {});
    let parts_dir = dir.path().join("parts");
    pack_bundle(PackOpts {
        bundle_dir: bundle,
        out: parts_dir.clone(),
        split_mb: 16,
    })
    .unwrap();
    fs::write(parts_dir.join("bundle-fixture.tar.part999"), b"extra").unwrap();
    let err = join_bundle(JoinOpts {
        parts_json: parts_dir.join("bundle-fixture.parts.json"),
        out: dir.path().join("joined"),
    })
    .unwrap_err();
    assert!(err.contains("extra part"), "{err}");
}

#[test]
fn join_refuses_swapped_part_bytes() {
    let dir = tempdir().unwrap();
    let bundle = export_min_bundle(dir.path(), "bundle-fixture", |shared| {
        let blob = vec![b'y'; 17 * 1024 * 1024];
        fs::write(shared.join("pad.bin"), blob).unwrap();
    });
    let parts_dir = dir.path().join("parts");
    pack_bundle(PackOpts {
        bundle_dir: bundle,
        out: parts_dir.clone(),
        split_mb: 16,
    })
    .unwrap();
    let p1 = fs::read(parts_dir.join("bundle-fixture.tar.part001")).unwrap();
    let p2 = fs::read(parts_dir.join("bundle-fixture.tar.part002")).unwrap();
    fs::write(parts_dir.join("bundle-fixture.tar.part001"), &p2).unwrap();
    fs::write(parts_dir.join("bundle-fixture.tar.part002"), &p1).unwrap();
    let err = join_bundle(JoinOpts {
        parts_json: parts_dir.join("bundle-fixture.parts.json"),
        out: dir.path().join("joined"),
    })
    .unwrap_err();
    assert!(err.contains("mismatch"), "{err}");
}

#[test]
fn join_refuses_non_empty_target() {
    let dir = tempdir().unwrap();
    let bundle = export_min_bundle(dir.path(), "bundle-fixture", |_| {});
    let parts_dir = dir.path().join("parts");
    pack_bundle(PackOpts {
        bundle_dir: bundle,
        out: parts_dir.clone(),
        split_mb: 16,
    })
    .unwrap();
    let target = dir.path().join("occupied");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("marker"), b"x").unwrap();
    let err = join_bundle(JoinOpts {
        parts_json: parts_dir.join("bundle-fixture.parts.json"),
        out: target,
    })
    .unwrap_err();
    assert!(err.contains("non-empty"), "{err}");
}

#[test]
fn unpack_refuses_dotdot_tar() {
    let dir = tempdir().unwrap();
    let mut raw = Vec::new();
    build_evil_tar(&mut raw).expect("craft dotdot tar");
    let tar = dir.path().join("evil.tar");
    fs::write(&tar, raw).unwrap();
    let out = dir.path().join("out");
    let err = unpack_tar_to_bundle(&tar, &out).unwrap_err();
    assert!(err.contains("..") || err.contains("forbidden"), "{err}");
}

#[test]
fn unpack_refuses_symlink_tar() {
    let dir = tempdir().unwrap();
    let mut raw = Vec::new();
    build_symlink_tar(&mut raw).unwrap();
    let tar = dir.path().join("link.tar");
    fs::write(&tar, raw).unwrap();
    let err = unpack_tar_to_bundle(&tar, &dir.path().join("out")).unwrap_err();
    assert!(err.contains("symlink"), "{err}");
}
