use std::collections::HashSet;
use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::hash::hash_file;
use crate::manifest::BundleManifest;
use crate::path_safety::{bundle_file_path, validate_manifest_paths};
use crate::verify_ics::verify_ics_file;

#[derive(Debug, Clone, Default)]
pub struct VerifyReport {
    pub ok: bool,
    pub errors: Vec<String>,
}

pub fn verify_bundle(bundle_root: &Path) -> VerifyReport {
    let mut report = VerifyReport {
        ok: true,
        errors: vec![],
    };
    let manifest = match BundleManifest::read(bundle_root) {
        Ok(m) => m,
        Err(e) => {
            report.ok = false;
            report.errors.push(e);
            return report;
        }
    };
    if let Err(paths) = validate_manifest_paths(&manifest) {
        report.ok = false;
        report.errors.extend(paths);
        return report;
    }
    for entry in &manifest.files {
        if entry.excluded == Some(true) {
            continue;
        }
        let path = match bundle_file_path(bundle_root, &entry.path) {
            Ok(p) => p,
            Err(e) => {
                report.ok = false;
                report
                    .errors
                    .push(format!("unsafe path {}: {e}", entry.path));
                continue;
            }
        };
        if !path.is_file() {
            report.ok = false;
            report.errors.push(format!("missing file {}", entry.path));
            continue;
        }
        let h = match hash_file(&path) {
            Ok(x) => x,
            Err(e) => {
                report.ok = false;
                report.errors.push(format!("hash {}: {e}", entry.path));
                continue;
            }
        };
        if h.sha256 != entry.sha256 || h.bytes != entry.bytes {
            report.ok = false;
            report.errors.push(format!(
                "checksum mismatch {} expected sha256={} bytes={}",
                entry.path, entry.sha256, entry.bytes
            ));
        }
        if entry.path.ends_with(".jsonl") {
            if let Err(e) = verify_jsonl(&path) {
                report.ok = false;
                report.errors.push(format!("jsonl {}: {e}", entry.path));
            }
        }
        if entry.path.ends_with(".ics") {
            if let Err(e) = verify_ics_file(&path) {
                report.ok = false;
                report.errors.push(format!("ics {}: {e}", entry.path));
            }
        }
    }
    verify_no_extra_files(bundle_root, &manifest, &mut report);
    report
}

fn verify_no_extra_files(bundle_root: &Path, manifest: &BundleManifest, report: &mut VerifyReport) {
    let mut allowed: HashSet<String> = manifest.files.iter().map(|f| f.path.clone()).collect();
    allowed.insert("MANIFEST.json".to_string());
    let walker = WalkDir::new(bundle_root).follow_links(false);
    for entry in walker.into_iter() {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                report.ok = false;
                report.errors.push(format!("walk bundle tree: {e}"));
                return;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(bundle_root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if !allowed.contains(&rel) {
            report.ok = false;
            report
                .errors
                .push(format!("extra file not in manifest: {rel}"));
        }
    }
}

fn verify_jsonl(path: &Path) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    for (i, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        serde_json::from_str::<serde_json::Value>(line)
            .map_err(|e| format!("line {}: {e}", i + 1))?;
    }
    Ok(())
}
