use ensembly_bundle::load_backup_sources;
use std::fs;
use tempfile::tempdir;

#[test]
fn duplicate_name_same_section_exits_config_error() {
    let dir = tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    fs::write(
        &cfg,
        r#"
[[backup.sources]]
name = "vault"
section = "work"
path = "fixture-a"
kind = "dir"

[[backup.sources]]
name = "vault"
section = "work"
path = "fixture-b"
kind = "dir"
"#,
    )
    .unwrap();
    std::env::set_var("HOME", dir.path());
    let err = load_backup_sources(&cfg).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("duplicate"), "{msg}");
    assert!(msg.contains("vault"));
    assert!(
        msg.contains(":9:"),
        "second duplicate must cite its line: {msg}"
    );
}

#[test]
fn invalid_source_name() {
    let dir = tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    fs::write(
        &cfg,
        r#"
[[backup.sources]]
name = "bad.name"
section = "shared"
path = "~/notes"
kind = "dir"
"#,
    )
    .unwrap();
    std::env::set_var("HOME", dir.path());
    let err = load_backup_sources(&cfg).unwrap_err();
    assert!(err.to_string().contains("invalid source name"));
}

#[test]
fn invalid_section_name() {
    let dir = tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    fs::write(
        &cfg,
        r#"
[[backup.sources]]
name = "notes"
section = "life.backup"
path = "~/notes"
kind = "dir"
"#,
    )
    .unwrap();
    std::env::set_var("HOME", dir.path());
    let err = load_backup_sources(&cfg).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("invalid section"), "{msg}");
}
