//! Env discover: prefer `ENSEMBLY_*`, accept `PERAM_*` as one-release aliases.

use std::path::{Path, PathBuf};

pub const DEFAULT_MEMORY_PATH: &str = "data/local/ensembly-memory.json";
pub const LEGACY_MEMORY_PATH: &str = "data/local/peram-memory.json";
pub const DEFAULT_AGENT_ID: &str = "ensembly-swarm";

pub fn env_alias(primary: &str, legacy: &str) -> Option<String> {
    std::env::var(primary)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var(legacy).ok().filter(|s| !s.is_empty()))
}

pub fn resolve_memory_path(explicit: Option<PathBuf>) -> PathBuf {
    resolve_memory_path_at(Path::new(""), explicit)
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

pub fn resolve_agent_id(explicit: Option<String>) -> String {
    if let Some(id) = explicit.filter(|s| !s.is_empty()) {
        return id;
    }
    env_alias("ENSEMBLY_AGENT_ID", "PERAM_AGENT_ID").unwrap_or_else(|| DEFAULT_AGENT_ID.into())
}
