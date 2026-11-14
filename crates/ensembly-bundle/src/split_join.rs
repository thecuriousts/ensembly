//! Split and join portable bundles for size-limited transfers (plain tar + parts manifest).

use crate::hash::{hash_file, HashResult};
use crate::path_safety::{refuse_symlink_in_target, validate_manifest_relpath};
use crate::safe_io::copy_reader_nofollow;
use crate::section::validate_safe_segment;
use crate::verify::verify_bundle;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use tar::{Builder, EntryType, Header};
use walkdir::WalkDir;

pub const PARTS_FORMAT: &str = "ensembly-portable-bundle-parts-v1";
const TAR_MTIME: u64 = 0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartEntry {
    pub index: u32,
    pub file: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartsManifest {
    pub parts_format: String,
    pub bundle_name: String,
    pub split_mb: u64,
    pub part_count: u32,
    pub total_sha256: String,
    pub parts: Vec<PartEntry>,
}

impl PartsManifest {
    pub fn read(path: &Path) -> Result<Self, String> {
        let raw = fs::read(path).map_err(|e| e.to_string())?;
        let m: Self = serde_json::from_slice(&raw).map_err(|e| format!("parts json: {e}"))?;
        if m.parts_format != PARTS_FORMAT {
            return Err(format!(
                "unsupported parts_format {:?}; want {PARTS_FORMAT}",
                m.parts_format
            ));
        }
        Ok(m)
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let body = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, body).map_err(|e| e.to_string())
    }

    pub fn path(&self, parts_json: &Path) -> PathBuf {
        parts_json
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

pub struct PackOpts {
    pub bundle_dir: PathBuf,
    pub out: PathBuf,
    pub split_mb: u64,
}

#[derive(Debug)]
pub struct PackReport {
    pub parts_json: PathBuf,
    pub part_count: u32,
    pub total_bytes: u64,
}

pub struct JoinOpts {
    pub parts_json: PathBuf,
    pub out: PathBuf,
}

#[derive(Debug)]
pub struct JoinReport {
    pub bundle_root: PathBuf,
}

fn refuse_non_empty_dir(dir: &Path, label: &str) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    if dir.is_file() {
        return Err(format!(
            "--out must be a directory, not an existing file: {}",
            dir.display()
        ));
    }
    let mut entries = fs::read_dir(dir).map_err(|e| e.to_string())?;
    if entries.next().is_some() {
        return Err(format!(
            "refusing non-empty {label} {}; pick an empty directory",
            dir.display()
        ));
    }
    Ok(())
}

fn bundle_base_name(bundle_dir: &Path) -> Result<String, String> {
    let name = bundle_dir
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "bundle directory has no name".to_string())?;
    validate_safe_segment(name, "bundle name")?;
    Ok(name.to_string())
}

enum TarItem {
    Dir(String),
    File(String, PathBuf),
}

impl TarItem {
    fn rel(&self) -> &str {
        match self {
            TarItem::Dir(r) | TarItem::File(r, _) => r,
        }
    }
}

fn collect_tar_items(bundle_root: &Path) -> Result<Vec<TarItem>, String> {
    if !bundle_root.is_dir() {
        return Err(format!(
            "bundle dir is not a directory: {}",
            bundle_root.display()
        ));
    }
    let manifest = bundle_root.join("MANIFEST.json");
    if !manifest.is_file() {
        return Err("bundle dir missing MANIFEST.json".into());
    }
    let mut items = Vec::new();
    let walker = WalkDir::new(bundle_root)
        .follow_links(false)
        .sort_by(|a, b| a.path().cmp(b.path()));
    for entry in walker {
        let entry = entry.map_err(|e| format!("walk bundle: {e}"))?;
        if entry.file_type().is_symlink() {
            return Err(format!(
                "refusing symlink in bundle: {}",
                entry.path().display()
            ));
        }
        let rel = entry
            .path()
            .strip_prefix(bundle_root)
            .map_err(|_| "strip bundle root".to_string())?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let rel_unix = rel.to_string_lossy().replace('\\', "/");
        validate_manifest_relpath(&rel_unix)?;
        if entry.file_type().is_dir() {
            items.push(TarItem::Dir(rel_unix));
        } else if entry.file_type().is_file() {
            items.push(TarItem::File(rel_unix, entry.path().to_path_buf()));
        } else {
            return Err(format!("unsupported bundle entry type: {rel_unix}"));
        }
    }
    items.sort_by(|a, b| a.rel().cmp(b.rel()));
    Ok(items)
}

/// Fixed metadata for pack tar entries. GNU (not ustar) so `append_data` can emit
/// `././@LongLink` extension records for paths longer than the 100-byte ustar name field.
fn prepare_deterministic_header(size: u64, is_dir: bool) -> Header {
    let mut header = Header::new_gnu();
    if is_dir {
        header.set_entry_type(EntryType::Directory);
        header.set_size(0);
        header.set_mode(0o755);
    } else {
        header.set_entry_type(EntryType::Regular);
        header.set_size(size);
        header.set_mode(0o644);
    }
    header.set_mtime(TAR_MTIME);
    header.set_uid(0);
    header.set_gid(0);
    header
}

fn refuse_stray_tar_metadata(header: &Header) -> Result<(), String> {
    let entry_type = header.entry_type();
    if entry_type.is_gnu_longname()
        || entry_type.is_gnu_longlink()
        || entry_type.is_pax_local_extensions()
        || entry_type.is_pax_global_extensions()
    {
        return Err("refusing stray tar metadata entry".into());
    }
    Ok(())
}

fn validate_resolved_tar_path(path: &Path) -> Result<String, String> {
    let rel = path
        .to_str()
        .ok_or_else(|| "tar entry path is not UTF-8".to_string())?
        .replace('\\', "/");
    validate_manifest_relpath(&rel)?;
    Ok(rel)
}

pub fn build_bundle_tar(bundle_root: &Path, tar_out: &Path) -> Result<HashResult, String> {
    let items = collect_tar_items(bundle_root)?;
    if let Some(parent) = tar_out.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tar_file = File::create(tar_out).map_err(|e| e.to_string())?;
    let mut builder = Builder::new(tar_file);
    for item in items {
        match item {
            TarItem::Dir(rel) => {
                let mut header = prepare_deterministic_header(0, true);
                builder
                    .append_data(&mut header, rel.as_str(), &mut std::io::empty())
                    .map_err(|e| e.to_string())?;
            }
            TarItem::File(rel, path) => {
                let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
                let mut header = prepare_deterministic_header(meta.len(), false);
                let mut file = File::open(&path).map_err(|e| e.to_string())?;
                builder
                    .append_data(&mut header, rel.as_str(), &mut file)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    builder.into_inner().map_err(|e| e.to_string())?;
    hash_file(tar_out).map_err(|e| e.to_string())
}

fn split_tar(
    tar_path: &Path,
    out_dir: &Path,
    base_name: &str,
    split_mb: u64,
) -> Result<Vec<PartEntry>, String> {
    let max = split_mb
        .checked_mul(1024 * 1024)
        .ok_or_else(|| "split_mb overflow".to_string())?;
    if max == 0 {
        return Err("split_mb must be positive".into());
    }
    let mut reader = BufReader::new(File::open(tar_path).map_err(|e| e.to_string())?);
    let mut buf = vec![0u8; 64 * 1024];
    let mut index = 1u32;
    let mut parts = Vec::new();
    loop {
        let part_name = format!("{base_name}.tar.part{index:03}");
        let part_path = out_dir.join(&part_name);
        let mut part_file = File::create(&part_path).map_err(|e| e.to_string())?;
        let mut part_len = 0u64;
        while part_len < max {
            let room = (max - part_len) as usize;
            let to_read = room.min(buf.len());
            let n = reader
                .read(&mut buf[..to_read])
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            part_file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            part_len += n as u64;
        }
        if part_len == 0 {
            if index == 1 {
                return Err("empty tar archive".into());
            }
            let _ = fs::remove_file(&part_path);
            break;
        }
        if part_len > max {
            return Err(format!(
                "internal error: part {index} exceeds split size ({part_len} > {max})"
            ));
        }
        let hashed = hash_file(&part_path).map_err(|e| e.to_string())?;
        parts.push(PartEntry {
            index,
            file: part_name,
            sha256: hashed.sha256,
            bytes: hashed.bytes,
        });
        index += 1;
        if part_len < max {
            break;
        }
    }
    Ok(parts)
}

pub fn pack_bundle(opts: PackOpts) -> Result<PackReport, String> {
    if opts.split_mb == 0 {
        return Err("split_mb must be positive".into());
    }
    refuse_non_empty_dir(&opts.out, "output")?;
    fs::create_dir_all(&opts.out).map_err(|e| e.to_string())?;
    let base_name = bundle_base_name(&opts.bundle_dir)?;
    let staging = opts.out.join(format!("{base_name}.staging.tar"));
    let total = build_bundle_tar(&opts.bundle_dir, &staging)?;
    let parts = split_tar(&staging, &opts.out, &base_name, opts.split_mb)?;
    fs::remove_file(&staging).map_err(|e| e.to_string())?;
    if parts.iter().any(|p| p.bytes > opts.split_mb * 1024 * 1024) {
        return Err("a part exceeds split_mb limit".into());
    }
    let manifest = PartsManifest {
        parts_format: PARTS_FORMAT.to_string(),
        bundle_name: base_name.clone(),
        split_mb: opts.split_mb,
        part_count: parts.len() as u32,
        total_sha256: total.sha256,
        parts,
    };
    if manifest.part_count as usize != manifest.parts.len() {
        return Err("internal part_count mismatch".into());
    }
    let parts_json = opts.out.join(format!("{base_name}.parts.json"));
    manifest.write(&parts_json)?;
    Ok(PackReport {
        parts_json,
        part_count: manifest.part_count,
        total_bytes: total.bytes,
    })
}

fn part_file_pattern(base_name: &str) -> String {
    format!("{base_name}.tar.part")
}

fn refuse_extra_parts(
    dir: &Path,
    base_name: &str,
    expected: &HashSet<String>,
) -> Result<(), String> {
    let prefix = part_file_pattern(base_name);
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix)
            && name.len() == prefix.len() + 3
            && name[prefix.len()..].chars().all(|c| c.is_ascii_digit())
            && !expected.contains(&name)
        {
            return Err(format!("extra part file {name}"));
        }
    }
    Ok(())
}

pub fn concat_parts(manifest: &PartsManifest, parts_dir: &Path) -> Result<PathBuf, String> {
    let mut parts = manifest.parts.clone();
    parts.sort_by_key(|p| p.index);
    if parts.len() != manifest.part_count as usize {
        return Err(format!(
            "part_count {} does not match parts entries {}",
            manifest.part_count,
            parts.len()
        ));
    }
    let expected_names: HashSet<String> = parts.iter().map(|p| p.file.clone()).collect();
    refuse_extra_parts(parts_dir, &manifest.bundle_name, &expected_names)?;
    let mut seen_index = 0u32;
    let staging = parts_dir.join(format!("{}.join.staging.tar", manifest.bundle_name));
    let mut out_file = File::create(&staging).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    for part in &parts {
        if part.index != seen_index + 1 {
            return Err(format!(
                "missing or out-of-range part index {}; expected {}",
                part.index,
                seen_index + 1
            ));
        }
        seen_index = part.index;
        let path = parts_dir.join(&part.file);
        if !path.is_file() {
            return Err(format!("missing part file {}", part.file));
        }
        let hashed = hash_file(&path).map_err(|e| e.to_string())?;
        if hashed.sha256 != part.sha256 || hashed.bytes != part.bytes {
            return Err(format!(
                "part {} checksum/size mismatch (want sha256 {} bytes {}",
                part.file, part.sha256, part.bytes
            ));
        }
        let mut reader = BufReader::new(File::open(&path).map_err(|e| e.to_string())?);
        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            out_file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            hasher.update(&buf[..n]);
        }
    }
    let total = hex::encode(hasher.finalize());
    if total != manifest.total_sha256 {
        return Err("concatenated tar sha256 does not match manifest total_sha256".into());
    }
    Ok(staging)
}

pub fn unpack_tar_to_bundle(tar_path: &Path, out: &Path) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let root = out.canonicalize().map_err(|e| e.to_string())?;
    let file = File::open(tar_path).map_err(|e| e.to_string())?;
    let mut archive = tar::Archive::new(file);
    let mut seen_paths: HashSet<String> = HashSet::new();
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        refuse_stray_tar_metadata(entry.header())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        let rel = validate_resolved_tar_path(&path)?;
        if !seen_paths.insert(rel.clone()) {
            return Err(format!("duplicate tar path in bundle join: {rel}"));
        }
        if let Ok(Some(link)) = entry.link_name() {
            let link_path = link.into_owned();
            validate_resolved_tar_path(&link_path)?;
        }
        refuse_symlink_in_target(&root, &root.join(&rel))?;
        let dest = root.join(&rel);
        if !dest.starts_with(&root) {
            return Err(format!("tar path escapes output: {rel}"));
        }
        for comp in Path::new(&rel).components() {
            if matches!(
                comp,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            ) {
                return Err(format!("tar path forbidden component: {rel}"));
            }
        }
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        } else if entry_type.is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                refuse_symlink_in_target(&root, dest.as_path())?;
            }
            copy_reader_nofollow(&dest, &mut entry, false)?;
        } else if entry_type.is_symlink() || entry_type == EntryType::Link {
            return Err(format!("refusing tar symlink/hardlink entry: {rel}"));
        } else {
            return Err(format!("refusing tar entry type for {rel}"));
        }
    }
    Ok(())
}

pub fn join_bundle(opts: JoinOpts) -> Result<JoinReport, String> {
    refuse_non_empty_dir(&opts.out, "target")?;
    let manifest = PartsManifest::read(&opts.parts_json)?;
    let parts_dir = manifest.path(&opts.parts_json);
    let staging = concat_parts(&manifest, &parts_dir)?;
    let join_result = (|| {
        unpack_tar_to_bundle(&staging, &opts.out)?;
        let report = verify_bundle(&opts.out);
        if !report.ok {
            return Err(report.errors.join("; "));
        }
        Ok(JoinReport {
            bundle_root: opts.out.clone(),
        })
    })();
    let _ = fs::remove_file(&staging);
    if join_result.is_err() {
        let _ = fs::remove_dir_all(&opts.out);
    }
    join_result
}

fn tar_set_checksum(block: &mut [u8; 512]) {
    for b in &mut block[148..156] {
        *b = b' ';
    }
    let sum: u32 = block.iter().map(|&x| u32::from(x)).sum();
    let chk = format!("{sum:06o}\0");
    block[148..148 + chk.len()].copy_from_slice(chk.as_bytes());
}

/// Craft a tar with a GNU long-name extension whose payload is `../escape`.
#[doc(hidden)]
pub fn build_evil_gnu_longname_tar(writer: &mut Vec<u8>) -> Result<(), String> {
    let mut builder = Builder::new(writer);
    let mut long_header = Header::new_gnu();
    long_header
        .set_path("././@LongLink")
        .map_err(|e| e.to_string())?;
    const EVIL: &[u8] = b"../escape\0";
    long_header.set_size(EVIL.len() as u64);
    long_header.set_entry_type(EntryType::GNULongName);
    long_header.set_cksum();
    builder
        .append(&long_header, EVIL)
        .map_err(|e| e.to_string())?;
    let mut file_header = Header::new_gnu();
    file_header.set_path("x").map_err(|e| e.to_string())?;
    file_header.set_size(4);
    file_header.set_entry_type(EntryType::Regular);
    file_header.set_cksum();
    builder
        .append(&file_header, &b"evil"[..])
        .map_err(|e| e.to_string())?;
    builder.into_inner().map_err(|e| e.to_string())?;
    Ok(())
}

#[doc(hidden)]
pub fn build_evil_tar(writer: &mut Vec<u8>) -> Result<(), String> {
    let mut block = [0u8; 512];
    block[..9].copy_from_slice(b"../escape");
    block[156] = b'0';
    block[124..136].copy_from_slice(b"000000000004");
    block[136..148].copy_from_slice(b"000000000000");
    tar_set_checksum(&mut block);
    writer.extend_from_slice(&block);
    writer.extend_from_slice(b"evil");
    let pad = 512 - (writer.len() % 512);
    if pad < 512 {
        writer.extend(vec![0u8; pad]);
    }
    writer.extend([0u8; 512]);
    Ok(())
}

#[doc(hidden)]
pub fn build_symlink_tar(writer: &mut Vec<u8>) -> Result<(), String> {
    let mut header = Header::new_gnu();
    header.set_path("link.txt").map_err(|e| e.to_string())?;
    header.set_size(0);
    header.set_entry_type(EntryType::Symlink);
    header
        .set_link_name("target.txt")
        .map_err(|e| e.to_string())?;
    header.set_cksum();
    let mut builder = Builder::new(writer);
    builder
        .append(&header, &mut std::io::empty())
        .map_err(|e| e.to_string())?;
    builder.into_inner().map_err(|e| e.to_string())?;
    Ok(())
}
