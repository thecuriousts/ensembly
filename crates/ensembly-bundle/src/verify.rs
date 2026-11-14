use std::fs;
use std::path::Path;

use crate::hash::hash_file;
use crate::manifest::BundleManifest;

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
    for entry in &manifest.files {
        if entry.excluded == Some(true) {
            continue;
        }
        let path = bundle_root.join(&entry.path);
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
            if let Err(e) = verify_ics(&path) {
                report.ok = false;
                report.errors.push(format!("ics {}: {e}", entry.path));
            }
        }
    }
    report
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

fn verify_ics(path: &Path) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    if !raw.contains("BEGIN:VCALENDAR") || !raw.contains("END:VCALENDAR") {
        return Err("missing VCALENDAR envelope".into());
    }
    Ok(())
}