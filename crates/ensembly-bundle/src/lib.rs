//! Portable plain-text bundles: export, verify, import, restore.

mod config;
mod export;
mod hash;
mod import;
mod manifest;
mod scan;
mod section;
mod verify;

pub use config::{load_backup_sources, BackupSource, BackupSourceKind, ConfigSourcesError};
pub use export::{export_bundle, EnsemblyStorePaths, ExportOpts, ExportReport};
pub use import::{import_bundle, restore_bundle, ImportOpts, RestoreOpts, RestorePlan};
pub use manifest::{BundleManifest, FileEntry, BUNDLE_FORMAT};
pub use section::{normalize_section, RESERVED_SECTION_ENSEMBLY, SECTION_RE};
pub use verify::{verify_bundle, VerifyReport};

pub const DEFAULT_MAX_FILE_MB: u64 = 200;