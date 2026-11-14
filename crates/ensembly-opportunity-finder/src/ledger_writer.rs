//! Published writer for the application ledger (opportunity-finder pack only).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ensembly_schema::{link_key, map_ledger_status, LedgerRow};
use thiserror::Error;

use crate::application_ledger;

/// Result of a ledger append: either a new line, or a retry of the same key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerWriteReceipt {
    pub key: String,
    pub appended: bool,
}

#[derive(Debug, Error)]
pub enum LedgerWriterError {
    #[error("idempotency conflict on key {key}: {detail}")]
    Conflict { key: String, detail: String },
    #[error("ledger: {0}")]
    Ledger(String),
}

/// The published writer for the application ledger JSONL store.
pub struct LedgerWriter {
    path: PathBuf,
}

impl LedgerWriter {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, LedgerWriterError> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
        })
    }

    /// Used only from `ensembly-kernel` `KernelWriter::open_ledger`; not a public writer API.
    #[doc(hidden)]
    pub fn attach_for_kernel(path: impl AsRef<Path>) -> Result<Self, LedgerWriterError> {
        Self::open(path)
    }

    /// Seal an empty or missing ledger. Refuses a non-empty ledger or an existing seal.
    pub fn init_ledger(&self) -> Result<(), LedgerWriterError> {
        let path = &self.path;
        let bytes = read_ledger_bytes(path)?;
        if !bytes.is_empty() {
            return Err(LedgerWriterError::Ledger(
                "ledger already has rows; init is only for a new empty ledger".into(),
            ));
        }
        if seal_path(path).exists() {
            return Err(LedgerWriterError::Ledger("ledger is already sealed".into()));
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|err| {
                LedgerWriterError::Ledger(format!("create {}: {err}", dir.display()))
            })?;
        }
        if !path.exists() {
            fs::write(path, b"").map_err(|err| {
                LedgerWriterError::Ledger(format!("create {}: {err}", path.display()))
            })?;
        }
        write_seal(path, b"")?;
        lock_ledger(path)
    }

    /// Append one ledger row, or skip a retry with the same key and payload.
    pub fn append_ledger(&self, row: &LedgerRow) -> Result<LedgerWriteReceipt, LedgerWriterError> {
        let path = &self.path;
        let key = idempotency_key(row);
        let before = read_ledger_bytes(path)?;
        check_seal(path, &before)?;
        if let Some(decision) = existing_decision(&before, row, &key)? {
            return Ok(decision);
        }
        unlock_ledger(path)?;
        let result = application_ledger::append_row(path, row)
            .map_err(|err| LedgerWriterError::Ledger(err.to_string()));
        let after_result = result.and_then(|_| {
            let after = read_ledger_bytes(path)?;
            if !ledger_grew_without_rewrite(&before, &after) {
                return Err(LedgerWriterError::Ledger(
                    "ledger history was rewritten; the writer will not keep a mutated prefix"
                        .into(),
                ));
            }
            write_seal(path, &after)?;
            Ok(after)
        });
        let lock = lock_ledger(path);
        after_result?;
        lock?;
        Ok(LedgerWriteReceipt {
            key,
            appended: true,
        })
    }
}

/// `link_key` plus a tab plus the record-layer stage token.
pub fn idempotency_key(row: &LedgerRow) -> String {
    format!(
        "{}\t{}",
        link_key(&row.link),
        map_ledger_status(row.status).token
    )
}

fn existing_decision(
    bytes: &[u8],
    row: &LedgerRow,
    key: &str,
) -> Result<Option<LedgerWriteReceipt>, LedgerWriterError> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let text =
        std::str::from_utf8(bytes).map_err(|err| LedgerWriterError::Ledger(err.to_string()))?;
    let rows = application_ledger::parse_ledger(text)
        .map_err(|err| LedgerWriterError::Ledger(err.to_string()))?;
    let matches: Vec<&LedgerRow> = rows
        .iter()
        .filter(|existing| idempotency_key(existing) == key)
        .collect();
    if matches.is_empty() {
        return Ok(None);
    }
    if matches
        .iter()
        .any(|existing| same_ledger_payload(existing, row))
    {
        return Ok(Some(LedgerWriteReceipt {
            key: key.to_string(),
            appended: false,
        }));
    }
    Err(LedgerWriterError::Conflict {
        key: key.to_string(),
        detail: "payload for this stage differs from a stored row. The writer does not rewrite a stored row. Append a later stage.".into(),
    })
}

fn same_ledger_payload(left: &LedgerRow, right: &LedgerRow) -> bool {
    left.company.trim() == right.company.trim()
        && left.role.trim() == right.role.trim()
        && link_key(left.link.trim()) == link_key(right.link.trim())
        && normalize_url(left.apply_url.as_deref()) == normalize_url(right.apply_url.as_deref())
        && left.status == right.status
        && left.submit_time.as_deref().map(str::trim) == right.submit_time.as_deref().map(str::trim)
        && left.submit_time_unknown == right.submit_time_unknown
        && left.cv_version.trim() == right.cv_version.trim()
}

fn normalize_url(url: Option<&str>) -> Option<String> {
    url.map(|value| value.trim().trim_end_matches('/').to_string())
}

fn read_ledger_bytes(path: &Path) -> Result<Vec<u8>, LedgerWriterError> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(LedgerWriterError::Ledger(format!(
            "read {}: {err}",
            path.display()
        ))),
    }
}

fn ledger_grew_without_rewrite(before: &[u8], after: &[u8]) -> bool {
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

fn unlock_ledger(path: &Path) -> Result<(), LedgerWriterError> {
    if path.exists() {
        set_ledger_mode(path, 0o644)?;
    }
    let seal = seal_path(path);
    if seal.exists() {
        set_ledger_mode(&seal, 0o644)?;
    }
    Ok(())
}

fn lock_ledger(path: &Path) -> Result<(), LedgerWriterError> {
    if path.exists() {
        set_ledger_mode(path, 0o444)?;
    }
    let seal = seal_path(path);
    if seal.exists() {
        set_ledger_mode(&seal, 0o444)?;
    }
    Ok(())
}

fn set_ledger_mode(path: &Path, mode: u32) -> Result<(), LedgerWriterError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|err| {
            LedgerWriterError::Ledger(format!("permissions {}: {err}", path.display()))
        })
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

fn seal_path(ledger: &Path) -> PathBuf {
    let mut name = ledger
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("ledger.jsonl"))
        .to_os_string();
    name.push(".writer-hash");
    ledger.with_file_name(name)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

fn check_seal(path: &Path, bytes: &[u8]) -> Result<(), LedgerWriterError> {
    let seal = seal_path(path);
    match fs::read_to_string(&seal) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(LedgerWriterError::Ledger(
            "ledger seal is missing; the writer will not adopt current bytes. Init only a new empty ledger".into(),
        )),
        Ok(expected) => {
            if expected.trim() == sha256_hex(bytes) {
                Ok(())
            } else {
                Err(LedgerWriterError::Ledger(
                    "ledger history was rewritten; the writer will not append".into(),
                ))
            }
        }
        Err(err) => Err(LedgerWriterError::Ledger(format!(
            "read {}: {err}",
            seal.display()
        ))),
    }
}

fn write_seal(path: &Path, bytes: &[u8]) -> Result<(), LedgerWriterError> {
    let seal = seal_path(path);
    if seal.exists() {
        set_ledger_mode(&seal, 0o644)?;
    }
    fs::write(
        &seal,
        format!(
            "{}
",
            sha256_hex(bytes)
        ),
    )
    .map_err(|err| LedgerWriterError::Ledger(format!("write {}: {err}", seal.display())))?;
    set_ledger_mode(&seal, 0o444)
}
