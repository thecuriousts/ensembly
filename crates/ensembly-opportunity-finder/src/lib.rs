//! Optional **opportunity-finder** pack: application ledger, company audits, tracker export.
//!
//! Off by default. Enable with `ensembly-kernel` feature `opportunity-finder` for CLI
//! subcommands and `ensembly_kernel::application_ledger` re-exports.

pub mod application_ledger;
pub mod apps_record;
pub mod company_audit;
pub mod ledger_writer;
pub mod tracker;

pub use application_ledger::{LedgerRow, Row, Status, LEGACY_UNKNOWN_CV};
pub use apps_record::{compare_files, import_ledger, ImportReceipt};
pub use company_audit::{Audit, GateStatus};
pub use ledger_writer::{idempotency_key, LedgerWriteReceipt, LedgerWriter, LedgerWriterError};

use std::path::Path;

/// Kernel-only entry for the application ledger writer ([`KernelWriter::open_ledger`]).
pub fn open_application_ledger_writer(
    path: impl AsRef<Path>,
) -> Result<LedgerWriter, LedgerWriterError> {
    LedgerWriter::open(path)
}
