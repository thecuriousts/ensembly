//! Application-ledger writer methods (opportunity-finder pack).

use std::path::Path;

use ensembly_opportunity_finder::ledger_writer::{LedgerWriter, LedgerWriterError};
use ensembly_schema::LedgerRow;

use super::writer::{KernelWriter, WriterError};

/// Result of a ledger append: either a new line, or a retry of the same key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerWriteReceipt {
    pub key: String,
    pub appended: bool,
}

pub use ensembly_opportunity_finder::ledger_writer::idempotency_key;

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
            decisions: None,
        })
    }

    pub fn init_ledger(&self) -> Result<(), WriterError> {
        self.ledger_writer()?.init_ledger().map_err(map_ledger_err)
    }

    pub fn append_ledger(&self, row: &LedgerRow) -> Result<LedgerWriteReceipt, WriterError> {
        let receipt = self
            .ledger_writer()?
            .append_ledger(row)
            .map_err(map_ledger_err)?;
        Ok(LedgerWriteReceipt {
            key: receipt.key,
            appended: receipt.appended,
        })
    }

    pub(crate) fn ledger_path(&self) -> Result<&Path, WriterError> {
        self.ledger
            .as_deref()
            .ok_or_else(|| WriterError::Refused("ledger path is not open".into()))
    }

    fn ledger_writer(&self) -> Result<LedgerWriter, WriterError> {
        let path = self.ledger_path()?;
        LedgerWriter::attach_for_kernel(path).map_err(|e| WriterError::Ledger(e.to_string()))
    }
}

fn map_ledger_err(err: LedgerWriterError) -> WriterError {
    match err {
        LedgerWriterError::Conflict { key, detail } => WriterError::Conflict { key, detail },
        LedgerWriterError::Ledger(msg) => WriterError::Ledger(msg),
    }
}
