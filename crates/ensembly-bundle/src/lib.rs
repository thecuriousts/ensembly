//! Portable plain-text bundles: export, verify, import, restore.

use std::path::{Path, PathBuf};

mod config;
mod export;
mod file_merge;
mod hash;
mod import;
mod manifest;
mod merge;
mod path_safety;
mod safe_io;
mod scan;
mod section;
mod split_join;
mod verify;
mod verify_ics;
mod views;

pub use config::{load_backup_sources, BackupSource, BackupSourceKind, ConfigSourcesError};
pub use export::{export_bundle, EnsemblyStorePaths, ExportOpts, ExportReport};
pub use file_merge::{merge_json_objects, merge_jsonl_records, restore_file_with_merge};
pub use hash::{hash_bytes, hash_file, HashResult};
pub use import::{import_bundle, restore_bundle, ImportOpts, RestoreOpts, RestorePlan};
pub use manifest::{BundleManifest, FileEntry, BUNDLE_FORMAT};
pub use merge::{
    merge_jsonl_text, merge_ops_bundles, merge_ops_preserve_existing, KvRow, OpsBundleMerge,
};
pub use path_safety::{
    bundle_file_path, refuse_symlink_in_target, restore_dest_path, validate_manifest_paths,
};
pub use safe_io::copy_nofollow;
pub use section::{
    normalize_section, validate_safe_segment, validate_source_name, RESERVED_SECTION_ENSEMBLY,
    SECTION_RE,
};
pub use split_join::{
    build_bundle_tar, build_evil_gnu_longname_tar, build_evil_tar, build_symlink_tar, join_bundle,
    pack_bundle, unpack_tar_to_bundle, JoinOpts, JoinReport, PackOpts, PackReport, PartEntry,
    PartsManifest, PARTS_FORMAT,
};
pub use verify::{verify_bundle, VerifyReport};
pub use verify_ics::{
    verify_ics_content, verify_ics_file, VERIFY_ICS_MAX_BYTES, VERIFY_ICS_MAX_LOGICAL_LINE_BYTES,
};
pub use views::{derive_views_from_ops_bundle, DerivedViews};

pub const DEFAULT_MAX_FILE_MB: u64 = 200;

pub const MANIFEST_OPS_PATH: &str = "ensembly/ops/ops-bundle.json";
pub const MANIFEST_MEMORY_PATH: &str = "ensembly/memory/ensembly-memory.json";
pub const MANIFEST_LEDGER_PATH: &str = "ensembly/ledger/ledger.jsonl";

pub fn load_verified_manifest(bundle_root: &Path) -> Result<BundleManifest, String> {
    let report = verify_bundle(bundle_root);
    if !report.ok {
        return Err(report.errors.join("; "));
    }
    BundleManifest::read(bundle_root)
}

pub fn manifest_file_path(
    bundle_root: &Path,
    manifest: &BundleManifest,
    rel: &str,
) -> Result<PathBuf, String> {
    if !manifest
        .files
        .iter()
        .any(|f| f.path == rel && f.excluded != Some(true))
    {
        return Err(format!("bundle manifest has no active file {rel:?}"));
    }
    bundle_file_path(bundle_root, rel)
}
