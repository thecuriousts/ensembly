//! Layout × config × flags: existing users keep the same resolved paths.

use ensembly_paths::{
    resolve_data_paths, DEFAULT_MEMORY_PATH, DEFAULT_OPS_PATH, LEGACY_MEMORY_PATH, LEGACY_OPS_PATH,
    LEGACY_OPS_PRIVATE_PATH,
};
use serial_test::serial;
use std::fs;
use std::path::{Path, PathBuf};

type SeedFn = fn(&Path);

struct MatrixCase {
    name: &'static str,
    seed: SeedFn,
    config: Option<ConfigMode>,
    explicit_db: Option<&'static str>,
    explicit_memory: Option<&'static str>,
    expect_ops: &'static str,
    expect_memory: &'static str,
}

enum ConfigMode {
    Present,
    Missing,
}

fn seed_empty(_: &Path) {}

fn seed_new_local_pair(root: &Path) {
    fs::create_dir_all(root.join("data/local")).unwrap();
    fs::write(root.join(DEFAULT_OPS_PATH), b"o").unwrap();
    fs::write(root.join(DEFAULT_MEMORY_PATH), b"m").unwrap();
}

fn seed_legacy_local_pair(root: &Path) {
    fs::create_dir_all(root.join("data/local")).unwrap();
    fs::write(root.join(LEGACY_OPS_PATH), b"o").unwrap();
    fs::write(root.join(LEGACY_MEMORY_PATH), b"m").unwrap();
}

fn seed_private_state_legacy_ops_data_local_mem(root: &Path) {
    fs::create_dir_all(root.join("private/state")).unwrap();
    fs::create_dir_all(root.join("data/local")).unwrap();
    fs::write(root.join(LEGACY_OPS_PRIVATE_PATH), b"o").unwrap();
    fs::write(root.join(LEGACY_MEMORY_PATH), b"m").unwrap();
}

fn seed_private_state_new_ops_data_local_mem(root: &Path) {
    fs::create_dir_all(root.join("private/state")).unwrap();
    fs::create_dir_all(root.join("data/local")).unwrap();
    fs::write(root.join("private/state/ensembly-ops.sqlite"), b"o").unwrap();
    fs::write(root.join(DEFAULT_MEMORY_PATH), b"m").unwrap();
}

fn seed_private_state_ops_only(root: &Path) {
    fs::create_dir_all(root.join("private/state")).unwrap();
    fs::write(root.join(LEGACY_OPS_PRIVATE_PATH), b"o").unwrap();
}

fn seed_mcp_only_new_memory(root: &Path) {
    fs::create_dir_all(root.join("data/local")).unwrap();
    fs::write(root.join(DEFAULT_MEMORY_PATH), b"{}").unwrap();
}

fn seed_mcp_only_legacy_memory(root: &Path) {
    fs::create_dir_all(root.join("data/local")).unwrap();
    fs::write(root.join(LEGACY_MEMORY_PATH), b"{}").unwrap();
}

fn seed_local_ops_only(root: &Path) {
    fs::create_dir_all(root.join("data/local")).unwrap();
    fs::write(root.join(DEFAULT_OPS_PATH), b"o").unwrap();
}

fn seed_ops_in_both_folders(root: &Path) {
    fs::create_dir_all(root.join("data/local")).unwrap();
    fs::create_dir_all(root.join("private/state")).unwrap();
    fs::write(root.join(DEFAULT_OPS_PATH), b"local").unwrap();
    fs::write(root.join("private/state/ensembly-ops.sqlite"), b"priv").unwrap();
}

fn seed_mixed_new_ops_legacy_memory(root: &Path) {
    fs::create_dir_all(root.join("data/local")).unwrap();
    fs::write(root.join(DEFAULT_OPS_PATH), b"o").unwrap();
    fs::write(root.join(LEGACY_MEMORY_PATH), b"m").unwrap();
}

fn config_path(root: &Path, mode: &ConfigMode) -> PathBuf {
    match mode {
        ConfigMode::Present => root.join("share/ensembly"),
        ConfigMode::Missing => root.join("share/missing"),
    }
}

fn ensure_config(root: &Path, mode: &ConfigMode) {
    if matches!(mode, ConfigMode::Present) {
        fs::create_dir_all(config_path(root, mode)).unwrap();
    }
}

#[serial]
#[test]
fn layout_flag_config_matrix() {
    let cases = [
        MatrixCase {
            name: "fresh cwd no config",
            seed: seed_empty,
            config: None,
            explicit_db: None,
            explicit_memory: None,
            expect_ops: DEFAULT_OPS_PATH,
            expect_memory: DEFAULT_MEMORY_PATH,
        },
        MatrixCase {
            name: "new local pair",
            seed: seed_new_local_pair,
            config: None,
            explicit_db: None,
            explicit_memory: None,
            expect_ops: DEFAULT_OPS_PATH,
            expect_memory: DEFAULT_MEMORY_PATH,
        },
        MatrixCase {
            name: "legacy local pair",
            seed: seed_legacy_local_pair,
            config: None,
            explicit_db: None,
            explicit_memory: None,
            expect_ops: LEGACY_OPS_PATH,
            expect_memory: LEGACY_MEMORY_PATH,
        },
        MatrixCase {
            name: "private/state legacy ops + data/local legacy memory",
            seed: seed_private_state_legacy_ops_data_local_mem,
            config: Some(ConfigMode::Present),
            explicit_db: None,
            explicit_memory: None,
            expect_ops: LEGACY_OPS_PRIVATE_PATH,
            expect_memory: LEGACY_MEMORY_PATH,
        },
        MatrixCase {
            name: "private/state new ops + data/local new memory",
            seed: seed_private_state_new_ops_data_local_mem,
            config: Some(ConfigMode::Present),
            explicit_db: None,
            explicit_memory: None,
            expect_ops: "private/state/ensembly-ops.sqlite",
            expect_memory: DEFAULT_MEMORY_PATH,
        },
        MatrixCase {
            name: "private/state ops only no config",
            seed: seed_private_state_ops_only,
            config: None,
            explicit_db: None,
            explicit_memory: None,
            expect_ops: LEGACY_OPS_PRIVATE_PATH,
            expect_memory: "private/state/ensembly-memory.json",
        },
        MatrixCase {
            name: "private/state ops only missing config",
            seed: seed_private_state_ops_only,
            config: Some(ConfigMode::Missing),
            explicit_db: None,
            explicit_memory: None,
            expect_ops: LEGACY_OPS_PRIVATE_PATH,
            expect_memory: "private/state/ensembly-memory.json",
        },
        MatrixCase {
            name: "ops in data/local and private/state",
            seed: seed_ops_in_both_folders,
            config: None,
            explicit_db: None,
            explicit_memory: None,
            expect_ops: DEFAULT_OPS_PATH,
            expect_memory: DEFAULT_MEMORY_PATH,
        },
        MatrixCase {
            name: "mixed new ops legacy memory",
            seed: seed_mixed_new_ops_legacy_memory,
            config: None,
            explicit_db: None,
            explicit_memory: None,
            expect_ops: DEFAULT_OPS_PATH,
            expect_memory: LEGACY_MEMORY_PATH,
        },
        MatrixCase {
            name: "MCP-only new memory + config",
            seed: seed_mcp_only_new_memory,
            config: Some(ConfigMode::Present),
            explicit_db: None,
            explicit_memory: None,
            expect_ops: "share/ensembly/ensembly-ops.sqlite",
            expect_memory: DEFAULT_MEMORY_PATH,
        },
        MatrixCase {
            name: "MCP-only legacy memory + config",
            seed: seed_mcp_only_legacy_memory,
            config: Some(ConfigMode::Present),
            explicit_db: None,
            explicit_memory: None,
            expect_ops: "share/ensembly/ensembly-ops.sqlite",
            expect_memory: LEGACY_MEMORY_PATH,
        },
        MatrixCase {
            name: "MCP-only new memory + missing config",
            seed: seed_mcp_only_new_memory,
            config: Some(ConfigMode::Missing),
            explicit_db: None,
            explicit_memory: None,
            expect_ops: DEFAULT_OPS_PATH,
            expect_memory: DEFAULT_MEMORY_PATH,
        },
        MatrixCase {
            name: "local ops only + missing config",
            seed: seed_local_ops_only,
            config: Some(ConfigMode::Missing),
            explicit_db: None,
            explicit_memory: None,
            expect_ops: DEFAULT_OPS_PATH,
            expect_memory: DEFAULT_MEMORY_PATH,
        },
        MatrixCase {
            name: "fresh install config data_dir only",
            seed: seed_empty,
            config: Some(ConfigMode::Present),
            explicit_db: None,
            explicit_memory: None,
            expect_ops: "share/ensembly/ensembly-ops.sqlite",
            expect_memory: "share/ensembly/ensembly-memory.json",
        },
        MatrixCase {
            name: "missing config falls back to data/local",
            seed: seed_empty,
            config: Some(ConfigMode::Missing),
            explicit_db: None,
            explicit_memory: None,
            expect_ops: DEFAULT_OPS_PATH,
            expect_memory: DEFAULT_MEMORY_PATH,
        },
        MatrixCase {
            name: "local ops wins over config",
            seed: seed_local_ops_only,
            config: Some(ConfigMode::Present),
            explicit_db: None,
            explicit_memory: None,
            expect_ops: DEFAULT_OPS_PATH,
            expect_memory: DEFAULT_MEMORY_PATH,
        },
        MatrixCase {
            name: "--db alone pairs memory from config",
            seed: seed_empty,
            config: Some(ConfigMode::Present),
            explicit_db: Some("share/ensembly/ensembly-ops.sqlite"),
            explicit_memory: None,
            expect_ops: "share/ensembly/ensembly-ops.sqlite",
            expect_memory: "share/ensembly/ensembly-memory.json",
        },
        MatrixCase {
            name: "--db alone missing config uses legacy default memory",
            seed: seed_empty,
            config: Some(ConfigMode::Missing),
            explicit_db: Some("custom/ops.sqlite"),
            explicit_memory: None,
            expect_ops: "custom/ops.sqlite",
            expect_memory: "custom/ensembly-memory.json",
        },
        MatrixCase {
            name: "--memory alone pairs ops from config",
            seed: seed_empty,
            config: Some(ConfigMode::Present),
            explicit_db: None,
            explicit_memory: Some("share/ensembly/ensembly-memory.json"),
            expect_ops: "share/ensembly/ensembly-ops.sqlite",
            expect_memory: "share/ensembly/ensembly-memory.json",
        },
        MatrixCase {
            name: "--memory alone missing config uses peer beside flagged file",
            seed: seed_empty,
            config: Some(ConfigMode::Missing),
            explicit_db: None,
            explicit_memory: Some("custom/mem.json"),
            expect_ops: "custom/ensembly-ops.sqlite",
            expect_memory: "custom/mem.json",
        },
        MatrixCase {
            name: "--memory alone MCP-only layout missing config",
            seed: seed_mcp_only_new_memory,
            config: Some(ConfigMode::Missing),
            explicit_db: None,
            explicit_memory: Some("custom/mem.json"),
            expect_ops: "custom/ensembly-ops.sqlite",
            expect_memory: "custom/mem.json",
        },
        MatrixCase {
            name: "both flags explicit",
            seed: seed_empty,
            config: Some(ConfigMode::Present),
            explicit_db: Some("custom/a.sqlite"),
            explicit_memory: Some("custom/b.json"),
            expect_ops: "custom/a.sqlite",
            expect_memory: "custom/b.json",
        },
        MatrixCase {
            name: "both flags missing config",
            seed: seed_empty,
            config: Some(ConfigMode::Missing),
            explicit_db: Some("custom/a.sqlite"),
            explicit_memory: Some("custom/b.json"),
            expect_ops: "custom/a.sqlite",
            expect_memory: "custom/b.json",
        },
        MatrixCase {
            name: "--db alone uses cwd data/local memory",
            seed: seed_mcp_only_new_memory,
            config: Some(ConfigMode::Present),
            explicit_db: Some("custom/ops.sqlite"),
            explicit_memory: None,
            expect_ops: "custom/ops.sqlite",
            expect_memory: DEFAULT_MEMORY_PATH,
        },
        MatrixCase {
            name: "--memory alone uses cwd local ops",
            seed: seed_local_ops_only,
            config: Some(ConfigMode::Present),
            explicit_db: None,
            explicit_memory: Some("share/ensembly/ensembly-memory.json"),
            expect_ops: DEFAULT_OPS_PATH,
            expect_memory: "share/ensembly/ensembly-memory.json",
        },
        MatrixCase {
            name: "--db private/state ops uses data/local legacy memory",
            seed: seed_private_state_legacy_ops_data_local_mem,
            config: Some(ConfigMode::Present),
            explicit_db: Some(LEGACY_OPS_PRIVATE_PATH),
            explicit_memory: None,
            expect_ops: LEGACY_OPS_PRIVATE_PATH,
            expect_memory: LEGACY_MEMORY_PATH,
        },
        MatrixCase {
            name: "--memory data/local uses private/state legacy ops",
            seed: seed_private_state_legacy_ops_data_local_mem,
            config: Some(ConfigMode::Present),
            explicit_db: None,
            explicit_memory: Some(LEGACY_MEMORY_PATH),
            expect_ops: LEGACY_OPS_PRIVATE_PATH,
            expect_memory: LEGACY_MEMORY_PATH,
        },
        MatrixCase {
            name: "--db scratch no peers creates beside flag",
            seed: seed_empty,
            config: Some(ConfigMode::Present),
            explicit_db: Some("scratch/x.sqlite"),
            explicit_memory: None,
            expect_ops: "scratch/x.sqlite",
            expect_memory: "scratch/ensembly-memory.json",
        },
    ];

    for case in cases {
        let root = tempfile::tempdir().unwrap();
        (case.seed)(root.path());
        let config = case.config.as_ref().map(|m| {
            ensure_config(root.path(), m);
            config_path(root.path(), m)
        });
        let db = case.explicit_db.map(|rel| {
            let p = root.path().join(rel);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            p
        });
        let mem = case.explicit_memory.map(|rel| {
            let p = root.path().join(rel);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            p
        });

        let got = resolve_data_paths(root.path(), db, mem, config.as_deref());
        assert_eq!(
            got.ops_db,
            root.path().join(case.expect_ops),
            "{} ops_db",
            case.name
        );
        assert_eq!(
            got.memory,
            root.path().join(case.expect_memory),
            "{} memory",
            case.name
        );
    }
}
