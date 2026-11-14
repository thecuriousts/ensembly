//! Published writer for the application ledger (opportunity-finder pack only).
//!
//! ```compile_fail
//! use ensembly_opportunity_finder::ledger_writer::LedgerWriter;
//! LedgerWriter::open("/tmp/x").unwrap();
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use ensembly_schema::{
    check_seal, grew_without_rewrite, link_key, lock, map_ledger_status, read_bytes, seal_path,
    unlock, write_seal, LedgerRow, SealedIoError,
};
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
    #[cfg_attr(not(feature = "kernel-bridge"), allow(dead_code))]
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, LedgerWriterError> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
        })
    }

    /// Seal an empty or missing ledger. Refuses a non-empty ledger or an existing seal.
    pub fn init_ledger(&self) -> Result<(), LedgerWriterError> {
        let path = &self.path;
        let bytes = read_bytes(path).map_err(map_sealed)?;
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
        write_seal(path, b"").map_err(map_sealed)?;
        lock(path).map_err(map_sealed)
    }

    /// Append one ledger row, or skip a retry with the same key and payload.
    pub fn append_ledger(&self, row: &LedgerRow) -> Result<LedgerWriteReceipt, LedgerWriterError> {
        let path = &self.path;
        let key = idempotency_key(row);
        let before = read_bytes(path).map_err(map_sealed)?;
        check_seal(path, &before).map_err(map_sealed)?;
        if let Some(decision) = existing_decision(&before, row, &key)? {
            return Ok(decision);
        }
        unlock(path).map_err(map_sealed)?;
        let result = application_ledger::append_row(path, row)
            .map_err(|err| LedgerWriterError::Ledger(err.to_string()));
        let after_result = result.and_then(|_| {
            let after = read_bytes(path).map_err(map_sealed)?;
            if !grew_without_rewrite(&before, &after) {
                return Err(LedgerWriterError::Ledger(
                    "history was rewritten; the writer will not append".into(),
                ));
            }
            write_seal(path, &after).map_err(map_sealed)?;
            Ok(after)
        });
        let lock = lock(path).map_err(map_sealed);
        after_result?;
        lock?;
        Ok(LedgerWriteReceipt {
            key,
            appended: true,
        })
    }
}

#[cfg(feature = "kernel-bridge")]
#[doc(hidden)]
pub fn open_kernel_ledger_writer(
    path: impl AsRef<Path>,
) -> Result<LedgerWriter, LedgerWriterError> {
    LedgerWriter::open(path)
}

fn map_sealed(err: SealedIoError) -> LedgerWriterError {
    match err {
        SealedIoError::Io(msg) | SealedIoError::Refused(msg) => LedgerWriterError::Ledger(msg),
    }
}

/// Canonical `link_key` plus a tab plus the record-layer stage token.
pub fn idempotency_key(row: &LedgerRow) -> String {
    format!(
        "{}\t{}",
        link_key(&row.link),
        map_ledger_status(row.status).token
    )
}

fn existing_decision(
    before: &[u8],
    row: &LedgerRow,
    key: &str,
) -> Result<Option<LedgerWriteReceipt>, LedgerWriterError> {
    let text = std::str::from_utf8(before).map_err(|e| LedgerWriterError::Ledger(e.to_string()))?;
    if text.is_empty() {
        return Ok(None);
    }
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let prev: LedgerRow = serde_json::from_str(line)
            .map_err(|e| LedgerWriterError::Ledger(format!("parse ledger: {e}")))?;
        if idempotency_key(&prev) != key {
            continue;
        }
        if same_payload(&prev, row) {
            return Ok(Some(LedgerWriteReceipt {
                key: key.into(),
                appended: false,
            }));
        }
        return Err(LedgerWriterError::Conflict {
            key: key.into(),
            detail: "a different payload is already stored for this key".into(),
        });
    }
    Ok(None)
}

fn same_payload(left: &LedgerRow, right: &LedgerRow) -> bool {
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
