use chrono::{TimeZone, Utc};
use ensembly_bundle::{
    export_bundle, merge_ops_bundles, restore_bundle, verify_bundle, BackupSource,
    BackupSourceKind, BundleManifest, EnsemblyStorePaths, ExportOpts, KvRow, OpsBundleMerge,
    RestoreOpts, DEFAULT_MAX_FILE_MB,
};
use sha2::{Digest, Sha256};
use std::fs;
use tempfile::tempdir;

fn synthetic_ops_bytes() -> Vec<u8> {
    let life = serde_json::json!({
        "graph": {
            "nodes": {
                "t1": {"id":"t1","title":"Plan","status":"open","kind":"todo"},
                "c1": {
                    "id":"c1",
                    "title":"Sweep",
                    "status":"open",
                    "kind":"chore",
                    "deadline_at": "2026-06-01T09:00:00Z"
                }
            },
            "edges": []
        }
    });
    let bundle = serde_json::json!({
        "format": "ensembly-ops-bundle-v1",
        "schema_version": 1,
        "exported_at": "2026-01-01T00:00:00Z",
        "kv": [{
            "key": "life_state",
            "value": life.to_string(),
            "updated_at": "2026-01-01T00:00:00Z"
        }]
    });
    serde_json::to_vec(&bundle).unwrap()
}

fn export_fixture(
    dir: &std::path::Path,
    ts: chrono::DateTime<Utc>,
    out_name: &str,
) -> std::path::PathBuf {
    let ops_tmp = dir.join("ops.json");
    fs::write(&ops_tmp, synthetic_ops_bytes()).unwrap();
    let shared = dir.join("src-shared");
    fs::create_dir_all(&shared).unwrap();
    fs::write(shared.join("ok.md"), b"ok").unwrap();
    let secret = dir.join("src-secret");
    fs::create_dir_all(&secret).unwrap();
    fs::write(
        secret.join("leak.pem"),
        b"-----BEGIN RSA PRIVATE KEY-----\nfake\n",
    )
    .unwrap();
    let work = dir.join("src-work");
    fs::create_dir_all(&work).unwrap();
    fs::write(work.join("w.csv"), b"a,b\n1,2").unwrap();
    let out = dir.join(out_name);
    export_bundle(ExportOpts {
        out: out.clone(),
        force_new_subdir: false,
        max_file_mb: DEFAULT_MAX_FILE_MB,
        ensembly_version: "0.3.0-test".into(),
        created_at: ts,
        stores: EnsemblyStorePaths {
            ops_json: Some(ops_tmp),
            memory_json: None,
            ledger_jsonl: None,
        },
        user_sources: vec![
            BackupSource {
                name: "docs".into(),
                section: "shared".into(),
                path: shared,
                kind: BackupSourceKind::Dir,
                include: vec!["**/*".into()],
                exclude: vec![],
                line: 1,
            },
            BackupSource {
                name: "vault".into(),
                section: "shared".into(),
                path: secret,
                kind: BackupSourceKind::Dir,
                include: vec!["**/*".into()],
                exclude: vec![],
                line: 2,
            },
            BackupSource {
                name: "sheet".into(),
                section: "work".into(),
                path: work,
                kind: BackupSourceKind::Dir,
                include: vec!["**/*".into()],
                exclude: vec![],
                line: 3,
            },
        ],
    })
    .unwrap();
    out
}

fn dir_hash(root: &std::path::Path) -> String {
    let mut paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    fn walk(dir: &std::path::Path, acc: &mut Vec<std::path::PathBuf>) {
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_file() {
                    acc.push(p);
                } else if p.is_dir() {
                    walk(&p, acc);
                }
            }
        }
    }
    paths.clear();
    walk(root, &mut paths);
    paths.sort();
    let mut h = Sha256::new();
    for p in paths {
        if p.file_name().and_then(|s| s.to_str()) == Some("MANIFEST.json") {
            continue;
        }
        h.update(fs::read(&p).unwrap());
    }
    hex::encode(h.finalize())
}

#[test]
fn byte_equal_repeat_export() {
    let dir = tempdir().unwrap();
    let ts = Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap();
    let a = export_fixture(dir.path(), ts, "bundle-a");
    let b = export_fixture(dir.path(), ts, "bundle-b");
    assert_eq!(dir_hash(&a), dir_hash(&b));
}

#[test]
fn derived_views_present_and_valid() {
    let dir = tempdir().unwrap();
    let ts = Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap();
    let root = export_fixture(dir.path(), ts, "bundle-views");
    let todos = fs::read(root.join("ensembly/views/todos.csv")).unwrap();
    let chores = fs::read(root.join("ensembly/views/chores.csv")).unwrap();
    let ics = fs::read(root.join("ensembly/views/deadlines.ics")).unwrap();
    assert!(std::str::from_utf8(&todos).unwrap().contains("t1,Plan"));
    assert!(std::str::from_utf8(&chores).unwrap().contains("c1,Sweep"));
    let ics_s = std::str::from_utf8(&ics).unwrap();
    assert!(ics_s.contains("BEGIN:VCALENDAR"));
    assert!(ics_s.contains("BEGIN:VEVENT"));
    let m = BundleManifest::read(&root).unwrap();
    assert!(m
        .files
        .iter()
        .any(|f| f.view == Some(true) && f.path.contains("todos.csv")));
}

#[test]
fn secret_fixture_excluded() {
    let dir = tempdir().unwrap();
    let root = export_fixture(
        dir.path(),
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        "bundle-secret",
    );
    assert!(!root.join("shared/vault/leak.pem").exists());
    let m = BundleManifest::read(&root).unwrap();
    assert!(m
        .files
        .iter()
        .any(|f| f.excluded == Some(true) && f.path.contains("leak.pem")));
}

#[test]
fn restore_no_overwrite() {
    let dir = tempdir().unwrap();
    let root = export_fixture(
        dir.path(),
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        "bundle-no-overwrite",
    );
    let dest = dir.path().join("restore");
    let target = dest.join("shared/docs/ok.md");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"existing").unwrap();
    let err = restore_bundle(
        &root,
        &RestoreOpts {
            into_ops: None,
            into_memory: None,
            sources_to: Some(dest),
            merge: false,
            dry_run: false,
        },
    )
    .unwrap_err();
    assert!(err.contains("refusing overwrite"), "{err}");
}

#[test]
fn merge_ops_dedup_never_drops() {
    let a = OpsBundleMerge {
        format: "ensembly-ops-bundle-v1".into(),
        schema_version: 1,
        exported_at: "t".into(),
        kv: vec![
            KvRow {
                key: "k1".into(),
                value: "1".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
            KvRow {
                key: "k2".into(),
                value: "2".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
        ],
    };
    let b = OpsBundleMerge {
        format: "ensembly-ops-bundle-v1".into(),
        schema_version: 1,
        exported_at: "t2".into(),
        kv: vec![KvRow {
            key: "k3".into(),
            value: "3".into(),
            updated_at: "2026-02-01T00:00:00Z".into(),
        }],
    };
    let m = merge_ops_bundles(&a, &b);
    assert_eq!(m.kv.len(), 3);
}

#[test]
fn tamper_still_fails_after_views() {
    let dir = tempdir().unwrap();
    let root = export_fixture(
        dir.path(),
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        "bundle-tamper",
    );
    fs::write(root.join("ensembly/views/todos.csv"), b"tamper\n").unwrap();
    assert!(!verify_bundle(&root).ok);
}
