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

use std::path::{Path, PathBuf};

use ensembly_schema::require_configured_writer;
#[cfg(feature = "opportunity-finder")]
use ensembly_schema::{
    check_seal as schema_check_seal, grew_without_rewrite, lock as schema_lock, read_bytes,
    seal_path as schema_seal_path, unlock as schema_unlock, write_seal as schema_write_seal,
    SealedIoError,
};
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
    read_bytes(path).map_err(map_sealed)
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn sealed_grew_without_rewrite(before: &[u8], after: &[u8]) -> bool {
    grew_without_rewrite(before, after)
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn unlock_sealed(path: &Path) -> Result<(), WriterError> {
    schema_unlock(path).map_err(map_sealed)
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn lock_sealed(path: &Path) -> Result<(), WriterError> {
    schema_lock(path).map_err(map_sealed)
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn seal_path(ledger: &Path) -> PathBuf {
    schema_seal_path(ledger)
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn check_seal(path: &Path, bytes: &[u8]) -> Result<(), WriterError> {
    schema_check_seal(path, bytes).map_err(map_sealed)
}

#[cfg(feature = "opportunity-finder")]
pub(crate) fn write_seal(path: &Path, bytes: &[u8]) -> Result<(), WriterError> {
    schema_write_seal(path, bytes).map_err(map_sealed)
}

#[cfg(feature = "opportunity-finder")]
fn map_sealed(err: SealedIoError) -> WriterError {
    match err {
        SealedIoError::Refused(msg) => WriterError::Refused(msg),
        SealedIoError::Io(msg) => WriterError::Ledger(msg),
    }
}

pub(crate) fn store(err: impl ToString) -> WriterError {
    WriterError::Store(err.to_string())
}
