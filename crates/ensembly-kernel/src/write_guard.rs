//! Accident-guard triggers for the pulse store.
//!
//! These triggers ask for a per-connection SQL function. That is enough to
//! catch a forgotten raw INSERT from tests or a casual CLI, but it is not a
//! security boundary: any local process that can open the file can register
//! the same function or drop the trigger. Crate users are limited by the
//! public API ([`crate::KernelWriter`] is the only published writer). Local
//! tooling is limited by file permissions and ownership. Anything beyond
//! that is out of scope.

use rusqlite::functions::FunctionFlags;
use rusqlite::Connection;

const GRANT_FN: &str = "kernel_writer_grant";
const TABLES: &[&str] = &["memory_traces", "supersession_events", "conflicts"];

/// Mark this connection as a kernel write connection so the accident-guard
/// triggers allow it through. Not a secret: any rusqlite client can do this.
pub fn grant_session(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.create_scalar_function(GRANT_FN, 0, FunctionFlags::SQLITE_UTF8, |_| Ok(1i64))
}

/// Install append/update/delete guards on pulse tables that already exist.
/// Missing tables are skipped so a later kernel open can finish the job.
pub fn install_triggers(conn: &Connection) -> Result<(), rusqlite::Error> {
    for table in TABLES {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )?;
        if exists == 0 {
            continue;
        }
        for event in ["INSERT", "UPDATE", "DELETE"] {
            let name = format!("kernel_single_writer_{table}_{}", event.to_lowercase());
            let sql = format!(
                "CREATE TRIGGER IF NOT EXISTS {name} BEFORE {event} ON {table}
                 BEGIN
                   SELECT CASE {GRANT_FN}()
                     WHEN 1 THEN 1
                     ELSE RAISE(ABORT, 'pulse writes must go through the kernel writer')
                   END;
                 END;"
            );
            conn.execute_batch(&sql)?;
        }
    }
    Ok(())
}
