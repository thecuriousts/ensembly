//! Operator config on disk (`ensembly init`). Local only; no network reads.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config: HOME is unset")]
    NoHome,
    #[error("config: cannot read {}: {message}", path.display())]
    Io { path: PathBuf, message: String },
    #[error("config: {}:{line}: invalid TOML: {detail}", path.display())]
    Parse {
        path: PathBuf,
        line: usize,
        detail: String,
    },
    #[error("config: {}: paths.data_dir: {detail}", path.display())]
    DataDir { path: PathBuf, detail: String },
}

#[derive(Debug, Deserialize)]
struct UserConfig {
    paths: Option<PathsSection>,
}

#[derive(Debug, Deserialize)]
struct PathsSection {
    data_dir: Option<String>,
}

/// `${XDG_CONFIG_HOME:-$HOME/.config}/ensembly/config.toml`
pub fn config_file_path() -> Result<PathBuf, ConfigError> {
    let home = std::env::var("HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or(ConfigError::NoHome)?;
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty() && Path::new(s).is_absolute())
        .unwrap_or_else(|| format!("{home}/.config"));
    Ok(PathBuf::from(base).join("ensembly/config.toml"))
}

fn line_at(text: &str, byte_index: usize) -> usize {
    text.get(..byte_index.min(text.len()))
        .map(|prefix| prefix.lines().count().max(1))
        .unwrap_or(1)
}

fn expand_data_dir(raw: &str) -> Result<PathBuf, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("must not be empty".into());
    }
    let home = std::env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .ok_or_else(|| "HOME is unset (needed for ~ expansion)".to_string())?;
    let expanded = if s == "~" {
        PathBuf::from(&home)
    } else if let Some(rest) = s.strip_prefix("~/") {
        PathBuf::from(home).join(rest)
    } else if s.starts_with('~') {
        return Err("only ~ or ~/… are supported".into());
    } else {
        PathBuf::from(s)
    };
    if expanded.is_relative() {
        return Err(format!(
            "must be absolute after expansion (got {})",
            expanded.display()
        ));
    }
    Ok(expanded)
}

/// Load `paths.data_dir` when the config file exists. Missing file → `Ok(None)`.
/// A present file must parse; errors name the path and line.
pub fn load_paths_data_dir() -> Result<Option<PathBuf>, ConfigError> {
    let path = config_file_path()?;
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| ConfigError::Io {
        path: path.clone(),
        message: e.to_string(),
    })?;
    let cfg: UserConfig = match toml::from_str(&text) {
        Ok(c) => c,
        Err(e) => {
            let line = e.span().map(|s| line_at(&text, s.start)).unwrap_or(1);
            return Err(ConfigError::Parse {
                path,
                line,
                detail: e.message().to_string(),
            });
        }
    };
    let Some(raw) = cfg.paths.and_then(|p| p.data_dir) else {
        return Ok(None);
    };
    expand_data_dir(&raw)
        .map(Some)
        .map_err(|detail| ConfigError::DataDir { path, detail })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_data_dir() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path();
        let cfg = home.join(".config/ensembly");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("config.toml"),
            "schema = 1\n\n[paths]\ndata_dir = \"relative/data\"\n",
        )
        .unwrap();
        std::env::set_var("HOME", home);
        std::env::set_var("XDG_CONFIG_HOME", home.join(".config"));
        let err = load_paths_data_dir().unwrap_err().to_string();
        assert!(err.contains("paths.data_dir"), "{err}");
        assert!(err.contains("absolute"), "{err}");
    }
}
