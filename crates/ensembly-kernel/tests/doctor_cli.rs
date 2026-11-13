//! `ensembly doctor` on a clean profile, offline, and with broken state.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn doctor(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .args(["--no-memory", "--db"])
        .arg(home.join("ops.sqlite"))
        .arg("doctor")
        .args(args)
        .env("HOME", home)
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        .env("ENSEMBLY_DOCTOR_REMOTE", "https://127.0.0.1:9/none.git")
        .env_remove("ENSEMBLY_OFFLINE")
        .current_dir(home)
        .output()
        .expect("run doctor")
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn installed(home: &Path, path: &Path) {
    let d = home.join(".claude/plugins");
    fs::create_dir_all(&d).unwrap();
    let body = serde_json::json!({"version": 2, "plugins": {"flowingstack@local": [
        {"scope": "user", "installPath": path, "version": "0.1.0"}]}});
    fs::write(d.join("installed_plugins.json"), body.to_string()).unwrap();
}

#[test]
fn clean_profile_unreachable_remote_exits_zero() {
    let t = tempfile::tempdir().unwrap();
    let o = doctor(t.path(), &[]);
    let s = text(&o);
    assert_eq!(o.status.code(), Some(0), "{s}");
    assert!(
        s.starts_with(&format!("ensembly-cli {}\n", env!("CARGO_PKG_VERSION"))),
        "{s}"
    );
    for line in [
        "harness claude-code absent",
        "harness cursor absent",
        "ops-db absent",
        "latest offline",
    ] {
        assert!(s.lines().any(|l| l == line), "missing {line:?} in {s}");
    }
    assert!(!t.path().join("ops.sqlite").exists());
}

#[test]
fn offline_flag_skips_lookup_and_json_parses() {
    let t = tempfile::tempdir().unwrap();
    let o = doctor(t.path(), &["--offline", "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["latest"], "skipped");
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(v["failures"].as_array().unwrap().len(), 0);
}

#[test]
fn installed_plugins_are_listed() {
    let t = tempfile::tempdir().unwrap();
    installed(t.path(), t.path());
    let cursor = t
        .path()
        .join(".cursor/plugins/local/flowingstack/.cursor-plugin");
    fs::create_dir_all(&cursor).unwrap();
    fs::write(
        cursor.join("plugin.json"),
        r#"{"name":"flowingstack","version":"0.1.0"}"#,
    )
    .unwrap();
    let o = doctor(t.path(), &["--offline"]);
    let s = text(&o);
    assert_eq!(o.status.code(), Some(0), "{s}");
    assert!(s
        .lines()
        .any(|l| l == "plugin claude-code flowingstack@local 0.1.0"));
    assert!(s.lines().any(|l| l == "plugin cursor flowingstack 0.1.0"));
}

#[test]
fn missing_install_path_exits_one() {
    let t = tempfile::tempdir().unwrap();
    installed(t.path(), &t.path().join("gone"));
    let o = doctor(t.path(), &["--offline"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
}

#[test]
fn unreadable_installed_plugins_exits_one() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir_all(t.path().join(".claude/plugins")).unwrap();
    fs::write(t.path().join(".claude/plugins/installed_plugins.json"), "{").unwrap();
    let o = doctor(t.path(), &["--offline"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("harness claude-code broken"));
}

#[test]
fn corrupt_ops_db_exits_one() {
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("ops.sqlite"), "not a database").unwrap();
    let o = doctor(t.path(), &["--offline"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("ops-db broken"));
}

#[test]
fn bad_flag_exits_two() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(doctor(t.path(), &["--nope"]).status.code(), Some(2));
}
