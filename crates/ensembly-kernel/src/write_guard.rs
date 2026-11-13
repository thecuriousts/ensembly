//! Per-connection write grant and durable pulse-store triggers.
//!
//! Triggers call a SQL function that only a kernel connection registers.
//! A bot that opens the same file with rusqlite has no such function and
//! the write is refused.

use rusqlite::functions::FunctionFlags;
use rusqlite::Connection;

const GRANT_FN: &str = "kernel_writer_grant";
const TABLES: &[&str] = &["memory_traces", "supersession_events", "conflicts"];

/// Allow this connection to pass the pulse write triggers.
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
