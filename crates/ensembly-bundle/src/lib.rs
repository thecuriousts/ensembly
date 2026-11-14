//! Portable plain-text bundles: export, verify, import, restore.

mod config;
mod export;
mod hash;
mod import;
mod manifest;
mod merge;
mod scan;
mod section;
mod verify;
mod views;

pub use config::{load_backup_sources, BackupSource, BackupSourceKind, ConfigSourcesError};
pub use export::{export_bundle, EnsemblyStorePaths, ExportOpts, ExportReport};
pub use import::{import_bundle, restore_bundle, ImportOpts, RestoreOpts, RestorePlan};
pub use manifest::{BundleManifest, FileEntry, BUNDLE_FORMAT};
pub use merge::{merge_jsonl_text, merge_ops_bundles, KvRow, OpsBundleMerge};
pub use section::{normalize_section, RESERVED_SECTION_ENSEMBLY, SECTION_RE};
pub use verify::{verify_bundle, VerifyReport};
pub use views::{derive_views_from_ops_bundle, DerivedViews};

pub const DEFAULT_MAX_FILE_MB: u64 = 200;
