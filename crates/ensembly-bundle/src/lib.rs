//! Portable plain-text bundles: export, verify, import, restore.

mod config;
mod export;
mod file_merge;
mod hash;
mod import;
mod manifest;
mod merge;
mod path_safety;
mod scan;
mod section;
mod verify;
mod views;

pub use config::{load_backup_sources, BackupSource, BackupSourceKind, ConfigSourcesError};
pub use export::{export_bundle, EnsemblyStorePaths, ExportOpts, ExportReport};
pub use file_merge::{merge_json_objects, merge_jsonl_records, restore_file_with_merge};
pub use hash::hash_bytes;
pub use import::{import_bundle, restore_bundle, ImportOpts, RestoreOpts, RestorePlan};
pub use manifest::{BundleManifest, FileEntry, BUNDLE_FORMAT};
pub use merge::{
    merge_jsonl_text, merge_ops_bundles, merge_ops_preserve_existing, KvRow, OpsBundleMerge,
};
pub use path_safety::{
    bundle_file_path, refuse_symlink_in_target, restore_dest_path, validate_manifest_paths,
};
pub use section::{
    normalize_section, validate_safe_segment, validate_source_name, RESERVED_SECTION_ENSEMBLY,
    SECTION_RE,
};
pub use verify::{verify_bundle, VerifyReport};
pub use views::{derive_views_from_ops_bundle, DerivedViews};

pub const DEFAULT_MAX_FILE_MB: u64 = 200;
