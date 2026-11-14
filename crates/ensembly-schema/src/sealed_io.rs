//! Shared append-only sealed file helpers (JSONL logs, decision logs, ledgers).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum SealedIoError {
    Io(String),
    Refused(String),
}

impl SealedIoError {
    pub fn io(path: &Path, err: impl std::fmt::Display) -> Self {
        Self::Io(format!("read {}: {err}", path.display()))
    }

    pub fn refused(msg: impl Into<String>) -> Self {
        Self::Refused(msg.into())
    }
}

pub fn read_bytes(path: &Path) -> Result<Vec<u8>, SealedIoError> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(SealedIoError::io(path, err)),
    }
}

pub fn grew_without_rewrite(before: &[u8], after: &[u8]) -> bool {
    if before.is_empty() {
        return !after.is_empty();
    }
    if after.starts_with(before) {
        return after.len() > before.len();
    }
    let mut with_nl = before.to_vec();
    if !with_nl.ends_with(b"\n") {
        with_nl.push(b'\n');
    }
    after.starts_with(&with_nl) && after.len() > with_nl.len()
}

pub fn seal_path(data: &Path) -> PathBuf {
    let mut name = data
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("data.jsonl"))
        .to_os_string();
    name.push(".writer-hash");
    data.with_file_name(name)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

pub fn check_seal(path: &Path, bytes: &[u8]) -> Result<(), SealedIoError> {
    let seal = seal_path(path);
    match fs::read_to_string(&seal) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(SealedIoError::refused(
            "seal is missing; the writer will not adopt current bytes",
        )),
        Ok(expected) => {
            if expected.trim() == sha256_hex(bytes) {
                Ok(())
            } else {
                Err(SealedIoError::refused(
                    "history was rewritten; the writer will not append",
                ))
            }
        }
        Err(err) => Err(SealedIoError::io(&seal, err)),
    }
}

pub fn write_seal(path: &Path, bytes: &[u8]) -> Result<(), SealedIoError> {
    let seal = seal_path(path);
    if seal.exists() {
        set_mode(&seal, 0o644)?;
    }
    fs::write(
        &seal,
        format!(
            "{}
",
            sha256_hex(bytes)
        ),
    )
    .map_err(|err| SealedIoError::Io(format!("write {}: {err}", seal.display())))?;
    set_mode(&seal, 0o444)
}

pub fn unlock(path: &Path) -> Result<(), SealedIoError> {
    if path.exists() {
        set_mode(path, 0o644)?;
    }
    let seal = seal_path(path);
    if seal.exists() {
        set_mode(&seal, 0o644)?;
    }
    Ok(())
}

pub fn lock(path: &Path) -> Result<(), SealedIoError> {
    if path.exists() {
        set_mode(path, 0o444)?;
    }
    let seal = seal_path(path);
    if seal.exists() {
        set_mode(&seal, 0o444)?;
    }
    Ok(())
}

fn set_mode(path: &Path, mode: u32) -> Result<(), SealedIoError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|err| SealedIoError::Io(format!("permissions {}: {err}", path.display())))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}
