//! Operator config on disk (`ensembly init`). Local only; no network reads.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct UserConfig {
    paths: Option<PathsSection>,
}

#[derive(Debug, Deserialize)]
struct PathsSection {
    data_dir: Option<String>,
}

/// `${XDG_CONFIG_HOME:-$HOME/.config}/ensembly/config.toml`
pub fn config_file_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok().filter(|s| !s.is_empty())?;
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty() && Path::new(s).is_absolute())
        .unwrap_or_else(|| format!("{home}/.config"));
    Some(PathBuf::from(base).join("ensembly/config.toml"))
}

/// `paths.data_dir` from the user config when the file exists and parses.
pub fn read_paths_data_dir() -> Option<PathBuf> {
    let path = config_file_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    let cfg: UserConfig = toml::from_str(&text).ok()?;
    let raw = cfg.paths?.data_dir?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}
