use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn bad_backup_sources_config_exits_2() {
    let dir = tempdir().unwrap();
    let cfg_dir = dir.path().join("config/ensembly");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(
        cfg_dir.join("config.toml"),
        r#"
[[backup.sources]]
name = "bad.name"
section = "shared"
path = "~/x"
kind = "dir"
"#,
    )
    .unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("config"))
        .args(["export", "--out", dir.path().join("out").to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(2),
        "stderr={}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn restore_into_refuses_unverified_bundle() {
    let dir = tempdir().unwrap();
    let bundle = dir.path().join("bundle");
    fs::create_dir_all(bundle.join("ensembly/ops")).unwrap();
    fs::write(bundle.join("ensembly/ops/ops-bundle.json"), b"not-json").unwrap();
    let m = r#"{
  "bundle_format": "ensembly-portable-bundle-v1",
  "ensembly_version": "t",
  "created_at": "2026-01-01T00:00:00Z",
  "sources": [],
  "files": [{
    "path": "ensembly/ops/ops-bundle.json",
    "sha256": "00",
    "bytes": 1,
    "section": "ensembly",
    "source": "ops"
  }]
}"#;
    fs::write(bundle.join("MANIFEST.json"), m).unwrap();
    let target = dir.path().join("data");
    fs::create_dir_all(&target).unwrap();
    let sentinel = target.join("ensembly-ops.sqlite");
    fs::write(&sentinel, b"untouched").unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .current_dir(dir.path())
        .args([
            "restore",
            bundle.to_str().unwrap(),
            "--into",
            target.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(fs::read(&sentinel).unwrap(), b"untouched");
}
