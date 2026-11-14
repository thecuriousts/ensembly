use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use walkdir::WalkDir;

use crate::config::BackupSource;
use crate::config::BackupSourceKind;
use crate::hash::{hash_bytes, hash_file};
use crate::manifest::{BundleManifest, FileEntry, SourceRef, BUNDLE_FORMAT};
use crate::scan::{default_excluded, scan_secrets, file_matches_glob};
use crate::section::RESERVED_SECTION_ENSEMBLY;
use crate::DEFAULT_MAX_FILE_MB;

#[derive(Debug, Clone)]
pub struct EnsemblyStorePaths {
    pub ops_json: Option<PathBuf>,
    pub memory_json: Option<PathBuf>,
    pub ledger_jsonl: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ExportOpts {
    pub out: PathBuf,
    pub force_new_subdir: bool,
    pub max_file_mb: u64,
    pub ensembly_version: String,
    pub created_at: DateTime<Utc>,
    pub stores: EnsemblyStorePaths,
    pub user_sources: Vec<BackupSource>,
}

#[derive(Debug, Clone)]
pub struct ExportReport {
    pub bundle_root: PathBuf,
    pub files: usize,
    pub skipped_oversize: usize,
    pub skipped_secret: usize,
}

fn max_bytes(max_file_mb: u64) -> u64 {
    max_file_mb.saturating_mul(1024 * 1024)
}

fn ensure_out_dir(out: &Path, force_new_subdir: bool) -> Result<PathBuf, String> {
    if out.exists() {
        let non_empty = fs::read_dir(out)
            .map_err(|e| e.to_string())?
            .next()
            .is_some();
        if non_empty && !force_new_subdir {
            return Err(format!(
                "refusing non-empty output {}; pass force_new_subdir or pick an empty directory",
                out.display()
            ));
        }
        if non_empty && force_new_subdir {
            let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
            let nested = out.join(format!("bundle-{stamp}"));
            fs::create_dir_all(&nested).map_err(|e| e.to_string())?;
            return Ok(nested);
        }
    }
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    Ok(out.to_path_buf())
}

fn rel_bundle_path(section: &str, source: &str, rel: &str) -> String {
    format!("{section}/{source}/{rel}")
}

fn add_plain_file(
    bundle_root: &Path,
    section: &str,
    source: &str,
    rel_name: &str,
    content: &[u8],
    files: &mut Vec<FileEntry>,
) -> Result<(), String> {
    let rel = rel_bundle_path(section, source, rel_name);
    let dest = bundle_root.join(&rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&dest, content).map_err(|e| e.to_string())?;
    let h = hash_bytes(content);
    files.push(FileEntry {
        path: rel,
        sha256: h.sha256,
        bytes: h.bytes,
        row_count: jsonl_rows_if_needed(rel_name, content),
        section: section.to_string(),
        source: source.to_string(),
        excluded: None,
        exclude_reason: None,
    });
    Ok(())
}

fn jsonl_rows_if_needed(name: &str, content: &[u8]) -> Option<u64> {
    if !name.ends_with(".jsonl") {
        return None;
    }
    let s = std::str::from_utf8(content).ok()?;
    Some(s.lines().filter(|l| !l.trim().is_empty()).count() as u64)
}

fn copy_or_hash_file(
    bundle_root: &Path,
    src: &Path,
    section: &str,
    source: &str,
    rel: &str,
    max_file_mb: u64,
    files: &mut Vec<FileEntry>,
    warnings: &mut (usize, usize),
) -> Result<(), String> {
    let meta = fs::metadata(src).map_err(|e| e.to_string())?;
    let max_b = max_bytes(max_file_mb);
    if meta.len() > max_b {
        eprintln!(
            "warn: skip oversize {} ({} bytes > {} MiB limit)",
            src.display(),
            meta.len(),
            max_file_mb
        );
        warnings.0 += 1;
        files.push(FileEntry {
            path: rel_bundle_path(section, source, rel),
            sha256: String::new(),
            bytes: meta.len(),
            row_count: None,
            section: section.to_string(),
            source: source.to_string(),
            excluded: Some(true),
            exclude_reason: Some(format!("oversize>{max_file_mb}mb")),
        });
        return Ok(());
    }
    if let Some(reason) = scan_secrets(src) {
        eprintln!("warn: skip secret-like {} ({reason})", src.display());
        warnings.1 += 1;
        files.push(FileEntry {
            path: rel_bundle_path(section, source, rel),
            sha256: String::new(),
            bytes: meta.len(),
            row_count: None,
            section: section.to_string(),
            source: source.to_string(),
            excluded: Some(true),
            exclude_reason: Some(reason),
        });
        return Ok(());
    }
    let dest = bundle_root.join(rel_bundle_path(section, source, rel));
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(src, &dest).map_err(|e| e.to_string())?;
    let h = hash_file(src).map_err(|e| e.to_string())?;
    files.push(FileEntry {
        path: rel_bundle_path(section, source, rel),
        sha256: h.sha256,
        bytes: h.bytes,
        row_count: jsonl_rows_from_path(rel, src),
        section: section.to_string(),
        source: source.to_string(),
        excluded: None,
        exclude_reason: None,
    });
    Ok(())
}

fn jsonl_rows_from_path(name: &str, path: &Path) -> Option<u64> {
    if !name.ends_with(".jsonl") {
        return None;
    }
    let s = fs::read_to_string(path).ok()?;
    Some(s.lines().filter(|l| !l.trim().is_empty()).count() as u64)
}

fn export_user_source(
    bundle_root: &Path,
    src: &BackupSource,
    max_file_mb: u64,
    files: &mut Vec<FileEntry>,
    warnings: &mut (usize, usize),
) -> Result<(), String> {
    match src.kind {
        BackupSourceKind::File => {
            if !src.path.is_file() {
                return Err(format!("source {}: missing file {}", src.name, src.path.display()));
            }
            let rel = src.path.file_name().and_then(|n| n.to_str()).unwrap_or("data");
            copy_or_hash_file(
                bundle_root,
                &src.path,
                &src.section,
                &src.name,
                rel,
                max_file_mb,
                files,
                warnings,
            )
        }
        BackupSourceKind::Dir => {
            if !src.path.is_dir() {
                return Err(format!("source {}: missing dir {}", src.name, src.path.display()));
            }
            let base = src.path.canonicalize().map_err(|e| e.to_string())?;
            for entry in WalkDir::new(&base).into_iter().filter_map(|e| e.ok()) {
                if !entry.file_type().is_file() {
                    continue;
                }
                let full = entry.path();
                let rel = full
                    .strip_prefix(&base)
                    .map_err(|_| "strip_prefix")?
                    .to_string_lossy()
                    .replace('\\', "/");
                if default_excluded(&rel) {
                    continue;
                }
                let included = src.include.iter().any(|g| file_matches_glob(&rel, g));
                if !included {
                    continue;
                }
                if src.exclude.iter().any(|g| file_matches_glob(&rel, g)) {
                    continue;
                }
                copy_or_hash_file(
                    bundle_root,
                    full,
                    &src.section,
                    &src.name,
                    &rel,
                    max_file_mb,
                    files,
                    warnings,
                )?;
            }
            Ok(())
        }
    }
}

/// Write `bundle_root/MANIFEST.json` and tree `bundle/<section>/<source>/…`.
pub fn export_bundle(opts: ExportOpts) -> Result<ExportReport, String> {
    let max_file_mb = if opts.max_file_mb == 0 {
        DEFAULT_MAX_FILE_MB
    } else {
        opts.max_file_mb
    };
    let bundle_root = ensure_out_dir(&opts.out, opts.force_new_subdir)?;
    let mut sources: Vec<SourceRef> = Vec::new();
    let mut files: Vec<FileEntry> = Vec::new();
    let mut warnings = (0usize, 0usize);

    let section = RESERVED_SECTION_ENSEMBLY;
    if let Some(ref ops) = opts.stores.ops_json {
        sources.push(SourceRef {
            name: "ops".into(),
            section: section.into(),
            kind: "json".into(),
        });
        let raw = fs::read(ops).map_err(|e| e.to_string())?;
        add_plain_file(&bundle_root, section, "ops", "ops-bundle.json", &raw, &mut files)?;
    }
    if let Some(ref mem) = opts.stores.memory_json {
        if mem.is_file() {
            sources.push(SourceRef {
                name: "memory".into(),
                section: section.into(),
                kind: "json".into(),
            });
            copy_or_hash_file(
                &bundle_root,
                mem,
                section,
                "memory",
                "ensembly-memory.json",
                max_file_mb,
                &mut files,
                &mut warnings,
            )?;
        }
    }
    if let Some(ref led) = opts.stores.ledger_jsonl {
        if led.is_file() {
            sources.push(SourceRef {
                name: "ledger".into(),
                section: section.into(),
                kind: "jsonl".into(),
            });
            copy_or_hash_file(
                &bundle_root,
                led,
                section,
                "ledger",
                "ledger.jsonl",
                max_file_mb,
                &mut files,
                &mut warnings,
            )?;
        }
    }

    for src in &opts.user_sources {
        sources.push(SourceRef {
            name: src.name.clone(),
            section: src.section.clone(),
            kind: format!("{:?}", src.kind).to_lowercase(),
        });
        export_user_source(&bundle_root, src, max_file_mb, &mut files, &mut warnings)?;
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    sources.sort_by(|a, b| {
        (a.section.as_str(), a.name.as_str()).cmp(&(b.section.as_str(), b.name.as_str()))
    });

    let manifest = BundleManifest {
        bundle_format: BUNDLE_FORMAT.to_string(),
        ensembly_version: opts.ensembly_version,
        created_at: opts.created_at,
        sources,
        files,
    };
    manifest.write(&bundle_root).map_err(|e| e.to_string())?;

  Ok(ExportReport {
        bundle_root,
        files: manifest.files.len(),
        skipped_oversize: warnings.0,
        skipped_secret: warnings.1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BackupSource, BackupSourceKind};
    use chrono::TimeZone;
    use tempfile::tempdir;

    fn fixture_export(dir: &Path, sources: Vec<BackupSource>) -> ExportReport {
        let shared = dir.join("shared-src");
        fs::create_dir_all(&shared).unwrap();
        fs::write(shared.join("note.md"), b"# hello").unwrap();
        let work = dir.join("work-src");
        fs::create_dir_all(&work).unwrap();
        fs::write(work.join("todo.csv"), b"id,title\n1,task").unwrap();
        let mut user = sources;
        if user.is_empty() {
            user = vec![
                BackupSource {
                    name: "notes".into(),
                    section: "shared".into(),
                    path: shared,
                    kind: BackupSourceKind::Dir,
                    include: vec!["**/*".into()],
                    exclude: vec![],
                    line: 1,
                },
                BackupSource {
                    name: "tasks".into(),
                    section: "work".into(),
                    path: work,
                    kind: BackupSourceKind::Dir,
                    include: vec!["**/*".into()],
                    exclude: vec![],
                    line: 2,
                },
            ];
        }
        let out = dir.join("bundle-out");
        export_bundle(ExportOpts {
            out,
            force_new_subdir: false,
            max_file_mb: DEFAULT_MAX_FILE_MB,
            ensembly_version: "0.0-test".into(),
            created_at: Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            stores: EnsemblyStorePaths {
                ops_json: None,
                memory_json: None,
                ledger_jsonl: None,
            },
            user_sources: user,
        })
        .unwrap()
    }

    #[test]
    fn multi_section_layout() {
        let dir = tempdir().unwrap();
        let rep = fixture_export(dir.path(), vec![]);
        assert!(rep.bundle_root.join("shared/notes/note.md").is_file());
        assert!(rep.bundle_root.join("work/tasks/todo.csv").is_file());
        let m = BundleManifest::read(&rep.bundle_root).unwrap();
        assert!(m.files.iter().any(|f| f.section == "shared" && f.source == "notes"));
        assert!(m.files.iter().any(|f| f.section == "work" && f.source == "tasks"));
    }

    #[test]
    fn refuses_nonempty_out_without_force() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("b");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("stray"), b"x").unwrap();
        let err = export_bundle(ExportOpts {
            out,
            force_new_subdir: false,
            max_file_mb: DEFAULT_MAX_FILE_MB,
            ensembly_version: "0".into(),
            created_at: Utc::now(),
            stores: EnsemblyStorePaths::default(),
            user_sources: vec![],
        })
        .unwrap_err();
        assert!(err.contains("non-empty"));
    }
}

impl Default for EnsemblyStorePaths {
    fn default() -> Self {
        Self {
            ops_json: None,
            memory_json: None,
            ledger_jsonl: None,
        }
    }
}