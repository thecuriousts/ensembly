//! The published writer for ledger and pulse stores on the operator machine.
//!
//! # What is enforced
//!
//! For other crates, [`KernelWriter`] is the only type that can append a
//! ledger row or admit, supersede, or resolve a pulse write. Raw write
//! functions are crate-private. The idempotency key is `link_key` plus a
//! tab plus the record stage token. Old ledger lines are never rewritten
//! and the type has no delete method.
//!
//! A new ledger must be initialised with [`KernelWriter::init_ledger`]
//! before the first append. A missing or mismatched `.writer-hash` seal
//! is refused; the writer will not adopt existing bytes.
//!
//! On Unix, a sealed ledger is marked read-only after each append so
//! casual edits fail. That `chmod` is a no-op on non-Unix systems.
//!
//! Writer identity comes from the `ENSEMBLY_WRITER` environment variable,
//! validated as non-empty and safe. There is no list of machine names in
//! the write path. The request must match that configured value.
//!
//! Pulse stores get accident-guard triggers on every canonical write
//! open. Those triggers are not a security boundary: a local process that
//! can open the file can register the same SQL function or drop them.
//! File permissions and ownership limit local tooling. Anything beyond
//! that is out of scope.
//!
//! # Compile-fail checks
//!
//! Crate users cannot call the raw write functions:
//!
//! ```compile_fail
//! let _ = ensembly_kernel::pulse_supersede::admit;
//! ```
//!
//! ```compile_fail
//! let _ = ensembly_kernel::application_ledger::append_row;
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ensembly_schema::{link_key, map_ledger_status, require_configured_writer};
use thiserror::Error;

use crate::application_ledger::{self, LedgerRow};
use crate::pulse_supersede::{
    AdmitReceipt, AdmitRequest, ResolveRequest, SupersedeError, SupersedeFault, SupersedeReceipt,
    SupersedeRequest,
};
use crate::write_guard;

/// Result of a ledger append: either a new line, or a retry of the same key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerWriteReceipt {
    pub key: String,
    pub appended: bool,
}

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

/// The published writer for the application ledger and the pulse store.
pub struct KernelWriter {
    ledger: Option<PathBuf>,
    pulse_db: Option<PathBuf>,
}

impl KernelWriter {
    /// Open both stores. The pulse file must already exist.
    pub fn open(ledger: impl AsRef<Path>, pulse_db: impl AsRef<Path>) -> Result<Self, WriterError> {
        let writer = Self::open_ledger(ledger)?;
        writer.with_pulse(pulse_db)
    }

    pub fn open_ledger(path: impl AsRef<Path>) -> Result<Self, WriterError> {
        Ok(Self {
            ledger: Some(path.as_ref().to_path_buf()),
            pulse_db: None,
        })
    }

    pub fn open_pulse(path: impl AsRef<Path>) -> Result<Self, WriterError> {
        Self {
            ledger: None,
            pulse_db: None,
        }
        .with_pulse(path)
    }

    fn with_pulse(mut self, path: impl AsRef<Path>) -> Result<Self, WriterError> {
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

    /// Seal an empty or missing ledger. Refuses a non-empty ledger or an
    /// existing seal: those are not a new ledger, and the writer will not
    /// adopt current bytes.
    pub fn init_ledger(&self) -> Result<(), WriterError> {
        let path = self.ledger_path()?;
        let bytes = read_ledger_bytes(path)?;
        if !bytes.is_empty() {
            return Err(WriterError::Ledger(
                "ledger already has rows; init is only for a new empty ledger".into(),
            ));
        }
        if seal_path(path).exists() {
            return Err(WriterError::Ledger("ledger is already sealed".into()));
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .map_err(|err| WriterError::Ledger(format!("create {}: {err}", dir.display())))?;
        }
        if !path.exists() {
            fs::write(path, b"")
                .map_err(|err| WriterError::Ledger(format!("create {}: {err}", path.display())))?;
        }
        write_seal(path, b"")?;
        lock_ledger(path)
    }

    /// Append one ledger row, or skip a retry with the same key and payload.
    pub fn append_ledger(&self, row: &LedgerRow) -> Result<LedgerWriteReceipt, WriterError> {
        let path = self.ledger_path()?;
        let key = idempotency_key(row);
        let before = read_ledger_bytes(path)?;
        check_seal(path, &before)?;
        if let Some(decision) = existing_decision(&before, row, &key)? {
            return Ok(decision);
        }
        unlock_ledger(path)?;
        let result = application_ledger::append_row(path, row)
            .map_err(|err| WriterError::Ledger(err.to_string()));
        let after_result = result.and_then(|_| {
            let after = read_ledger_bytes(path)?;
            if !ledger_grew_without_rewrite(&before, &after) {
                return Err(WriterError::Ledger(
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

    fn ledger_path(&self) -> Result<&Path, WriterError> {
        self.ledger
            .as_deref()
            .ok_or_else(|| WriterError::Refused("ledger path is not open".into()))
    }

    fn pulse_path(&self) -> Result<&Path, WriterError> {
        self.pulse_db
            .as_deref()
            .ok_or_else(|| WriterError::Refused("pulse db is not open".into()))
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

fn require_writer(writer: &str) -> Result<(), WriterError> {
    require_configured_writer(writer).map_err(WriterError::Refused)?;
    Ok(())
}

fn existing_decision(
    bytes: &[u8],
    row: &LedgerRow,
    key: &str,
) -> Result<Option<LedgerWriteReceipt>, WriterError> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let text = std::str::from_utf8(bytes).map_err(|err| WriterError::Ledger(err.to_string()))?;
    let rows = application_ledger::parse_ledger(text)
        .map_err(|err| WriterError::Ledger(err.to_string()))?;
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
    Err(WriterError::Conflict {
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

fn read_ledger_bytes(path: &Path) -> Result<Vec<u8>, WriterError> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(WriterError::Ledger(format!(
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
    // append_row may insert a missing trailing newline before the new row.
    let mut with_nl = before.to_vec();
    if !with_nl.ends_with(b"\n") {
        with_nl.push(b'\n');
    }
    after.starts_with(&with_nl) && after.len() > with_nl.len()
}

fn unlock_ledger(path: &Path) -> Result<(), WriterError> {
    if path.exists() {
        set_ledger_mode(path, 0o644)?;
    }
    let seal = seal_path(path);
    if seal.exists() {
        set_ledger_mode(&seal, 0o644)?;
    }
    Ok(())
}

fn lock_ledger(path: &Path) -> Result<(), WriterError> {
    if path.exists() {
        set_ledger_mode(path, 0o444)?;
    }
    let seal = seal_path(path);
    if seal.exists() {
        set_ledger_mode(&seal, 0o444)?;
    }
    Ok(())
}

/// On Unix this sets the file mode. On other systems it is a no-op, so a
/// writer-managed ledger is not made read-only there.
fn set_ledger_mode(path: &Path, mode: u32) -> Result<(), WriterError> {
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

fn check_seal(path: &Path, bytes: &[u8]) -> Result<(), WriterError> {
    let seal = seal_path(path);
    match fs::read_to_string(&seal) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(WriterError::Ledger(
            "ledger seal is missing; the writer will not adopt current bytes. Init only a new empty ledger".into(),
        )),
        Ok(expected) => {
            if expected.trim() == sha256_hex(bytes) {
                Ok(())
            } else {
                Err(WriterError::Ledger(
                    "ledger history was rewritten; the writer will not append".into(),
                ))
            }
        }
        Err(err) => Err(WriterError::Ledger(format!(
            "read {}: {err}",
            seal.display()
        ))),
    }
}

fn write_seal(path: &Path, bytes: &[u8]) -> Result<(), WriterError> {
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
    .map_err(|err| WriterError::Ledger(format!("write {}: {err}", seal.display())))?;
    set_ledger_mode(&seal, 0o444)
}

fn store(err: impl ToString) -> WriterError {
    WriterError::Store(err.to_string())
}
