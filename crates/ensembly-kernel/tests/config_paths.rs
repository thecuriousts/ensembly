//! `paths.data_dir` from init config is honored when CLI flags are omitted.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use ensembly_paths::{load_paths_data_dir, resolve_data_paths, DEFAULT_OPS_PATH};
use serial_test::serial;

fn run_init(home: &PathBuf, xdg_cfg: &PathBuf, xdg_data: &PathBuf) {
    let o = Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .arg("init")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", xdg_cfg)
        .env("XDG_DATA_HOME", xdg_data)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{:?}", o);
}

#[serial]
#[test]
fn resolve_ops_and_memory_from_config_data_dir() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().to_path_buf();
    let xdg_cfg = home.join("cfg");
    let xdg_data = home.join("share");
    fs::create_dir_all(&xdg_cfg).unwrap();
    run_init(&home, &xdg_cfg, &xdg_data);

    std::env::set_var("HOME", &home);
    std::env::set_var("XDG_CONFIG_HOME", &xdg_cfg);
    let data_dir = load_paths_data_dir().expect("config data_dir");
    let got = resolve_data_paths(t.path(), None, None, data_dir.as_deref());
    assert_eq!(got.ops_db, xdg_data.join("ensembly/ensembly-ops.sqlite"));
    assert_eq!(got.memory, xdg_data.join("ensembly/ensembly-memory.json"));
}

#[serial]
#[test]
fn existing_local_ops_wins_over_config_data_dir() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let xdg_cfg = home.join("cfg");
    let xdg_data = home.join("share");
    fs::create_dir_all(&xdg_cfg).unwrap();
    run_init(&home, &xdg_cfg, &xdg_data);

    fs::create_dir_all(t.path().join("data/local")).unwrap();
    let local_ops = t.path().join(DEFAULT_OPS_PATH);
    fs::write(&local_ops, b"sqlite").unwrap();

    std::env::set_var("HOME", &home);
    std::env::set_var("XDG_CONFIG_HOME", &xdg_cfg);
    let data_dir = load_paths_data_dir().unwrap();
    let got = resolve_data_paths(t.path(), None, None, data_dir.as_deref());
    assert_eq!(got.ops_db, local_ops);
    assert_eq!(got.memory, t.path().join("data/local/ensembly-memory.json"));
}

#[serial]
#[test]
fn malformed_config_exits_nonzero() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().to_path_buf();
    let cfg_dir = home.join("cfg/ensembly");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(cfg_dir.join("config.toml"), "schema = 1\n[paths\n").unwrap();

    let o = Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .arg("version")
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join("cfg"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2), "{:?}", o);
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("config:"), "{err}");
    assert!(
        err.contains("invalid TOML") || err.contains("TOML"),
        "{err}"
    );
    assert!(
        err.contains("config.toml") || err.contains("ensembly/config"),
        "{err}"
    );
}

#[serial]
#[test]
fn unset_home_means_no_config_not_error() {
    std::env::remove_var("HOME");
    std::env::remove_var("XDG_CONFIG_HOME");
    assert_eq!(load_paths_data_dir().unwrap(), None);
}
