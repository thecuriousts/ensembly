//! Single-flag pairing must not touch configured data_dir peers.

use ensembly_paths::resolve_data_paths;
use serial_test::serial;
use std::fs;

#[serial]
#[test]
fn db_flag_does_not_use_config_memory() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("share/ensembly");
    fs::create_dir_all(&config).unwrap();
    let cfg_mem = config.join("ensembly-memory.json");
    fs::write(&cfg_mem, b"real").unwrap();
    fs::create_dir_all(root.path().join("data/local")).unwrap();
    fs::write(root.path().join("data/local/ensembly-memory.json"), b"cwd").unwrap();

    let scratch_db = root.path().join("scratch/x.sqlite");
    fs::create_dir_all(scratch_db.parent().unwrap()).unwrap();

    let got = resolve_data_paths(root.path(), Some(scratch_db.clone()), None, Some(&config));
    assert_eq!(got.ops_db, scratch_db);
    assert_eq!(
        got.memory,
        scratch_db.parent().unwrap().join("ensembly-memory.json")
    );
    assert_ne!(got.memory, cfg_mem);
    assert_ne!(
        got.memory,
        root.path().join("data/local/ensembly-memory.json")
    );
}

#[serial]
#[test]
fn memory_flag_does_not_use_config_or_cwd_ops() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("share/ensembly");
    fs::create_dir_all(&config).unwrap();
    let cfg_ops = config.join("ensembly-ops.sqlite");
    fs::write(&cfg_ops, b"real").unwrap();
    fs::create_dir_all(root.path().join("data/local")).unwrap();
    fs::write(root.path().join("data/local/ensembly-ops.sqlite"), b"cwd").unwrap();

    let scratch_mem = root.path().join("scratch/m.json");
    fs::create_dir_all(scratch_mem.parent().unwrap()).unwrap();

    let got = resolve_data_paths(root.path(), None, Some(scratch_mem.clone()), Some(&config));
    assert_eq!(got.memory, scratch_mem);
    assert_eq!(
        got.ops_db,
        scratch_mem.parent().unwrap().join("ensembly-ops.sqlite")
    );
    assert_ne!(got.ops_db, cfg_ops);
    assert_ne!(
        got.ops_db,
        root.path().join("data/local/ensembly-ops.sqlite")
    );
}
