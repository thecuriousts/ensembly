use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::manifest::{BundleManifest, FileEntry};
use crate::section::validate_safe_segment;

pub fn validate_manifest_relpath(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("manifest path must not be empty".into());
    }
    if path.contains('\0') {
        return Err("manifest path contains NUL".into());
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err("manifest path must be relative".into());
    }
    if path.contains("..") {
        return Err("manifest path must not contain ..".into());
    }
    for comp in Path::new(path).components() {
        match comp {
            Component::ParentDir => {
                return Err("manifest path must not contain ..".into());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("manifest path must not be absolute".into());
            }
            Component::CurDir => {
                return Err("manifest path must not contain empty components".into());
            }
            Component::Normal(s) => {
                if s.is_empty() {
                    return Err("manifest path must not contain empty components".into());
                }
            }
        }
    }
    Ok(())
}

pub fn validate_entry(entry: &FileEntry) -> Result<(), String> {
    validate_safe_segment(&entry.section, "section")?;
    validate_safe_segment(&entry.source, "source")?;
    validate_manifest_relpath(&entry.path)?;
    let prefix = format!("{}/{}", entry.section, entry.source);
    if entry.path != prefix && !entry.path.starts_with(&format!("{prefix}/")) {
        return Err(format!(
            "manifest path {:?} does not match section/source {}/{}",
            entry.path, entry.section, entry.source
        ));
    }
    Ok(())
}

pub fn validate_manifest_paths(manifest: &BundleManifest) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for src in &manifest.sources {
        if let Err(e) = validate_safe_segment(&src.section, "section") {
            errors.push(format!("source {:?}: {e}", src.name));
        }
        if let Err(e) = validate_safe_segment(&src.name, "source name") {
            errors.push(format!("source ref: {e}"));
        }
    }
    for entry in &manifest.files {
        if let Err(e) = validate_entry(entry) {
            errors.push(format!("file {:?}: {e}", entry.path));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Resolve a manifest-relative path for read; must stay inside `bundle_root`.
pub fn bundle_file_path(bundle_root: &Path, rel: &str) -> Result<PathBuf, String> {
    validate_manifest_relpath(rel)?;
    let root = bundle_root
        .canonicalize()
        .map_err(|e| format!("bundle root: {e}"))?;
    let mut acc = root.clone();
    for comp in Path::new(rel).components() {
        if let Component::Normal(p) = comp {
            acc = acc.join(p);
        }
    }
    if !acc.starts_with(&root) {
        return Err(format!("path escapes bundle root: {rel}"));
    }
    if acc.exists() {
        let canon = acc
            .canonicalize()
            .map_err(|e| format!("resolve {rel}: {e}"))?;
        if !canon.starts_with(&root) {
            return Err(format!("path escapes bundle root via symlink: {rel}"));
        }
        return Ok(canon);
    }
    Ok(acc)
}

pub fn restore_dest_path(
    sources_to: &Path,
    section: &str,
    source: &str,
    under: &str,
) -> Result<PathBuf, String> {
    validate_safe_segment(section, "section")?;
    validate_safe_segment(source, "source")?;
    validate_manifest_relpath(under)?;
    let base = if sources_to.exists() {
        sources_to.canonicalize().map_err(|e| e.to_string())?
    } else {
        fs::create_dir_all(sources_to).map_err(|e| e.to_string())?;
        sources_to.canonicalize().map_err(|e| e.to_string())?
    };
    let mut acc = base.clone();
    for part in [section, source] {
        acc = acc.join(part);
    }
    for comp in Path::new(under).components() {
        if let Component::Normal(p) = comp {
            acc = acc.join(p);
        }
    }
    if !acc.starts_with(&base) {
        return Err("restore target escapes --sources-to".into());
    }
    Ok(acc)
}

/// Refuse when an existing path component under `base` is a symlink (would write through it).
pub fn refuse_symlink_in_target(base: &Path, dest: &Path) -> Result<(), String> {
    let base = base.canonicalize().map_err(|e| e.to_string())?;
    let rel = dest
        .strip_prefix(&base)
        .map_err(|_| "restore path not under --sources-to".to_string())?;
    let mut current = base.clone();
    for comp in rel.components() {
        if let Component::Normal(name) = comp {
            current = current.join(name);
            if let Ok(meta) = fs::symlink_metadata(&current) {
                if meta.file_type().is_symlink() {
                    return Err(format!(
                        "refusing restore through symlink {}",
                        current.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_dotdot_and_absolute() {
        assert!(validate_manifest_relpath("../etc/passwd").is_err());
        assert!(validate_manifest_relpath("/abs/path").is_err());
        assert!(validate_manifest_relpath("ok/rel/path").is_ok());
    }
}
