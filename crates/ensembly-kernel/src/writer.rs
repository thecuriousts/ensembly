//! The published writer for pulse stores on the operator machine.
//!
//! Application-ledger and decision-log writes live in the optional opportunity-finder
//! pack (`--features opportunity-finder`); see [`crate::writer_ledger`] and
//! [`crate::writer_decision`] when enabled.
//!
//! Writer identity comes from the `ENSEMBLY_WRITER` environment variable (validated
//! as non-empty and safe via [`ensembly_schema::require_configured_writer`]).
//!
//! Pulse stores get accident-guard triggers on every canonical write open. Those
//! triggers are not a security boundary: a local process that can open the file can
//! register the same SQL function or drop them. File permissions and ownership limit
//! local tooling.
//!
//! For other crates, [`KernelWriter`] is the only type that can admit, supersede, or
//! resolve a pulse write when the pulse DB is open. Raw write functions are crate-private.
//!
//! # Compile-fail checks
//!
//! ```compile_fail
//! let _ = ensembly_kernel::pulse_supersede::admit;
//! ```
//!

use std::path::{Path, PathBuf};

#[cfg(feature = "opportunity-finder")]
use std::{fs, io};

use ensembly_schema::require_configured_writer;
use thiserror::Error;

use crate::pulse_supersede::{
    AdmitReceipt, AdmitRequest, ResolveRequest, SupersedeError, SupersedeFault, SupersedeReceipt,
    SupersedeRequest,
};
use crate::write_guard;

#[derive(Debug, Error)]
pub enum WriterError {
    #[error("{0}")]
    Refused(String),
    #[error("idempotency conflict on key {key}: {detail}")]
    Conflict { key: String, detail: String },
    #[error("ledger: {0}")]
    Ledger(String),
    #[error("pulse: {0}")]
    Pulse(#[from] SupersedeError),
    #[error("store: {0}")]
    Store(String),
}

/// The published writer for pulse stores (and optionally ledger / decision logs).
pub struct KernelWriter {
    #[cfg(feature = "opportunity-finder")]
    pub(crate) ledger: Option<PathBuf>,
    pub(crate) pulse_db: Option<PathBuf>,
    #[cfg(feature = "opportunity-finder")]
    pub(crate) decisions: Option<PathBuf>,
}

impl KernelWriter {
    pub fn open_pulse(path: impl AsRef<Path>) -> Result<Self, WriterError> {
        Self::empty().with_pulse(path)
    }

    pub(crate) fn empty() -> Self {
        Self {
            #[cfg(feature = "opportunity-finder")]
            ledger: None,
            pulse_db: None,
            #[cfg(feature = "opportunity-finder")]
            decisions: None,
        }
    }

    pub(crate) fn with_pulse(mut self, path: impl AsRef<Path>) -> Result<Self, WriterError> {
        let path = path.as_ref().to_path_buf();
        if !path.is_file() {
            return Err(WriterError::Store("pulse db missing".into()));
        }
        let conn = rusqlite::Connection::open(&path).map_err(store)?;
        write_guard::grant_session(&conn).map_err(store)?;
        write_guard::install_triggers(&conn).map_err(store)?;
        self.pulse_db = Some(path);
        Ok(self)
    }

    pub fn admit(
        &self,
        req: &AdmitRequest,
        fault: &SupersedeFault,
    ) -> Result<AdmitReceipt, WriterError> {
        let path = self.pulse_path()?;
        require_writer(&req.writer)?;
        let receipt = crate::pulse_supersede::admit(path, req, fault)?;
        self.refresh_pulse_guard()?;
        Ok(receipt)
    }

    pub fn supersede(
        &self,
        req: &SupersedeRequest,
        fault: &SupersedeFault,
    ) -> Result<SupersedeReceipt, WriterError> {
        let path = self.pulse_path()?;
        require_writer(&req.writer)?;
        let receipt = crate::pulse_supersede::supersede(path, req, fault)?;
        self.refresh_pulse_guard()?;
        Ok(receipt)
    }

    pub fn resolve_conflict(
        &self,
        req: &ResolveRequest,
        fault: &SupersedeFault,
    ) -> Result<SupersedeReceipt, WriterError> {
        let path = self.pulse_path()?;
        require_writer(&req.writer)?;
        let receipt = crate::pulse_supersede::resolve_conflict(path, req, fault)?;
        self.refresh_pulse_guard()?;
        Ok(receipt)
    }

    fn refresh_pulse_guard(&self) -> Result<(), WriterError> {
        let path = self.pulse_path()?;
        let conn = rusqlite::Connection::open(path).map_err(store)?;
        write_guard::grant_session(&conn).map_err(store)?;
        write_guard::install_triggers(&conn).map_err(store)?;
        Ok(())
    }

    fn pulse_path(&self) -> Result<&Path, WriterError> {
        self.pulse_db
            .as_deref()
            .ok_or_else(|| WriterError::Refused("pulse db is not open".into()))
    }
}

pub(crate) fn require_writer(writer: &str) -> Result<(), WriterError> {
    require_configured_writer(writer).map_err(WriterError::Refused)?;
    Ok(())
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn read_sealed_bytes(path: &Path) -> Result<Vec<u8>, WriterError> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(WriterError::Ledger(format!(
            "read {}: {err}",
            path.display()
        ))),
    }
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn sealed_grew_without_rewrite(before: &[u8], after: &[u8]) -> bool {
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

#[cfg(feature = "opportunity-finder")]
pub(crate) fn unlock_sealed(path: &Path) -> Result<(), WriterError> {
    if path.exists() {
        set_sealed_mode(path, 0o644)?;
    }
    let seal = seal_path(path);
    if seal.exists() {
        set_sealed_mode(&seal, 0o644)?;
    }
    Ok(())
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn lock_sealed(path: &Path) -> Result<(), WriterError> {
    if path.exists() {
        set_sealed_mode(path, 0o444)?;
    }
    let seal = seal_path(path);
    if seal.exists() {
        set_sealed_mode(&seal, 0o444)?;
    }
    Ok(())
}

#[cfg(feature = "opportunity-finder")]
fn set_sealed_mode(path: &Path, mode: u32) -> Result<(), WriterError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|err| WriterError::Ledger(format!("permissions {}: {err}", path.display())))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn seal_path(ledger: &Path) -> PathBuf {
    let mut name = ledger
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("ledger.jsonl"))
        .to_os_string();
    name.push(".writer-hash");
    ledger.with_file_name(name)
}

#[cfg(feature = "opportunity-finder")]
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn check_seal(path: &Path, bytes: &[u8]) -> Result<(), WriterError> {
    let seal = seal_path(path);
    match fs::read_to_string(&seal) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(WriterError::Ledger(
            "seal is missing; the writer will not adopt current bytes".into(),
        )),
        Ok(expected) => {
            if expected.trim() == sha256_hex(bytes) {
                Ok(())
            } else {
                Err(WriterError::Ledger(
                    "history was rewritten; the writer will not append".into(),
                ))
            }
        }
        Err(err) => Err(WriterError::Ledger(format!(
            "read {}: {err}",
            seal.display()
        ))),
    }
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn write_seal(path: &Path, bytes: &[u8]) -> Result<(), WriterError> {
    let seal = seal_path(path);
    if seal.exists() {
        set_sealed_mode(&seal, 0o644)?;
    }
    fs::write(
        &seal,
        format!(
            "{}
",
            sha256_hex(bytes)
        ),
    )
    .map_err(|err| WriterError::Ledger(format!("write {}: {err}", seal.display())))?;
    set_sealed_mode(&seal, 0o444)
}

pub(crate) fn store(err: impl ToString) -> WriterError {
    WriterError::Store(err.to_string())
}
