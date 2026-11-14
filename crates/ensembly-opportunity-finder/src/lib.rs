//! Optional **opportunity-finder** pack: application ledger, company audits, tracker export.
//!
//! Off by default. Enable with `ensembly-kernel` feature `opportunity-finder` for CLI
//! subcommands and `ensembly_kernel::application_ledger` re-exports.
//!
//! # Kernel bridge
//!
//! ```compile_fail
//! ensembly_opportunity_finder::open_kernel_ledger_writer(std::path::Path::new("x")).unwrap();
//! ```

pub mod ledger_writer;

pub mod application_ledger;
pub mod apps_record;
pub mod company_audit;
pub mod tracker;

pub use application_ledger::{LedgerRow, Row, Status, LEGACY_UNKNOWN_CV};
pub use apps_record::{compare_files, import_ledger, ImportReceipt};
pub use company_audit::{Audit, GateStatus};
#[cfg(feature = "kernel-bridge")]
#[doc(hidden)]
pub use ledger_writer::open_kernel_ledger_writer;
pub use ledger_writer::{idempotency_key, LedgerWriteReceipt, LedgerWriterError};
