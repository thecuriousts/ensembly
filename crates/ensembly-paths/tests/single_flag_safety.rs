//! Single-flag pairing must not use config data_dir but must reuse cwd peers (B2).

use ensembly_paths::{
    resolve_data_paths, DEFAULT_MEMORY_PATH, LEGACY_MEMORY_PATH, LEGACY_OPS_PRIVATE_PATH,
};
use serial_test::serial;
use std::fs;

#[serial]
#[test]
fn db_flag_uses_cwd_memory_not_config() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("share/ensembly");
    fs::create_dir_all(&config).unwrap();
    let cfg_mem = config.join("ensembly-memory.json");
    fs::write(&cfg_mem, b"real").unwrap();
    fs::create_dir_all(root.path().join("data/local")).unwrap();
    let cwd_mem = root.path().join(DEFAULT_MEMORY_PATH);
    fs::write(&cwd_mem, b"cwd").unwrap();

    let scratch_db = root.path().join("scratch/x.sqlite");
    fs::create_dir_all(scratch_db.parent().unwrap()).unwrap();

    let got = resolve_data_paths(root.path(), Some(scratch_db.clone()), None, Some(&config));
    assert_eq!(got.ops_db, scratch_db);
    assert_eq!(got.memory, cwd_mem);
    assert_ne!(got.memory, cfg_mem);
}

#[serial]
#[test]
fn memory_flag_uses_cwd_ops_not_config() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("share/ensembly");
    fs::create_dir_all(&config).unwrap();
    let cfg_ops = config.join("ensembly-ops.sqlite");
    fs::write(&cfg_ops, b"real").unwrap();
    fs::create_dir_all(root.path().join("private/state")).unwrap();
    let cwd_ops = root.path().join(LEGACY_OPS_PRIVATE_PATH);
    fs::write(&cwd_ops, b"cwd").unwrap();

    let scratch_mem = root.path().join("data/local/m.json");
    fs::create_dir_all(scratch_mem.parent().unwrap()).unwrap();

    let got = resolve_data_paths(root.path(), None, Some(scratch_mem.clone()), Some(&config));
    assert_eq!(got.memory, scratch_mem);
    assert_eq!(got.ops_db, cwd_ops);
    assert_ne!(got.ops_db, cfg_ops);
}

#[serial]
#[test]
fn scratch_db_creates_peer_beside_flag_without_config() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("share/ensembly");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("ensembly-memory.json"), b"config").unwrap();
    fs::write(config.join("ensembly-ops.sqlite"), b"config").unwrap();

    let scratch_db = root.path().join("scratch/x.sqlite");
    fs::create_dir_all(scratch_db.parent().unwrap()).unwrap();

    let got = resolve_data_paths(root.path(), Some(scratch_db.clone()), None, Some(&config));
    assert_eq!(got.ops_db, scratch_db);
    assert_eq!(
        got.memory,
        scratch_db.parent().unwrap().join("ensembly-memory.json")
    );
    assert_ne!(got.memory, config.join("ensembly-memory.json"));
}

#[serial]
#[test]
fn db_flag_private_state_ops_uses_data_local_memory() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("private/state")).unwrap();
    fs::create_dir_all(root.path().join("data/local")).unwrap();
    let ops = root.path().join(LEGACY_OPS_PRIVATE_PATH);
    fs::write(&ops, b"o").unwrap();
    let mem = root.path().join(LEGACY_MEMORY_PATH);
    fs::write(&mem, b"m").unwrap();

    let got = resolve_data_paths(root.path(), Some(ops.clone()), None, None);
    assert_eq!(got.ops_db, ops);
    assert_eq!(got.memory, mem);
}
