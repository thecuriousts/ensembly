//! T1 operational ledger — SQLite WAL, single-writer, crash-safe snapshots.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde_json;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::approvals::Snapshot;
use crate::digital_flow::DigitalFlow;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub struct OpsStore {
    path: PathBuf,
    conn: Connection,
}

impl OpsStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path.as_ref())?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;",
        )?;
        let store = Self {
            path: path.as_ref().to_path_buf(),
            conn,
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let store = Self {
            path: PathBuf::from(":memory:"),
            conn,
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn migrate(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS kv (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS audit (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL,
                kind TEXT NOT NULL,
                payload TEXT NOT NULL
            );",
        )?;
        let v: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if v < 1 {
            self.conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (1, ?1)",
                params![Utc::now().to_rfc3339()],
            )?;
        }
        Ok(())
    }

    pub fn save_snapshot(&self, snap: &Snapshot) -> Result<(), StoreError> {
        let json = serde_json::to_string_pretty(snap)?;
        self.conn.execute(
            "INSERT INTO kv (key, value, updated_at) VALUES ('wait_snapshot', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
            params![json, snap.updated_at.to_rfc3339()],
        )?;
        self.audit("snapshot.save", &json)?;
        Ok(())
    }

    pub fn load_snapshot(&self) -> Result<Option<Snapshot>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM kv WHERE key = 'wait_snapshot'")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let v: String = row.get(0)?;
            Ok(Some(serde_json::from_str(&v)?))
        } else {
            Ok(None)
        }
    }

    pub fn save_flow(&self, flow: &DigitalFlow) -> Result<(), StoreError> {
        let json = serde_json::to_string_pretty(flow)?;
        let key = format!("flow:{}", flow.id);
        self.conn.execute(
            "INSERT INTO kv (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
            params![key, json, flow.updated_at.to_rfc3339()],
        )?;
        self.audit("flow.save", &json)?;
        Ok(())
    }

    pub fn load_flow(&self, id: &str) -> Result<Option<DigitalFlow>, StoreError> {
        let key = format!("flow:{id}");
        let mut stmt = self.conn.prepare("SELECT value FROM kv WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            let v: String = row.get(0)?;
            Ok(Some(serde_json::from_str(&v)?))
        } else {
            Ok(None)
        }
    }

    pub fn audit(&self, kind: &str, payload: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO audit (ts, kind, payload) VALUES (?1, ?2, ?3)",
            params![Utc::now().to_rfc3339(), kind, payload],
        )?;
        Ok(())
    }

    pub fn audit_tail(&self, limit: usize) -> Result<Vec<(String, String, String)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT ts, kind, payload FROM audit ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
        let mut out = vec![];
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Export all kv + schema version for backup pack.
    pub fn export_bundle(&self) -> Result<OpsBundle, StoreError> {
        let mut stmt = self.conn.prepare("SELECT key, value, updated_at FROM kv")?;
        let rows = stmt.query_map([], |r| {
            Ok(KvRow {
                key: r.get(0)?,
                value: r.get(1)?,
                updated_at: r.get(2)?,
            })
        })?;
        let mut kv = vec![];
        for row in rows {
            kv.push(row?);
        }
        let version: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        Ok(OpsBundle {
            format: "peram-ops-bundle-v1".into(),
            schema_version: version,
            exported_at: Utc::now(),
            kv,
        })
    }

    pub fn import_bundle(&self, bundle: &OpsBundle) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        for row in &bundle.kv {
            tx.execute(
                "INSERT INTO kv (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
                params![row.key, row.value, row.updated_at],
            )?;
        }
        tx.commit()?;
        self.audit("bundle.import", &format!("keys={}", bundle.kv.len()))?;
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KvRow {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpsBundle {
    pub format: String,
    pub schema_version: i64,
    pub exported_at: DateTime<Utc>,
    pub kv: Vec<KvRow>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approvals::Snapshot;
    use chrono::TimeZone;

    #[test]
    fn snapshot_roundtrip_wal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ops.sqlite");
        let store = OpsStore::open(&path).unwrap();
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let snap = Snapshot::empty(now);
        store.save_snapshot(&snap).unwrap();
        drop(store);
        let store2 = OpsStore::open(&path).unwrap();
        let loaded = store2.load_snapshot().unwrap().unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.phase, "CLEAR");
    }

    #[test]
    fn bundle_export_import() {
        let a = OpsStore::open_in_memory().unwrap();
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        a.save_snapshot(&Snapshot::empty(now)).unwrap();
        let bundle = a.export_bundle().unwrap();
        assert_eq!(bundle.format, "peram-ops-bundle-v1");
        let b = OpsStore::open_in_memory().unwrap();
        b.import_bundle(&bundle).unwrap();
        assert!(b.load_snapshot().unwrap().is_some());
    }
}
