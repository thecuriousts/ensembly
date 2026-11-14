use std::path::{Path, PathBuf};

use thiserror::Error;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPaths {
    pub ops_db: PathBuf,
    pub memory: PathBuf,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ResolveError {
    #[error(
        "paths.data_dir is configured as {} but that directory does not exist; run ensembly init or create it",
        configured.display()
    )]
    ConfigDataDirMissing { configured: PathBuf },
}

pub fn env_alias(primary: &str, legacy: &str) -> Option<String> {
    std::env::var(primary)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var(legacy).ok().filter(|s| !s.is_empty()))
}

pub fn resolve_ops_db(explicit: Option<PathBuf>) -> PathBuf {
    resolve_data_paths(Path::new(""), explicit.clone(), None, None)
        .expect("resolve_ops_db")
        .ops_db
}

pub fn resolve_memory_path(explicit: Option<PathBuf>) -> PathBuf {
    resolve_data_paths(Path::new(""), None, explicit, None)
        .expect("resolve_memory_path")
        .memory
}

pub fn resolve_ops_db_config(explicit: Option<PathBuf>, config_data: Option<&Path>) -> PathBuf {
    resolve_data_paths(Path::new(""), explicit, None, config_data)
        .expect("resolve_ops_db_config")
        .ops_db
}

pub fn resolve_memory_path_config(
    explicit: Option<PathBuf>,
    config_data: Option<&Path>,
) -> PathBuf {
    resolve_data_paths(Path::new(""), None, explicit, config_data)
        .expect("resolve_memory_path_config")
        .memory
}

/// Resolve ops DB and memory together. Existing cwd files win over config; fresh installs use config.
pub fn resolve_data_paths(
    root: &Path,
    explicit_db: Option<PathBuf>,
    explicit_memory: Option<PathBuf>,
    config_data: Option<&Path>,
) -> Result<DataPaths, ResolveError> {
    match (explicit_db, explicit_memory) {
        (Some(db), Some(mem)) => Ok(DataPaths {
            ops_db: db,
            memory: mem,
        }),
        (Some(db), None) => Ok(DataPaths {
            ops_db: db.clone(),
            memory: paired_memory_for_explicit_db(root, &db, config_data)?,
        }),
        (None, Some(mem)) => Ok(DataPaths {
            ops_db: paired_ops_for_explicit_memory(root, &mem, config_data)?,
            memory: mem.clone(),
        }),
        (None, None) => resolve_implicit(root, config_data),
    }
}

fn resolve_implicit(root: &Path, config_data: Option<&Path>) -> Result<DataPaths, ResolveError> {
    if let Some(ops) = find_existing_ops_file(root) {
        return Ok(DataPaths {
            ops_db: ops.clone(),
            memory: memory_peer_for_ops(root, &ops),
        });
    }
    if let Some(mem) = find_existing_memory_file(root) {
        let ops = default_ops_when_memory_only(root, config_data)?;
        return Ok(DataPaths {
            ops_db: ops,
            memory: mem,
        });
    }
    if let Some(data) = config_data {
        if data.is_dir() {
            return Ok(DataPaths {
                ops_db: data.join(CONFIG_OPS_NAME),
                memory: data.join(CONFIG_MEMORY_NAME),
            });
        }
        return Err(ResolveError::ConfigDataDirMissing {
            configured: data.to_path_buf(),
        });
    }
    let ops = resolve_ops_db_at(root, None);
    Ok(DataPaths {
        memory: memory_peer_for_ops(root, &ops),
        ops_db: ops,
    })
}

fn paired_memory_for_explicit_db(
    root: &Path,
    db: &Path,
    config_data: Option<&Path>,
) -> Result<PathBuf, ResolveError> {
    if let Some(peer) = peer_memory_file_in_dir(db.parent()) {
        return Ok(peer);
    }
    if let Some(mem) = find_existing_memory_file(root) {
        return Ok(mem);
    }
    if let Some(data) = config_data {
        if data.is_dir() {
            return Ok(data.join(CONFIG_MEMORY_NAME));
        }
        return Err(ResolveError::ConfigDataDirMissing {
            configured: data.to_path_buf(),
        });
    }
    Ok(db.parent().unwrap_or(root).join(CONFIG_MEMORY_NAME))
}

fn paired_ops_for_explicit_memory(
    root: &Path,
    mem: &Path,
    config_data: Option<&Path>,
) -> Result<PathBuf, ResolveError> {
    if let Some(peer) = peer_ops_file_in_dir(mem.parent()) {
        return Ok(peer);
    }
    if let Some(ops) = find_existing_ops_file(root) {
        return Ok(ops);
    }
    if let Some(data) = config_data {
        if data.is_dir() {
            return Ok(data.join(CONFIG_OPS_NAME));
        }
        return Err(ResolveError::ConfigDataDirMissing {
            configured: data.to_path_buf(),
        });
    }
    Ok(resolve_ops_db_at(root, None))
}

fn default_ops_when_memory_only(
    root: &Path,
    config_data: Option<&Path>,
) -> Result<PathBuf, ResolveError> {
    if let Some(data) = config_data {
        if data.is_dir() {
            return Ok(data.join(CONFIG_OPS_NAME));
        }
        return Err(ResolveError::ConfigDataDirMissing {
            configured: data.to_path_buf(),
        });
    }
    Ok(resolve_ops_db_at(root, None))
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

fn find_existing_memory_file(root: &Path) -> Option<PathBuf> {
    for rel in [DEFAULT_MEMORY_PATH, LEGACY_MEMORY_PATH] {
        let path = root.join(rel);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Memory beside ops: cwd `data/local/*` first (legacy split layout), then ops' directory.
fn memory_peer_for_ops(root: &Path, ops: &Path) -> PathBuf {
    if let Some(mem) = find_existing_memory_file(root) {
        return mem;
    }
    peer_memory_in_dir(ops.parent().unwrap_or(root))
}

fn peer_memory_in_dir(dir: &Path) -> PathBuf {
    let ensembly = dir.join(CONFIG_MEMORY_NAME);
    let legacy = dir.join("peram-memory.json");
    if legacy.is_file() && !ensembly.is_file() {
        return legacy;
    }
    ensembly
}

fn peer_memory_file_in_dir(dir: Option<&Path>) -> Option<PathBuf> {
    let dir = dir?;
    let ensembly = dir.join(CONFIG_MEMORY_NAME);
    if ensembly.is_file() {
        return Some(ensembly);
    }
    let legacy = dir.join("peram-memory.json");
    if legacy.is_file() {
        return Some(legacy);
    }
    None
}

fn peer_ops_file_in_dir(dir: Option<&Path>) -> Option<PathBuf> {
    let dir = dir?;
    for name in [CONFIG_OPS_NAME, "peram-ops.sqlite", "ensembly-ops.sqlite"] {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn resolve_ops_db_at(root: &Path, explicit: Option<PathBuf>) -> PathBuf {
    if let Some(p) = explicit {
        return p;
    }
    if let Some(path) = find_existing_ops_file(root) {
        return path;
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
    if let Some(path) = find_existing_memory_file(root) {
        return path;
    }
    root.join(DEFAULT_MEMORY_PATH)
}
