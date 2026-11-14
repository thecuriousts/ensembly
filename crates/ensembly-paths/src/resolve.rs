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
const LEGACY_OPS_NAME: &str = "peram-ops.sqlite";
const LEGACY_MEMORY_NAME: &str = "peram-memory.json";

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
    resolve_data_paths(Path::new(""), explicit, None, None).ops_db
}

pub fn resolve_memory_path(explicit: Option<PathBuf>) -> PathBuf {
    resolve_data_paths(Path::new(""), None, explicit, None).memory
}

/// Resolve ops DB and memory together. Existing cwd files win over config; fresh installs use config.
/// Never errors: a missing configured `data_dir` logs a warning and falls back to cwd legacy paths.
pub fn resolve_data_paths(
    root: &Path,
    explicit_db: Option<PathBuf>,
    explicit_memory: Option<PathBuf>,
    config_data: Option<&Path>,
) -> DataPaths {
    let single_flag = explicit_db.is_some() ^ explicit_memory.is_some();
    let effective = if single_flag {
        config_data.filter(|p| p.is_dir())
    } else {
        effective_config_data_dir(config_data)
    };
    let paths = match (&explicit_db, &explicit_memory) {
        (Some(db), Some(mem)) => DataPaths {
            ops_db: db.clone(),
            memory: mem.clone(),
        },
        (Some(db), None) => DataPaths {
            ops_db: db.clone(),
            memory: paired_memory_for_explicit_db(root, db),
        },
        (None, Some(mem)) => DataPaths {
            ops_db: paired_ops_for_explicit_memory(root, mem),
            memory: mem.clone(),
        },
        (None, None) => resolve_implicit(root, effective),
    };
    warn_if_local_shadows_config(
        root,
        effective,
        config_data,
        &paths,
        explicit_db.is_some(),
        explicit_memory.is_some(),
    );
    paths
}

fn effective_config_data_dir(config_data: Option<&Path>) -> Option<&Path> {
    let path = config_data?;
    if path.is_dir() {
        return Some(path);
    }
    let reason = if path.is_file() {
        "is not a directory"
    } else {
        "does not exist"
    };
    eprintln!(
        "warning: paths.data_dir is configured as {} but that path {reason}; using cwd legacy paths (data/local or private/state)",
        path.display()
    );
    None
}

fn resolve_implicit(root: &Path, config_data: Option<&Path>) -> DataPaths {
    if let Some(ops) = find_existing_ops_file(root) {
        return DataPaths {
            ops_db: ops.clone(),
            memory: memory_peer_for_ops(root, &ops, config_data),
        };
    }
    if let Some(mem) = find_existing_memory_file(root) {
        return DataPaths {
            ops_db: default_ops_when_memory_only(root, config_data),
            memory: mem,
        };
    }
    if let Some(data) = config_data {
        return DataPaths {
            ops_db: data.join(CONFIG_OPS_NAME),
            memory: data.join(CONFIG_MEMORY_NAME),
        };
    }
    let ops = resolve_ops_db_at(root, None);
    DataPaths {
        memory: memory_peer_for_ops(root, &ops, None),
        ops_db: ops,
    }
}

/// Single `--db`: existing peer beside flag, else cwd `data/local`, else fresh beside flag (never config).
fn paired_memory_for_explicit_db(root: &Path, db: &Path) -> PathBuf {
    if let Some(peer) = peer_memory_file_in_dir(db.parent()) {
        return peer;
    }
    if let Some(mem) = find_existing_memory_file(root) {
        return mem;
    }
    let paired = db.parent().unwrap_or(root).join(CONFIG_MEMORY_NAME);
    eprintln!(
        "note: --db {} pairs memory with {} (no existing peer beside flag or in cwd data/local)",
        db.display(),
        paired.display()
    );
    paired
}

/// Single `--memory`: existing peer beside flag, else cwd ops paths, else fresh beside flag (never config).
fn paired_ops_for_explicit_memory(root: &Path, mem: &Path) -> PathBuf {
    if let Some(peer) = peer_ops_file_in_dir(mem.parent()) {
        return peer;
    }
    if let Some(ops) = find_existing_ops_file(root) {
        return ops;
    }
    let paired = mem.parent().unwrap_or(root).join(CONFIG_OPS_NAME);
    eprintln!(
        "note: --memory {} pairs ops DB with {} (no existing peer beside flag or in cwd)",
        mem.display(),
        paired.display()
    );
    paired
}

fn default_ops_when_memory_only(root: &Path, config_data: Option<&Path>) -> PathBuf {
    if let Some(data) = config_data {
        return data.join(CONFIG_OPS_NAME);
    }
    resolve_ops_db_at(root, None)
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

fn config_memory_file(cfg: &Path) -> Option<PathBuf> {
    for name in [CONFIG_MEMORY_NAME, LEGACY_MEMORY_NAME] {
        let p = cfg.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Memory beside ops: config memory when only local ops exists (first run), then cwd `data/local/*`, then ops dir.
fn memory_peer_for_ops(root: &Path, ops: &Path, config_data: Option<&Path>) -> PathBuf {
    if find_existing_memory_file(root).is_none() {
        if let Some(cfg) = config_data {
            if let Some(mem) = config_memory_file(cfg) {
                return mem;
            }
        }
    }
    if let Some(mem) = find_existing_memory_file(root) {
        return mem;
    }
    peer_memory_in_dir(ops.parent().unwrap_or(root))
}

fn peer_memory_in_dir(dir: &Path) -> PathBuf {
    let ensembly = dir.join(CONFIG_MEMORY_NAME);
    let legacy = dir.join(LEGACY_MEMORY_NAME);
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
    let legacy = dir.join(LEGACY_MEMORY_NAME);
    if legacy.is_file() {
        return Some(legacy);
    }
    None
}

fn peer_ops_file_in_dir(dir: Option<&Path>) -> Option<PathBuf> {
    let dir = dir?;
    for name in [CONFIG_OPS_NAME, LEGACY_OPS_NAME] {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn warn_if_local_shadows_config(
    root: &Path,
    effective_config: Option<&Path>,
    raw_config: Option<&Path>,
    paths: &DataPaths,
    explicit_db: bool,
    explicit_memory: bool,
) {
    if explicit_db || explicit_memory {
        return;
    }
    let Some(cfg) = raw_config.filter(|p| p.is_dir()).or(effective_config) else {
        return;
    };
    let cfg_ops = cfg.join(CONFIG_OPS_NAME);
    let cfg_mem = cfg.join(CONFIG_MEMORY_NAME);
    let local_ops = find_existing_ops_file(root);
    let local_mem = find_existing_memory_file(root);

    if let Some(ref local) = local_ops {
        if cfg_ops.is_file() && paths.ops_db != cfg_ops && paths.ops_db == *local {
            eprintln!(
                "warning: using cwd ops DB {} instead of configured {}",
                paths.ops_db.display(),
                cfg_ops.display()
            );
        }
        if local_mem.is_none()
            && cfg_mem.is_file()
            && paths.memory == cfg_mem
            && paths.ops_db == *local
        {
            eprintln!(
                "warning: local ops {} with memory from configured {} (no cwd memory file yet)",
                local.display(),
                cfg_mem.display()
            );
        }
    }
    if let Some(ref local) = local_mem {
        if cfg_mem.is_file() && paths.memory != cfg_mem && paths.memory == *local {
            eprintln!(
                "warning: using cwd memory {} instead of configured {}",
                paths.memory.display(),
                cfg_mem.display()
            );
        }
    }
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
