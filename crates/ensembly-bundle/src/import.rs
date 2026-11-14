use std::fs;
use std::path::{Path, PathBuf};

use crate::manifest::BundleManifest;
use crate::verify::verify_bundle;

#[derive(Debug, Clone, Default)]
pub struct ImportOpts {
    pub merge: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct RestoreOpts {
    pub into_ops: Option<PathBuf>,
    pub into_memory: Option<PathBuf>,
    pub sources_to: Option<PathBuf>,
    pub merge: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RestorePlan {
    pub copies: Vec<(PathBuf, PathBuf)>,
}

pub fn import_bundle(bundle_root: &Path, opts: &ImportOpts) -> Result<(), String> {
    let report = verify_bundle(bundle_root);
    if !report.ok {
        return Err(report.errors.join("; "));
    }
    if opts.dry_run {
        return Ok(());
    }
    let _manifest = BundleManifest::read(bundle_root)?;
    if !opts.merge {
        // import is verify-only unless merge destinations are added later
    }
    Ok(())
}

pub fn plan_restore(bundle_root: &Path, opts: &RestoreOpts) -> Result<RestorePlan, String> {
    let report = verify_bundle(bundle_root);
    if !report.ok {
        return Err(report.errors.join("; "));
    }
    let manifest = BundleManifest::read(bundle_root)?;
    let mut plan = RestorePlan::default();
    for entry in &manifest.files {
        if entry.excluded == Some(true) || entry.view == Some(true) {
            continue;
        }
        if entry.section == "ensembly" {
            continue;
        }
        let src = bundle_root.join(&entry.path);
        let under_source = entry
            .path
            .strip_prefix(&format!("{}/{}", entry.section, entry.source))
            .map(|s| s.trim_start_matches('/'))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                entry
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap_or("file")
            });
        if let Some(base) = &opts.sources_to {
            let dest = base
                .join(entry.section.as_str())
                .join(&entry.source)
                .join(under_source);
            plan.copies.push((src, dest));
        }
    }
    Ok(plan)
}

pub fn restore_bundle(bundle_root: &Path, opts: &RestoreOpts) -> Result<RestorePlan, String> {
    let plan = plan_restore(bundle_root, opts)?;
    if opts.dry_run {
        return Ok(plan);
    }
    for (src, dest) in &plan.copies {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if dest.exists() && !opts.merge {
            return Err(format!(
                "refusing overwrite {}; pass --merge",
                dest.display()
            ));
        }
        fs::copy(src, dest).map_err(|e| e.to_string())?;
    }
    Ok(plan)
}