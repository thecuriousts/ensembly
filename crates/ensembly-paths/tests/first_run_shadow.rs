//! First-run: local ops + configured memory on disk.

use ensembly_paths::{resolve_data_paths, DEFAULT_OPS_PATH};
use serial_test::serial;
use std::fs;

#[serial]
#[test]
fn local_ops_uses_config_memory_when_only_ops_in_cwd() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("share/ensembly");
    fs::create_dir_all(&config).unwrap();
    fs::create_dir_all(root.path().join("data/local")).unwrap();
    let local_ops = root.path().join(DEFAULT_OPS_PATH);
    fs::write(&local_ops, b"ops").unwrap();
    let cfg_mem = config.join("ensembly-memory.json");
    fs::write(&cfg_mem, b"{}").unwrap();

    let got = resolve_data_paths(root.path(), None, None, Some(&config));
    assert_eq!(got.ops_db, local_ops);
    assert_eq!(got.memory, cfg_mem);
}
