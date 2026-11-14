//! `paths.data_dir` from init config is honored when CLI flags are omitted.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use ensembly_kernel::{config, paths};

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
    let data_dir = config::read_paths_data_dir().expect("config data_dir");
    let ops = paths::resolve_ops_db_config(None, Some(&data_dir));
    let mem = paths::resolve_memory_path_config(None, Some(&data_dir));
    assert_eq!(ops, xdg_data.join("ensembly").join("ensembly-ops.sqlite"));
    assert_eq!(mem, xdg_data.join("ensembly").join("ensembly-memory.json"));
}
