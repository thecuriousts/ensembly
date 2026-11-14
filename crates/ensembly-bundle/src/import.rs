use std::path::{Path, PathBuf};

use crate::file_merge::restore_file_with_merge;
use crate::manifest::BundleManifest;
use crate::path_safety::{bundle_file_path, refuse_symlink_in_target, restore_dest_path};
use crate::verify::verify_bundle;

#[derive(Debug, Clone, Default)]
pub struct ImportOpts {
    pub merge: bool,
    pub dry_run: bool,
    /// When true, skip `verify_bundle` (caller already verified).
    pub assume_verified: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RestoreOpts {
    pub into_ops: Option<PathBuf>,
    pub into_memory: Option<PathBuf>,
    pub sources_to: Option<PathBuf>,
    pub merge: bool,
    pub dry_run: bool,
    /// When true, skip `verify_bundle` in planning (caller already verified).
    pub assume_verified: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RestorePlan {
    pub copies: Vec<(PathBuf, PathBuf)>,
    pub sidecars: Vec<PathBuf>,
}

pub fn import_bundle(bundle_root: &Path, opts: &ImportOpts) -> Result<(), String> {
    if !opts.assume_verified {
        let report = verify_bundle(bundle_root);
        if !report.ok {
            return Err(report.errors.join("; "));
        }
    }
    if opts.dry_run {
        return Ok(());
    }
    let _manifest = BundleManifest::read(bundle_root)?;
    Ok(())
}

pub fn plan_restore(bundle_root: &Path, opts: &RestoreOpts) -> Result<RestorePlan, String> {
    if !opts.assume_verified {
        let report = verify_bundle(bundle_root);
        if !report.ok {
            return Err(report.errors.join("; "));
        }
    }
    let manifest = BundleManifest::read(bundle_root)?;
    let base = opts
        .sources_to
        .as_ref()
        .ok_or_else(|| "plan_restore requires sources_to".to_string())?;
    let mut plan = RestorePlan::default();
    for entry in &manifest.files {
        if entry.excluded == Some(true) || entry.view == Some(true) {
            continue;
        }
        if entry.section == "ensembly" {
            continue;
        }
        let src = bundle_file_path(bundle_root, &entry.path)?;
        let prefix = format!("{}/{}", entry.section, entry.source);
        let under_source = entry
            .path
            .strip_prefix(&prefix)
            .map(|s| s.trim_start_matches('/'))
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                format!(
                    "manifest path {:?} does not extend section/source prefix {prefix}/",
                    entry.path
                )
            })?;
        let dest = restore_dest_path(base, &entry.section, &entry.source, under_source)?;
        refuse_symlink_in_target(base, &dest)?;
        plan.copies.push((src, dest));
    }
    Ok(plan)
}

pub fn restore_bundle(bundle_root: &Path, opts: &RestoreOpts) -> Result<RestorePlan, String> {
    let Some(base) = opts.sources_to.as_ref() else {
        return Ok(RestorePlan::default());
    };
    let plan = plan_restore(bundle_root, opts)?;
    if opts.dry_run {
        return Ok(plan);
    }
    let mut sidecars = Vec::new();
    for (src, dest) in &plan.copies {
        refuse_symlink_in_target(base, dest)?;
        let sc = restore_file_with_merge(src, dest, opts.merge, base)?;
        sidecars.extend(sc);
    }
    Ok(RestorePlan {
        copies: plan.copies,
        sidecars,
    })
}
