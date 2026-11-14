use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const BUNDLE_FORMAT: &str = "ensembly-portable-bundle-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceRef {
    pub name: String,
    pub section: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<u64>,
    pub section: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_reason: Option<String>,
    /// Read-only derived file; restore never writes these (SoT is ops JSON).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleManifest {
    pub bundle_format: String,
    pub ensembly_version: String,
    pub created_at: DateTime<Utc>,
    pub sources: Vec<SourceRef>,
    pub files: Vec<FileEntry>,
}

impl BundleManifest {
    pub fn write(&self, bundle_root: &std::path::Path) -> Result<(), std::io::Error> {
        let p = bundle_root.join("MANIFEST.json");
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(p, json)
    }

    pub fn read(bundle_root: &std::path::Path) -> Result<Self, String> {
        let p = bundle_root.join("MANIFEST.json");
        let raw = std::fs::read_to_string(&p).map_err(|e| format!("read manifest: {e}"))?;
        let m: Self = serde_json::from_str(&raw).map_err(|e| format!("parse manifest: {e}"))?;
        if m.bundle_format != BUNDLE_FORMAT {
            return Err(format!("unsupported bundle_format {:?}", m.bundle_format));
        }
        Ok(m)
    }
}
