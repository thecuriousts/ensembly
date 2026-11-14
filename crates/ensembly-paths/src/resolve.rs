use std::path::{Path, PathBuf};

pub const DEFAULT_OPS_PATH: &str = "data/local/ensembly-ops.sqlite";
pub const DEFAULT_OPS_PRIVATE_PATH: &str = "private/state/ensembly-ops.sqlite";
pub const LEGACY_OPS_PATH: &str = "data/local/peram-ops.sqlite";
pub const LEGACY_OPS_PRIVATE_PATH: &str = "private/state/peram-ops.sqlite";

pub const DEFAULT_MEMORY_PATH: &str = "data/local/ensembly-memory.json";
pub const LEGACY_MEMORY_PATH: &str = "data/local/peram-memory.json";

pub const DEFAULT_AGENT_ID: &str = "ensembly-swarm";
pub const LEGACY_AGENT_ID: &str = "peram-swarm";

const CONFIG_OPS_NAME: &str = "ensembly-ops.sqlite";
const CONFIG_MEMORY_NAME: &str = "ensembly-memory.json";

/// Resolved ops DB and memory paths chosen together (no split layouts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPaths {
    pub ops_db: PathBuf,
    pub memory: PathBuf,
}

pub fn env_alias(primary: &str, legacy: &str) -> Option<String> {
    std::env::var(primary)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var(legacy).ok().filter(|s| !s.is_empty()))
}

pub fn resolve_ops_db(explicit: Option<PathBuf>) -> PathBuf {
    resolve_data_paths(Path::new(""), explicit.clone(), None, None).ops_db
}

pub fn resolve_memory_path(explicit: Option<PathBuf>) -> PathBuf {
    resolve_data_paths(Path::new(""), None, explicit, None).memory
}

pub fn resolve_ops_db_config(explicit: Option<PathBuf>, config_data: Option<&Path>) -> PathBuf {
    resolve_data_paths(Path::new(""), explicit, None, config_data).ops_db
}

pub fn resolve_memory_path_config(
    explicit: Option<PathBuf>,
    config_data: Option<&Path>,
) -> PathBuf {
    resolve_data_paths(Path::new(""), None, explicit, config_data).memory
}

/// One decision for implicit paths: explicit flags win; else an existing cwd ops
/// file pairs memory in the same directory; else config `data_dir`; else cwd defaults.
pub fn resolve_data_paths(
    root: &Path,
    explicit_db: Option<PathBuf>,
    explicit_memory: Option<PathBuf>,
    config_data: Option<&Path>,
) -> DataPaths {
    if explicit_db.is_some() || explicit_memory.is_some() {
        return DataPaths {
            ops_db: resolve_ops_db_at(root, explicit_db),
            memory: resolve_memory_path_at(root, explicit_memory),
        };
    }

    if let Some(ops) = find_existing_ops_file(root) {
        return DataPaths {
            ops_db: ops.clone(),
            memory: memory_peer_for_ops(root, &ops),
        };
    }

    if let Some(data) = config_data.filter(|d| d.is_dir()) {
        return DataPaths {
            ops_db: data.join(CONFIG_OPS_NAME),
            memory: data.join(CONFIG_MEMORY_NAME),
        };
    }

    let ops = resolve_ops_db_at(root, None);
    DataPaths {
        memory: memory_peer_for_ops(root, &ops),
        ops_db: ops,
    }
}

fn find_existing_ops_file(root: &Path) -> Option<PathBuf> {
    for rel in [
        DEFAULT_OPS_PATH,
        DEFAULT_OPS_PRIVATE_PATH,
        LEGACY_OPS_PATH,
        LEGACY_OPS_PRIVATE_PATH,
    ] {
        let path = root.join(rel);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn memory_peer_for_ops(root: &Path, ops: &Path) -> PathBuf {
    let dir = ops.parent().unwrap_or(root);
    let ensembly = dir.join(CONFIG_MEMORY_NAME);
    let legacy = dir.join("peram-memory.json");
    if legacy.is_file() && !ensembly.is_file() {
        return legacy;
    }
    ensembly
}

pub fn resolve_ops_db_at(root: &Path, explicit: Option<PathBuf>) -> PathBuf {
    if let Some(p) = explicit {
        return p;
    }
    let existing = [
        DEFAULT_OPS_PATH,
        DEFAULT_OPS_PRIVATE_PATH,
        LEGACY_OPS_PATH,
        LEGACY_OPS_PRIVATE_PATH,
    ];
    for rel in existing {
        let path = root.join(rel);
        if path.is_file() {
            return path;
        }
    }
    let create = [DEFAULT_OPS_PATH, DEFAULT_OPS_PRIVATE_PATH];
    for rel in create {
        let path = root.join(rel);
        if path.parent().map(|p| p.exists()).unwrap_or(false) {
            return path;
        }
    }
    root.join(DEFAULT_OPS_PATH)
}

pub fn resolve_memory_path_at(root: &Path, explicit: Option<PathBuf>) -> PathBuf {
    if let Some(p) = explicit {
        return p;
    }
    let ensembly = root.join(DEFAULT_MEMORY_PATH);
    if ensembly.is_file() {
        return ensembly;
    }
    let legacy = root.join(LEGACY_MEMORY_PATH);
    if legacy.is_file() {
        return legacy;
    }
    ensembly
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn existing_local_ops_pairs_memory_in_same_dir_not_config() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("data/local")).unwrap();
        let ops = dir.path().join(DEFAULT_OPS_PATH);
        fs::write(&ops, b"sqlite").unwrap();
        let config = dir.path().join("xdg-data/ensembly");
        fs::create_dir_all(&config).unwrap();
        let got = resolve_data_paths(dir.path(), None, None, Some(&config));
        assert_eq!(got.ops_db, ops);
        assert_eq!(
            got.memory,
            dir.path().join("data/local/ensembly-memory.json")
        );
    }

    #[test]
    fn config_dir_used_when_no_local_ops_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("share/ensembly");
        fs::create_dir_all(&config).unwrap();
        let got = resolve_data_paths(dir.path(), None, None, Some(&config));
        assert_eq!(got.ops_db, config.join(CONFIG_OPS_NAME));
        assert_eq!(got.memory, config.join(CONFIG_MEMORY_NAME));
    }
}
