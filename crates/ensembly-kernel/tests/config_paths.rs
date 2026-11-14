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
    let cfg_mem = xdg_data.join("ensembly/ensembly-memory.json");
    fs::write(&cfg_mem, b"{}").unwrap();

    std::env::set_var("HOME", &home);
    std::env::set_var("XDG_CONFIG_HOME", &xdg_cfg);
    let data_dir = load_paths_data_dir().unwrap();
    let got = resolve_data_paths(t.path(), None, None, data_dir.as_deref());
    assert_eq!(got.ops_db, local_ops);
    assert_eq!(got.memory, cfg_mem);
}

#[serial]
#[test]
fn malformed_config_blocks_runtime_not_version() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().to_path_buf();
    let cfg_dir = home.join("cfg/ensembly");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(cfg_dir.join("config.toml"), "schema = 1\n[paths\n").unwrap();

    let env_home = |cmd: &mut Command| {
        cmd.env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join("cfg"));
    };

    let o = {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ensembly"));
        c.arg("version");
        env_home(&mut c);
        c.output().unwrap()
    };
    assert_eq!(o.status.code(), Some(0), "version: {:?}", o);

    let o = {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ensembly"));
        c.arg("doctor");
        env_home(&mut c);
        c.output().unwrap()
    };
    assert_ne!(o.status.code(), Some(0), "doctor should report config");
    let out = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(out.contains("operator config"), "{out}");

    let o = {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ensembly"));
        c.arg("runtime").arg("status");
        env_home(&mut c);
        c.output().unwrap()
    };
    assert_eq!(o.status.code(), Some(2), "runtime: {:?}", o);
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("config:"), "{err}");

    let o = {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ensembly"));
        c.arg("init").arg("--force");
        env_home(&mut c);
        c.output().unwrap()
    };
    assert_eq!(o.status.code(), Some(0), "init --force: {:?}", o);
}

#[serial]
#[test]
fn single_db_flag_skips_malformed_config() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().to_path_buf();
    let cfg_dir = home.join("cfg/ensembly");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(cfg_dir.join("config.toml"), "schema = 1\n[paths\n").unwrap();
    let scratch = t.path().join("only.sqlite");

    let o = Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .args(["--db", scratch.to_str().unwrap(), "version"])
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join("cfg"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{:?}", o);
}

#[serial]
#[test]
fn both_flags_skip_malformed_config_load() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().to_path_buf();
    let cfg_dir = home.join("cfg/ensembly");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(cfg_dir.join("config.toml"), "schema = 1\n[paths\n").unwrap();
    let scratch = t.path().join("pair.sqlite");
    let scratch_mem = t.path().join("pair.json");

    let o = Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .args([
            "--db",
            scratch.to_str().unwrap(),
            "--memory",
            scratch_mem.to_str().unwrap(),
            "version",
        ])
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join("cfg"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{:?}", o);
}

#[serial]
#[test]
fn unset_home_means_no_config_not_error() {
    std::env::remove_var("HOME");
    std::env::remove_var("XDG_CONFIG_HOME");
    assert_eq!(load_paths_data_dir().unwrap(), None);
}
