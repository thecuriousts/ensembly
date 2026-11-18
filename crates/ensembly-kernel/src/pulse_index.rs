//! Opt-in speed indexes on the pulse-memory SQLite file (not owned by ensembly).
//!
//! Nothing here runs implicitly: only `ensembly pulse index apply|revert|status`.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, Transaction, TransactionBehavior};
use serde::Serialize;
use std::path::Path;
use std::time::Duration;

pub const INDEX_LAST_SEEN: &str = "ensembly_memory_traces_last_seen";
pub const INDEX_LIVE_LIST: &str = "ensembly_memory_traces_live_list";
pub const INDEX_SUPERSEDED_BY: &str = "ensembly_memory_traces_superseded_by";

pub const MANAGED_INDEXES: &[&str] = &[INDEX_LAST_SEEN, INDEX_LIVE_LIST, INDEX_SUPERSEDED_BY];

const AUDIT_TABLE: &str = "ensembly_audit";

const SQL_LAST_SEEN: &str =
    "CREATE INDEX IF NOT EXISTS ensembly_memory_traces_last_seen ON memory_traces (last_seen DESC)";
const SQL_LIVE_LIST: &str = "CREATE INDEX IF NOT EXISTS ensembly_memory_traces_live_list \
     ON memory_traces (time DESC, nat_key ASC) \
     WHERE kind = 'memory' AND (superseded_by IS NULL OR superseded_by = '')";
const SQL_SUPERSEDED_BY: &str =
    "CREATE INDEX IF NOT EXISTS ensembly_memory_traces_superseded_by ON memory_traces (superseded_by)";

const ENSURE_AUDIT: &str = "CREATE TABLE IF NOT EXISTS ensembly_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts TEXT NOT NULL,
    writer TEXT NOT NULL,
    action TEXT NOT NULL,
    sql_text TEXT NOT NULL,
    outcome TEXT NOT NULL
)";

#[derive(Debug, Clone, Copy)]
pub struct ApplyOptions {
    pub dry_run: bool,
    /// Run `ANALYZE memory_traces` after indexes (writes `sqlite_stat*`; not undone by revert).
    pub analyze: bool,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            analyze: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexStatusRow {
    pub name: String,
    pub present: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexApplyReport {
    pub applied: Vec<String>,
    pub already_present: Vec<String>,
    pub dry_run: bool,
    pub analyzed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexRevertReport {
    pub dropped: Vec<String>,
    pub absent: Vec<String>,
    pub dry_run: bool,
}

pub fn managed_sql_statements() -> Vec<(&'static str, &'static str)> {
    vec![
        (INDEX_LAST_SEEN, SQL_LAST_SEEN),
        (INDEX_LIVE_LIST, SQL_LIVE_LIST),
        (INDEX_SUPERSEDED_BY, SQL_SUPERSEDED_BY),
    ]
}

pub fn status(conn: &Connection) -> Result<Vec<IndexStatusRow>> {
    let mut out = Vec::with_capacity(MANAGED_INDEXES.len());
    for name in MANAGED_INDEXES {
        let present: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
            [*name],
            |r| r.get(0),
        )?;
        out.push(IndexStatusRow {
            name: (*name).to_string(),
            present: present > 0,
        });
    }
    Ok(out)
}

pub fn apply(path: &Path, writer: &str, opts: ApplyOptions) -> Result<IndexApplyReport> {
    apply_with_statements(path, writer, opts, &managed_sql_statements())
}

/// Test hook: run apply DDL with a custom statement list (same transaction rules).
#[doc(hidden)]
pub fn apply_with_statements(
    path: &Path,
    writer: &str,
    opts: ApplyOptions,
    statements: &[(&str, &str)],
) -> Result<IndexApplyReport> {
    let mut report = IndexApplyReport {
        applied: Vec::new(),
        already_present: Vec::new(),
        dry_run: opts.dry_run,
        analyzed: false,
    };
    if opts.dry_run {
        for (name, sql) in statements {
            if index_exists_at_path(path, name)? {
                report.already_present.push(name.to_string());
            } else {
                report.applied.push(name.to_string());
            }
            println!("{sql};");
        }
        if opts.analyze {
            println!("ANALYZE memory_traces;");
        }
        return Ok(report);
    }

    let mut conn = open_rw(path)?;
    let presence: Vec<(String, bool)> = statements
        .iter()
        .map(|(name, _)| {
            let existed = index_exists(&conn, name)?;
            Ok(((*name).to_string(), existed))
        })
        .collect::<Result<_>>()?;

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .context("begin apply transaction")?;
    ensure_audit_table(&tx)?;
    apply_statements_in_tx(&tx, writer, statements)?;
    if opts.analyze {
        tx.execute_batch("ANALYZE memory_traces")
            .context("ANALYZE memory_traces")?;
        writeln_audit(&tx, writer, "index.analyze", "ANALYZE memory_traces", "ok")?;
        report.analyzed = true;
    }
    tx.commit().context("commit apply transaction")?;

    for (name, existed) in presence {
        if existed {
            report.already_present.push(name);
        } else {
            report.applied.push(name);
        }
    }
    Ok(report)
}

pub fn revert(path: &Path, writer: &str, dry_run: bool) -> Result<IndexRevertReport> {
    let mut report = IndexRevertReport {
        dropped: Vec::new(),
        absent: Vec::new(),
        dry_run,
    };
    if dry_run {
        for name in MANAGED_INDEXES {
            let drop_sql = format!("DROP INDEX IF EXISTS {name}");
            if index_exists_at_path(path, name)? {
                report.dropped.push(name.to_string());
                println!("{drop_sql};");
            } else {
                report.absent.push(name.to_string());
            }
        }
        return Ok(report);
    }

    let mut conn = open_rw(path)?;
    let presence: Vec<(String, bool)> = MANAGED_INDEXES
        .iter()
        .map(|name| {
            let existed = index_exists(&conn, name)?;
            Ok(((*name).to_string(), existed))
        })
        .collect::<Result<_>>()?;

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .context("begin revert transaction")?;
    ensure_audit_table(&tx)?;
    for name in MANAGED_INDEXES {
        let drop_sql = format!("DROP INDEX IF EXISTS {name}");
        tx.execute(&drop_sql, [])
            .with_context(|| format!("execute {drop_sql}"))?;
        writeln_audit(&tx, writer, "index.revert", &drop_sql, "ok")?;
    }
    tx.commit().context("commit revert transaction")?;

    for (name, existed) in presence {
        if existed {
            report.dropped.push(name);
        } else {
            report.absent.push(name);
        }
    }
    Ok(report)
}

fn apply_statements_in_tx(
    tx: &Transaction<'_>,
    writer: &str,
    statements: &[(&str, &str)],
) -> Result<()> {
    for (_, sql) in statements {
        tx.execute_batch(sql)
            .with_context(|| format!("execute {sql}"))?;
        writeln_audit(tx, writer, "index.apply", sql, "ok")?;
    }
    Ok(())
}

fn index_exists_at_path(path: &Path, name: &str) -> Result<bool> {
    let conn = crate::pulse_core::open_ro(path)?;
    index_exists(&conn, name)
}

fn index_exists(conn: &Connection, name: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
        [name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub fn audit_table_exists(conn: &Connection) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [AUDIT_TABLE],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

fn open_rw(path: &Path) -> Result<Connection> {
    if !path.is_file() {
        anyhow::bail!("pulse db missing");
    }
    let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
    conn.busy_timeout(Duration::from_secs(5))?;
    crate::write_guard::grant_session(&conn)?;
    crate::write_guard::install_triggers(&conn)?;
    Ok(conn)
}

fn ensure_audit_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(ENSURE_AUDIT)?;
    Ok(())
}

fn writeln_audit(
    conn: &Connection,
    writer: &str,
    action: &str,
    sql_text: &str,
    outcome: &str,
) -> Result<()> {
    let ts = chrono::Utc::now().to_rfc3339();
    conn.execute(
        &format!(
            "INSERT INTO {AUDIT_TABLE} (ts, writer, action, sql_text, outcome) VALUES (?1, ?2, ?3, ?4, ?5)"
        ),
        params![ts, writer, action, sql_text, outcome],
    )?;
    Ok(())
}
