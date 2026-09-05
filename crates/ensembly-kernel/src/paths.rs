//! Default path + env discover law.
//!
//! Fresh trees create `ensembly-*` files. Existing `peram-*` files are opened
//! in place. Never copy or rename a live DB.

use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const DEFAULT_OPS_PATH: &str = "data/local/ensembly-ops.sqlite";
pub const DEFAULT_OPS_PRIVATE_PATH: &str = "private/state/ensembly-ops.sqlite";
pub const LEGACY_OPS_PATH: &str = "data/local/peram-ops.sqlite";
pub const LEGACY_OPS_PRIVATE_PATH: &str = "private/state/peram-ops.sqlite";

pub const DEFAULT_MEMORY_PATH: &str = "data/local/ensembly-memory.json";
pub const LEGACY_MEMORY_PATH: &str = "data/local/peram-memory.json";

/// Replica id for **new** empty memory documents. Existing docs keep theirs.
pub const DEFAULT_AGENT_ID: &str = "ensembly-swarm";
pub const LEGACY_AGENT_ID: &str = "peram-swarm";

/// Prefer `primary` when set and non-empty; else `legacy` (one-release alias).
pub fn env_alias(primary: &str, legacy: &str) -> Option<String> {
    std::env::var(primary)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var(legacy).ok().filter(|s| !s.is_empty()))
}

/// `--db` wins. Else existing ensembly file, else existing legacy file, else
/// create-default (parent-dir discover, new names).
pub fn resolve_ops_db(explicit: Option<PathBuf>) -> PathBuf {
    resolve_ops_db_at(Path::new(""), explicit)
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

/// `--memory` wins. Else existing ensembly file, else existing legacy file,
/// else the fresh `ensembly-memory.json` default.
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathCopyAction {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalPathMigrateReport {
    pub dry_run: bool,
    pub copied: Vec<PathCopyAction>,
    pub skipped_dest_exists: Vec<PathCopyAction>,
    pub skipped_src_missing: Vec<PathCopyAction>,
}

/// One-shot **copy-if-missing**. Never overwrites dest. Never deletes source.
/// Also copies SQLite `-wal` / `-shm` siblings when present.
pub fn migrate_local_paths_at(
    root: &Path,
    dry_run: bool,
) -> io::Result<LocalPathMigrateReport> {
    let pairs = [
        (LEGACY_OPS_PATH, DEFAULT_OPS_PATH),
        (LEGACY_OPS_PRIVATE_PATH, DEFAULT_OPS_PRIVATE_PATH),
        (LEGACY_MEMORY_PATH, DEFAULT_MEMORY_PATH),
    ];
    let mut report = LocalPathMigrateReport {
        dry_run,
        copied: vec![],
        skipped_dest_exists: vec![],
        skipped_src_missing: vec![],
    };
    for (from_rel, to_rel) in pairs {
        let from = root.join(from_rel);
        let to = root.join(to_rel);
        let action = PathCopyAction {
            from: from.clone(),
            to: to.clone(),
        };
        if !from.is_file() {
            report.skipped_src_missing.push(action);
            continue;
        }
        if to.is_file() {
            report.skipped_dest_exists.push(action);
            continue;
        }
        if !dry_run {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to)?;
            copy_sqlite_sidecars(&from, &to)?;
        }
        report.copied.push(action);
    }
    Ok(report)
}

fn copy_sqlite_sidecars(from: &Path, to: &Path) -> io::Result<()> {
    let from_s = from.to_string_lossy();
    if !from_s.ends_with(".sqlite") {
        return Ok(());
    }
    for suffix in ["-wal", "-shm"] {
        let src = PathBuf::from(format!("{from_s}{suffix}"));
        if src.is_file() {
            let dest = PathBuf::from(format!("{}{suffix}", to.to_string_lossy()));
            fs::copy(&src, &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn fresh_default_creates_ensembly_ops_and_memory() {
        let dir = tempfile::tempdir().unwrap();
        let ops = resolve_ops_db_at(dir.path(), None);
        let mem = resolve_memory_path_at(dir.path(), None);
        assert_eq!(ops, dir.path().join(DEFAULT_OPS_PATH));
        assert_eq!(mem, dir.path().join(DEFAULT_MEMORY_PATH));
        assert!(!ops.exists(), "must not create the file during resolve");
        assert!(!mem.exists());
    }

    #[test]
    fn parent_dir_present_still_picks_ensembly_create_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("data/local")).unwrap();
        let ops = resolve_ops_db_at(dir.path(), None);
        assert_eq!(ops, dir.path().join(DEFAULT_OPS_PATH));
    }

    #[test]
    fn legacy_ops_chosen_when_only_legacy_exists() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("data/local")).unwrap();
        let legacy = dir.path().join(LEGACY_OPS_PATH);
        fs::write(&legacy, b"legacy").unwrap();
        let got = resolve_ops_db_at(dir.path(), None);
        assert_eq!(got, legacy);
        assert!(!dir.path().join(DEFAULT_OPS_PATH).exists());
    }

    #[test]
    fn legacy_private_ops_chosen_when_only_that_exists() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("private/state")).unwrap();
        let legacy = dir.path().join(LEGACY_OPS_PRIVATE_PATH);
        fs::write(&legacy, b"legacy-private").unwrap();
        let got = resolve_ops_db_at(dir.path(), None);
        assert_eq!(got, legacy);
    }

    #[test]
    fn ensembly_ops_wins_over_legacy_when_both_exist() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("data/local")).unwrap();
        let neu = dir.path().join(DEFAULT_OPS_PATH);
        let old = dir.path().join(LEGACY_OPS_PATH);
        fs::write(&neu, b"new").unwrap();
        fs::write(&old, b"old").unwrap();
        assert_eq!(resolve_ops_db_at(dir.path(), None), neu);
    }

    #[test]
    fn legacy_memory_chosen_when_only_legacy_exists() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("data/local")).unwrap();
        let legacy = dir.path().join(LEGACY_MEMORY_PATH);
        fs::write(&legacy, b"{}").unwrap();
        let got = resolve_memory_path_at(dir.path(), None);
        assert_eq!(got, legacy);
        assert!(!dir.path().join(DEFAULT_MEMORY_PATH).exists());
    }

    #[test]
    fn explicit_db_and_memory_always_win() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("data/local")).unwrap();
        fs::write(dir.path().join(LEGACY_OPS_PATH), b"legacy").unwrap();
        let forced = dir.path().join("forced.sqlite");
        assert_eq!(
            resolve_ops_db_at(dir.path(), Some(forced.clone())),
            forced
        );
        let forced_mem = PathBuf::from("/tmp/forced-mem.json");
        assert_eq!(
            resolve_memory_path_at(dir.path(), Some(forced_mem.clone())),
            forced_mem
        );
    }

    #[test]
    fn resolve_does_not_rename_or_copy_legacy() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("data/local")).unwrap();
        let legacy = dir.path().join(LEGACY_OPS_PATH);
        fs::write(&legacy, b"keep-me").unwrap();
        let _ = resolve_ops_db_at(dir.path(), None);
        assert_eq!(fs::read(&legacy).unwrap(), b"keep-me");
        assert!(!dir.path().join(DEFAULT_OPS_PATH).exists());
    }

    #[test]
    fn migrate_copies_if_missing_keeps_legacy() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("data/local")).unwrap();
        let legacy_ops = dir.path().join(LEGACY_OPS_PATH);
        let legacy_mem = dir.path().join(LEGACY_MEMORY_PATH);
        fs::write(&legacy_ops, b"ops-bytes").unwrap();
        fs::write(format!("{}-wal", legacy_ops.display()), b"wal").unwrap();
        fs::write(&legacy_mem, b"mem-bytes").unwrap();

        let report = migrate_local_paths_at(dir.path(), false).unwrap();
        assert_eq!(report.copied.len(), 2);
        let dest_ops = dir.path().join(DEFAULT_OPS_PATH);
        let dest_mem = dir.path().join(DEFAULT_MEMORY_PATH);
        assert_eq!(fs::read(&dest_ops).unwrap(), b"ops-bytes");
        assert_eq!(fs::read(format!("{}-wal", dest_ops.display())).unwrap(), b"wal");
        assert_eq!(fs::read(&dest_mem).unwrap(), b"mem-bytes");
        assert_eq!(fs::read(&legacy_ops).unwrap(), b"ops-bytes");
        assert_eq!(fs::read(&legacy_mem).unwrap(), b"mem-bytes");
        assert_eq!(resolve_ops_db_at(dir.path(), None), dest_ops);
        assert_eq!(resolve_memory_path_at(dir.path(), None), dest_mem);
    }

    #[test]
    fn migrate_does_not_overwrite_existing_dest() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("data/local")).unwrap();
        fs::write(dir.path().join(LEGACY_OPS_PATH), b"old").unwrap();
        fs::write(dir.path().join(DEFAULT_OPS_PATH), b"already-new").unwrap();
        let report = migrate_local_paths_at(dir.path(), false).unwrap();
        assert!(report.copied.is_empty());
        assert_eq!(report.skipped_dest_exists.len(), 1);
        assert_eq!(
            fs::read(dir.path().join(DEFAULT_OPS_PATH)).unwrap(),
            b"already-new"
        );
    }

    #[test]
    fn migrate_dry_run_does_not_write() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("data/local")).unwrap();
        fs::write(dir.path().join(LEGACY_MEMORY_PATH), b"mem").unwrap();
        let report = migrate_local_paths_at(dir.path(), true).unwrap();
        assert_eq!(report.copied.len(), 1);
        assert!(report.dry_run);
        assert!(!dir.path().join(DEFAULT_MEMORY_PATH).exists());
    }
}
