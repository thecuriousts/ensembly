use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::section::{normalize_section, validate_source_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupSourceKind {
    Dir,
    File,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackupSourceRaw {
    pub name: String,
    pub path: String,
    pub kind: BackupSourceKind,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BackupSource {
    pub name: String,
    pub section: String,
    pub path: PathBuf,
    pub kind: BackupSourceKind,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    /// 1-based line in config.toml for error messages.
    pub line: usize,
}

#[derive(Debug, Error)]
pub enum ConfigSourcesError {
    #[error("config: cannot read {}: {message}", path.display())]
    Io { path: PathBuf, message: String },
    #[error("config: {}:{line}: invalid TOML: {detail}", path.display())]
    Parse {
        path: PathBuf,
        line: usize,
        detail: String,
    },
    #[error("config: {}:{line}: backup.sources: {detail}", path.display())]
    Source {
        path: PathBuf,
        line: usize,
        detail: String,
    },
}

#[derive(Debug, Deserialize)]
struct UserConfig {
    backup: Option<BackupSection>,
}

#[derive(Debug, Deserialize)]
struct BackupSection {
    sources: Option<Vec<BackupSourceRaw>>,
}

fn line_at(text: &str, byte_index: usize) -> usize {
    text.get(..byte_index.min(text.len()))
        .map(|prefix| prefix.lines().count().max(1))
        .unwrap_or(1)
}

fn expand_path(raw: &str) -> Result<PathBuf, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("path must not be empty".into());
    }
    let home = std::env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .ok_or_else(|| "HOME unset (needed for ~ expansion)".to_string())?;
    if s == "~" {
        return Ok(PathBuf::from(home));
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return Ok(PathBuf::from(home).join(rest));
    }
    if s.starts_with('~') {
        return Err("only ~ or ~/… supported".into());
    }
    Ok(PathBuf::from(s))
}

fn parse_name_line(line: &str) -> Option<String> {
    let t = line.trim();
    if !t.starts_with("name") {
        return None;
    }
    let (_, rhs) = t.split_once('=')?;
    let rhs = rhs.trim().trim_matches('"').trim_matches('\'');
    if rhs.is_empty() {
        None
    } else {
        Some(rhs.to_string())
    }
}

/// One `(line, name)` per `[[backup.sources]]` table in file order.
pub fn backup_source_name_lines(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut pending: Option<(usize, String)> = None;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t == "[[backup.sources]]" {
            if let Some(pair) = pending.take() {
                out.push(pair);
            }
            continue;
        }
        if t.starts_with("[[") {
            if let Some(pair) = pending.take() {
                out.push(pair);
            }
            continue;
        }
        if let Some(name) = parse_name_line(t) {
            pending = Some((i + 1, name));
        }
    }
    if let Some(pair) = pending {
        out.push(pair);
    }
    out
}

/// Load `[[backup.sources]]` from operator config. Missing file → empty list.
pub fn load_backup_sources(config_path: &Path) -> Result<Vec<BackupSource>, ConfigSourcesError> {
    if !config_path.is_file() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(config_path).map_err(|e| ConfigSourcesError::Io {
        path: config_path.to_path_buf(),
        message: e.to_string(),
    })?;
    let cfg: UserConfig = match toml::from_str(&text) {
        Ok(c) => c,
        Err(e) => {
            let line = e.span().map(|s| line_at(&text, s.start)).unwrap_or(1);
            return Err(ConfigSourcesError::Parse {
                path: config_path.to_path_buf(),
                line,
                detail: e.message().to_string(),
            });
        }
    };
    let Some(raw) = cfg.backup.and_then(|b| b.sources) else {
        return Ok(vec![]);
    };
    let name_lines = backup_source_name_lines(&text);
    let mut out = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for (idx, entry) in raw.into_iter().enumerate() {
        let line = name_lines.get(idx).map(|(ln, _)| *ln).unwrap_or(idx + 1);
        let section = normalize_section(entry.section.as_deref()).map_err(|detail| {
            ConfigSourcesError::Source {
                path: config_path.to_path_buf(),
                line,
                detail,
            }
        })?;
        validate_source_name(&entry.name).map_err(|detail| ConfigSourcesError::Source {
            path: config_path.to_path_buf(),
            line,
            detail,
        })?;
        let key = (section.clone(), entry.name.clone());
        if !seen.insert(key) {
            return Err(ConfigSourcesError::Source {
                path: config_path.to_path_buf(),
                line,
                detail: format!(
                    "duplicate source name {:?} in section {:?}",
                    entry.name, section
                ),
            });
        }
        let path = expand_path(&entry.path).map_err(|detail| ConfigSourcesError::Source {
            path: config_path.to_path_buf(),
            line,
            detail,
        })?;
        let include = if entry.include.is_empty() {
            vec!["**/*".to_string()]
        } else {
            entry.include
        };
        out.push(BackupSource {
            name: entry.name,
            section,
            path,
            kind: entry.kind,
            include,
            exclude: entry.exclude,
            line,
        });
    }
    Ok(out)
}
