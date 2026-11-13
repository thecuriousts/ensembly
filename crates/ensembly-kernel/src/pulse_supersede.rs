//! Explicit supersession for pulse `memory_traces`.
//!
//! Freshness is a different signal from nearness. A later fact records who
//! replaced the earlier one: source, event time, reason, and writer. The
//! earlier row stays in the table as archive. [`list_live`] hides it.
//! [`read_history`] reads the chain. Checks in `pulse_core` stay read-only.
//!
//! The write is intent, then the pointer, then a read-back. The check and the
//! pointer share one immediate transaction, so two writers cannot close a
//! cycle. `admit --supersedes` validates every refusal before insert, then
//! commits the new row, the pointer, and the event together. A refusal rolls
//! that row back. A crash between the supersede commits resumes on the same
//! request. Raw rows are not deleted.
//!
//! Essay: <https://captain.kingsparrow.space/articles/memory-issue/archive-not-memory>

use crate::pulse_core::{self, TraceRow};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

const TRACE_COLS: &str =
    "nat_key, snippet, source, status, kind, lock, seen_count, superseded_by, first_seen, last_seen";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupersedePhase {
    Intent,
    Applied,
    Verified,
}

impl SupersedePhase {
    fn as_str(self) -> &'static str {
        match self {
            SupersedePhase::Intent => "intent",
            SupersedePhase::Applied => "applied",
            SupersedePhase::Verified => "verified",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "intent" => Some(SupersedePhase::Intent),
            "applied" => Some(SupersedePhase::Applied),
            "verified" => Some(SupersedePhase::Verified),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SupersedeFault {
    /// Stop before this phase's write. `Applied` leaves a committed intent
    /// and no pointer. `Verified` leaves the pointer with phase `applied`.
    pub crash_entering: Option<SupersedePhase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupersedeRequest {
    pub old_key: String,
    pub new_key: String,
    pub source: String,
    pub event_time: String,
    pub reason: String,
    pub writer: String,
    pub conflict_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SupersedeReceipt {
    pub old_key: String,
    pub new_key: String,
    pub phase: String,
    pub idempotent: bool,
    pub conflict_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmitRequest {
    pub key: String,
    pub snippet: String,
    pub source: String,
    pub time: String,
    pub status: String,
    pub kind: String,
    pub lock: String,
    pub supersedes: Option<String>,
    pub reason: Option<String>,
    pub writer: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdmitReceipt {
    pub nat_key: String,
    pub seen_count: i64,
    pub idempotent: bool,
    pub superseded: Option<SupersedeReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveRequest {
    pub conflict_id: i64,
    pub winner: String,
    pub source: String,
    pub event_time: String,
    pub reason: String,
    pub writer: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SupersessionEvent {
    pub old_key: String,
    pub new_key: String,
    pub source: String,
    pub event_time: String,
    pub reason: String,
    pub writer: String,
    pub phase: String,
    pub conflict_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct History {
    pub chain: Vec<TraceRow>,
    pub events: Vec<SupersessionEvent>,
}

#[derive(Debug, Error)]
pub enum SupersedeError {
    #[error("refused: {0}")]
    Refused(String),
    #[error("store: {0}")]
    Store(String),
}

struct StoredEvent {
    source: String,
    event_time: String,
    reason: String,
    writer: String,
    conflict_id: Option<i64>,
    phase: SupersedePhase,
}

struct Prepared {
    old_key: String,
    new_key: String,
    source: String,
    event_time: String,
    reason: String,
    writer: String,
    conflict_id: Option<i64>,
    old_snippet: String,
}

pub(crate) fn supersede(
    path: &Path,
    req: &SupersedeRequest,
    fault: &SupersedeFault,
) -> Result<SupersedeReceipt, SupersedeError> {
    let prepared = prepare_supersede(req)?;
    let mut conn = open_existing(path)?;
    let old = check_link(&conn, &prepared.old_key, &prepared.new_key, true)?;
    let prepared = Prepared {
        old_snippet: old.snippet,
        ..prepared
    };
    advance(&mut conn, &prepared, fault)
}

pub(crate) fn admit(
    path: &Path,
    req: &AdmitRequest,
    fault: &SupersedeFault,
) -> Result<AdmitReceipt, SupersedeError> {
    // Admit commits the row and the link together. A crash split would leave
    // the unlinked row this function refuses to keep.
    let _ = fault;
    let key = require_key(&req.key)?;
    let snippet = require_text("snippet", &req.snippet, 4000)?;
    let source = require_text("source", &req.source, 200)?;
    let time = require_time(&req.time)?;
    let status = require_enum(
        "status",
        &req.status,
        &["user-stated", "tool-verified", "inferred", "unknown"],
    )?;
    let kind = require_enum("kind", &req.kind, &["data", "context", "fact", "memory"])?;
    let lock = require_enum("lock", &req.lock, &["open", "locked"])?;
    let writer = require_writer(&req.writer)?;
    let supersedes = match &req.supersedes {
        Some(old) => Some(require_key(old)?),
        None => None,
    };
    if supersedes.as_deref() == Some(key.as_str()) {
        return Err(refused("self-supersession"));
    }
    let reason = match (&supersedes, &req.reason) {
        (Some(_), Some(reason)) => Some(require_text("reason", reason, 500)?),
        (Some(_), None) => return Err(refused("reason is required")),
        (None, _) => None,
    };

    let mut conn = open_existing(path)?;
    if let Some(old) = &supersedes {
        let _ = check_link(&conn, old, &key, false)?;
    }
    let existed = match load_trace_optional(&conn, &key)? {
        Some(row) => {
            if row.snippet != snippet {
                return Err(refused(snippet_refusal(&row.lock)));
            }
            true
        }
        None => false,
    };

    let link = in_tx(&mut conn, |tx| {
        if let Some(old) = &supersedes {
            let _ = check_link(tx, old, &key, false)?;
        }
        let existing = load_trace_optional(tx, &key)?;
        if let Some(row) = &existing {
            if row.snippet != snippet {
                return Err(refused(snippet_refusal(&row.lock)));
            }
        } else {
            insert_trace_tx(
                tx,
                &NewTrace {
                    key: &key,
                    snippet: &snippet,
                    source: &source,
                    time: &time,
                    status: &status,
                    kind: &kind,
                    lock: &lock,
                },
            )?;
        }
        let Some(old) = supersedes.clone() else {
            if existing.is_some() {
                bump_seen_tx(tx, &key, &time)?;
            }
            return Ok(None);
        };
        let old_row = load_trace(tx, &old)?;
        let prepared = Prepared {
            old_key: old,
            new_key: key.clone(),
            source: source.clone(),
            event_time: time.clone(),
            reason: reason.clone().unwrap_or_default(),
            writer: writer.clone(),
            conflict_id: None,
            old_snippet: old_row.snippet,
        };
        let idempotent = commit_link_tx(tx, &prepared)?;
        Ok(Some(receipt(
            &prepared,
            SupersedePhase::Verified,
            idempotent,
        )))
    })?;

    let seen_count = load_trace(&conn, &key)?.seen_count;
    let idempotent = existed && link.as_ref().is_some_and(|row| row.idempotent);
    Ok(AdmitReceipt {
        nat_key: key,
        seen_count,
        idempotent,
        superseded: link,
    })
}

pub(crate) fn resolve_conflict(
    path: &Path,
    req: &ResolveRequest,
    fault: &SupersedeFault,
) -> Result<SupersedeReceipt, SupersedeError> {
    let winner = require_key(&req.winner)?;
    let _ = require_writer(&req.writer)?;
    let conn = open_existing(path)?;
    let (left_id, right_id): (i64, i64) = conn
        .query_row(
            "SELECT left_id, right_id FROM conflicts WHERE id = ?1",
            [req.conflict_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(store)?
        .ok_or_else(|| refused(format!("unknown conflict {}", req.conflict_id)))?;
    let left = nat_key_for_id(&conn, left_id)?;
    let right = nat_key_for_id(&conn, right_id)?;
    let loser = if winner == left {
        right
    } else if winner == right {
        left
    } else {
        return Err(refused("winner is not a side of the conflict"));
    };
    drop(conn);
    supersede(
        path,
        &SupersedeRequest {
            old_key: loser,
            new_key: winner,
            source: req.source.clone(),
            event_time: req.event_time.clone(),
            reason: req.reason.clone(),
            writer: req.writer.clone(),
            conflict_id: Some(req.conflict_id),
        },
        fault,
    )
}

pub fn list_live(conn: &Connection, limit: i64) -> Result<Vec<TraceRow>, SupersedeError> {
    let limit = limit.max(0);
    let sql = format!(
        "SELECT {TRACE_COLS} FROM memory_traces \
         WHERE kind = 'memory' AND (superseded_by IS NULL OR superseded_by = '') \
         ORDER BY time DESC, nat_key ASC LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql).map_err(store)?;
    let rows = stmt
        .query_map([limit], trace_row)
        .map_err(store)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(store)?;
    Ok(rows)
}

pub fn read_history(conn: &Connection, key: &str) -> Result<History, SupersedeError> {
    let key = require_key(key)?;
    let start = load_trace(conn, &key)?;
    let mut forward = Vec::new();
    let mut cursor = nonempty(start.superseded_by.as_deref()).map(str::to_string);
    let mut guard = 0;
    while let Some(next) = cursor {
        if next == start.nat_key || forward.iter().any(|row: &TraceRow| row.nat_key == next) {
            break;
        }
        let Some(row) = load_trace_optional(conn, &next)? else {
            break;
        };
        cursor = nonempty(row.superseded_by.as_deref()).map(str::to_string);
        forward.push(row);
        guard += 1;
        if guard > 64 {
            return Err(refused("cycle"));
        }
    }

    let mut backward = Vec::new();
    let mut tip = start.nat_key.clone();
    guard = 0;
    loop {
        let pred = predecessor(conn, &tip)?;
        let Some(pred) = pred else { break };
        if pred.nat_key == start.nat_key
            || backward
                .iter()
                .any(|row: &TraceRow| row.nat_key == pred.nat_key)
        {
            break;
        }
        tip = pred.nat_key.clone();
        backward.push(pred);
        guard += 1;
        if guard > 64 {
            return Err(refused("cycle"));
        }
    }
    backward.reverse();
    let mut chain = backward;
    chain.push(start);
    chain.extend(forward);

    let keys: Vec<String> = chain.iter().map(|row| row.nat_key.clone()).collect();
    let events = if events_table(conn)? {
        load_events(conn, &keys)?
    } else {
        Vec::new()
    };
    Ok(History { chain, events })
}

fn prepare_supersede(req: &SupersedeRequest) -> Result<Prepared, SupersedeError> {
    let old_key = require_key(&req.old_key)?;
    let new_key = require_key(&req.new_key)?;
    if old_key == new_key {
        return Err(refused("self-supersession"));
    }
    Ok(Prepared {
        old_key,
        new_key,
        source: require_text("source", &req.source, 200)?,
        event_time: require_time(&req.event_time)?,
        reason: require_text("reason", &req.reason, 500)?,
        writer: require_writer(&req.writer)?,
        conflict_id: req.conflict_id,
        old_snippet: String::new(),
    })
}

/// Refuse a bad link before any pointer write. `new_must_exist` is false
/// while admit still has to insert the successor in the same transaction.
fn check_link(
    conn: &Connection,
    old_key: &str,
    new_key: &str,
    new_must_exist: bool,
) -> Result<TraceRow, SupersedeError> {
    if old_key == new_key {
        return Err(refused("self-supersession"));
    }
    let old = load_trace(conn, old_key)?;
    let new = load_trace_optional(conn, new_key)?;
    if new.is_none() && new_must_exist {
        return Err(refused(format!("unknown key: {new_key}")));
    }
    if let Some(existing) = nonempty(old.superseded_by.as_deref()) {
        if existing != new_key {
            return Err(refused(format!(
                "already superseded by {existing}; chain from that key"
            )));
        }
    } else if new.is_some() && reaches(conn, new_key, old_key)? {
        return Err(refused("cycle"));
    }
    Ok(old)
}

fn commit_link_tx(conn: &Connection, req: &Prepared) -> Result<bool, SupersedeError> {
    ensure_schema(conn)?;
    let existing = load_event(conn, &req.old_key, &req.new_key)?;
    if let Some(event) = &existing {
        if !same_provenance(event, req) {
            return Err(refused("provenance already recorded"));
        }
    }
    let phase = if let Some(event) = &existing {
        event.phase
    } else {
        insert_intent_tx(conn, req)?;
        SupersedePhase::Intent
    };
    let idempotent = phase == SupersedePhase::Verified;
    if phase == SupersedePhase::Intent {
        apply_pointer_tx(conn, req)?;
    }
    if phase != SupersedePhase::Verified {
        verify_pointer_tx(conn, req)?;
    }
    note_conflict_tx(conn, req)?;
    Ok(idempotent)
}

fn advance(
    conn: &mut Connection,
    req: &Prepared,
    fault: &SupersedeFault,
) -> Result<SupersedeReceipt, SupersedeError> {
    ensure_schema(conn)?;
    let existing = load_event(conn, &req.old_key, &req.new_key)?;
    if let Some(event) = &existing {
        if !same_provenance(event, req) {
            return Err(refused("provenance already recorded"));
        }
    }
    let mut phase = if let Some(event) = &existing {
        event.phase
    } else {
        if fault.crash_entering == Some(SupersedePhase::Intent) {
            return Ok(receipt(req, SupersedePhase::Intent, false));
        }
        insert_intent(conn, req)?;
        SupersedePhase::Intent
    };
    let resumed_verified = phase == SupersedePhase::Verified;
    if phase == SupersedePhase::Intent {
        if fault.crash_entering == Some(SupersedePhase::Applied) {
            return Ok(receipt(req, SupersedePhase::Intent, false));
        }
        apply_pointer(conn, req)?;
        phase = SupersedePhase::Applied;
    }
    if phase == SupersedePhase::Applied {
        if fault.crash_entering == Some(SupersedePhase::Verified) {
            return Ok(receipt(req, SupersedePhase::Applied, false));
        }
        verify_pointer(conn, req)?;
        phase = SupersedePhase::Verified;
    }
    if phase == SupersedePhase::Verified {
        note_conflict(conn, req)?;
        return Ok(receipt(req, SupersedePhase::Verified, resumed_verified));
    }
    Err(refused("supersession stopped short of verified"))
}

fn same_provenance(event: &StoredEvent, req: &Prepared) -> bool {
    event.source == req.source
        && event.event_time == req.event_time
        && event.reason == req.reason
        && event.writer == req.writer
        && match (event.conflict_id, req.conflict_id) {
            (Some(stored), Some(asked)) => stored == asked,
            _ => true,
        }
}

fn ensure_schema(conn: &Connection) -> Result<(), SupersedeError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS supersession_events (
            id INTEGER PRIMARY KEY,
            old_key TEXT NOT NULL,
            new_key TEXT NOT NULL,
            source TEXT NOT NULL,
            event_time TEXT NOT NULL,
            reason TEXT NOT NULL,
            writer TEXT NOT NULL,
            conflict_id INTEGER,
            phase TEXT NOT NULL CHECK (phase IN ('intent','applied','verified')),
            UNIQUE (old_key, new_key)
        );",
    )
    .map_err(store)
}

fn insert_intent(conn: &mut Connection, req: &Prepared) -> Result<(), SupersedeError> {
    in_tx(conn, |tx| insert_intent_tx(tx, req))
}

fn insert_intent_tx(conn: &Connection, req: &Prepared) -> Result<(), SupersedeError> {
    conn.execute(
        "INSERT INTO supersession_events (old_key, new_key, source, event_time, reason, writer, conflict_id, phase)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'intent')",
        params![
            req.old_key,
            req.new_key,
            req.source,
            req.event_time,
            req.reason,
            req.writer,
            req.conflict_id
        ],
    )
    .map_err(store)?;
    Ok(())
}

fn apply_pointer(conn: &mut Connection, req: &Prepared) -> Result<(), SupersedeError> {
    let result = in_tx(conn, |tx| apply_pointer_tx(tx, req));
    if matches!(result, Err(SupersedeError::Refused(_))) {
        let _ = drop_intent(conn, req);
    }
    result
}

fn apply_pointer_tx(conn: &Connection, req: &Prepared) -> Result<(), SupersedeError> {
    let old = check_link(conn, &req.old_key, &req.new_key, true)?;
    if old.snippet != req.old_snippet {
        return Err(refused(
            "verify failed: archive row or supersession pointer changed",
        ));
    }
    let changed = conn
        .execute(
            "UPDATE memory_traces SET superseded_by = ?1 \
             WHERE nat_key = ?2 AND (superseded_by IS NULL OR superseded_by = '' OR superseded_by = ?1)",
            params![req.new_key, req.old_key],
        )
        .map_err(store)?;
    if changed != 1 {
        let pointer: Option<String> = conn
            .query_row(
                "SELECT superseded_by FROM memory_traces WHERE nat_key = ?1",
                [&req.old_key],
                |row| row.get(0),
            )
            .map_err(store)?;
        if pointer.as_deref() != Some(req.new_key.as_str()) {
            return Err(refused(
                "already superseded; chain from that key".to_string(),
            ));
        }
    }
    conn.execute(
        "UPDATE supersession_events SET phase = 'applied' WHERE old_key = ?1 AND new_key = ?2",
        params![req.old_key, req.new_key],
    )
    .map_err(store)?;
    Ok(())
}

fn drop_intent(conn: &mut Connection, req: &Prepared) -> Result<(), SupersedeError> {
    if !events_table(conn)? {
        return Ok(());
    }
    in_tx(conn, |tx| {
        tx.execute(
            "DELETE FROM supersession_events \
             WHERE old_key = ?1 AND new_key = ?2 AND phase = 'intent'",
            params![req.old_key, req.new_key],
        )
        .map_err(store)?;
        Ok(())
    })
}

fn verify_pointer(conn: &mut Connection, req: &Prepared) -> Result<(), SupersedeError> {
    in_tx(conn, |tx| verify_pointer_tx(tx, req))
}

fn verify_pointer_tx(conn: &Connection, req: &Prepared) -> Result<(), SupersedeError> {
    let (snippet, pointer): (String, Option<String>) = conn
        .query_row(
            "SELECT snippet, superseded_by FROM memory_traces WHERE nat_key = ?1",
            [&req.old_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(store)?;
    if snippet != req.old_snippet || pointer.as_deref() != Some(req.new_key.as_str()) {
        return Err(refused(
            "verify failed: archive row or supersession pointer changed".to_string(),
        ));
    }
    let new_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memory_traces WHERE nat_key = ?1",
            [&req.new_key],
            |row| row.get(0),
        )
        .map_err(store)?;
    if new_exists != 1 {
        return Err(refused("verify failed: new key is missing".to_string()));
    }
    conn.execute(
        "UPDATE supersession_events SET phase = 'verified', conflict_id = COALESCE(?3, conflict_id) \
         WHERE old_key = ?1 AND new_key = ?2",
        params![req.old_key, req.new_key, req.conflict_id],
    )
    .map_err(store)?;
    Ok(())
}

fn note_conflict(conn: &mut Connection, req: &Prepared) -> Result<(), SupersedeError> {
    in_tx(conn, |tx| note_conflict_tx(tx, req))
}

fn note_conflict_tx(conn: &Connection, req: &Prepared) -> Result<(), SupersedeError> {
    let Some(conflict_id) = req.conflict_id else {
        return Ok(());
    };
    let resolution = format!("superseded by {}", req.new_key);
    let current: String = conn
        .query_row(
            "SELECT resolution FROM conflicts WHERE id = ?1",
            [conflict_id],
            |row| row.get(0),
        )
        .map_err(store)?;
    if current == resolution {
        return Ok(());
    }
    if !resolution_can_become_supersession(&current) {
        return Err(refused(format!(
            "conflict {conflict_id} is already resolved ({current})"
        )));
    }
    conn.execute(
        "UPDATE conflicts SET resolution = ?1 WHERE id = ?2",
        params![resolution, conflict_id],
    )
    .map_err(store)?;
    Ok(())
}

fn resolution_can_become_supersession(resolution: &str) -> bool {
    let trimmed = resolution.trim().to_lowercase();
    trimmed.is_empty()
        || matches!(
            trimmed.as_str(),
            "open" | "unresolved" | "pending" | "not in the evidence"
        )
}

fn load_event(
    conn: &Connection,
    old_key: &str,
    new_key: &str,
) -> Result<Option<StoredEvent>, SupersedeError> {
    if !events_table(conn)? {
        return Ok(None);
    }
    conn.query_row(
        "SELECT source, event_time, reason, writer, conflict_id, phase \
         FROM supersession_events WHERE old_key = ?1 AND new_key = ?2",
        params![old_key, new_key],
        |row| {
            let phase_raw: String = row.get(5)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<i64>>(4)?,
                phase_raw,
            ))
        },
    )
    .optional()
    .map_err(store)?
    .map(
        |(source, event_time, reason, writer, conflict_id, phase_raw)| {
            SupersedePhase::parse(&phase_raw)
                .map(|phase| StoredEvent {
                    source,
                    event_time,
                    reason,
                    writer,
                    conflict_id,
                    phase,
                })
                .ok_or_else(|| refused(format!("unknown supersession phase {phase_raw}")))
        },
    )
    .transpose()
}

fn load_events(
    conn: &Connection,
    keys: &[String],
) -> Result<Vec<SupersessionEvent>, SupersedeError> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT old_key, new_key, source, event_time, reason, writer, phase, conflict_id \
             FROM supersession_events ORDER BY id",
        )
        .map_err(store)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SupersessionEvent {
                old_key: row.get(0)?,
                new_key: row.get(1)?,
                source: row.get(2)?,
                event_time: row.get(3)?,
                reason: row.get(4)?,
                writer: row.get(5)?,
                phase: row.get(6)?,
                conflict_id: row.get(7)?,
            })
        })
        .map_err(store)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(store)?;
    Ok(rows
        .into_iter()
        .filter(|event| {
            keys.iter()
                .any(|key| key == &event.old_key || key == &event.new_key)
        })
        .collect())
}

fn events_table(conn: &Connection) -> Result<bool, SupersedeError> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'supersession_events'",
            [],
            |row| row.get(0),
        )
        .map_err(store)?;
    Ok(n == 1)
}

struct NewTrace<'a> {
    key: &'a str,
    snippet: &'a str,
    source: &'a str,
    time: &'a str,
    status: &'a str,
    kind: &'a str,
    lock: &'a str,
}

fn insert_trace_tx(conn: &Connection, row: &NewTrace<'_>) -> Result<(), SupersedeError> {
    conn.execute(
        "INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, lock, seen_count, first_seen, last_seen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?4, ?4)",
        params![row.key, row.snippet, row.source, row.time, row.status, row.kind, row.lock],
    )
    .map_err(store)?;
    let id = conn.last_insert_rowid();
    if fts_table(conn)? {
        conn.execute(
            "INSERT INTO memory_fts(rowid, nat_key, snippet, source) VALUES (?1, ?2, ?3, ?4)",
            params![id, row.key, row.snippet, row.source],
        )
        .map_err(store)?;
    }
    Ok(())
}

fn bump_seen_tx(conn: &Connection, key: &str, time: &str) -> Result<(), SupersedeError> {
    conn.execute(
        "UPDATE memory_traces SET seen_count = seen_count + 1, last_seen = ?1 WHERE nat_key = ?2",
        params![time, key],
    )
    .map_err(store)?;
    Ok(())
}

fn fts_table(conn: &Connection) -> Result<bool, SupersedeError> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'memory_fts'",
            [],
            |row| row.get(0),
        )
        .map_err(store)?;
    Ok(n == 1)
}

fn reaches(conn: &Connection, start: &str, target: &str) -> Result<bool, SupersedeError> {
    if start == target {
        return Ok(true);
    }
    let mut cursor = start.to_string();
    let mut seen = vec![cursor.clone()];
    for _ in 0..64 {
        let row = load_trace_optional(conn, &cursor)?;
        let Some(next) =
            row.and_then(|row| nonempty(row.superseded_by.as_deref()).map(str::to_string))
        else {
            return Ok(false);
        };
        if next == target {
            return Ok(true);
        }
        if seen.iter().any(|key| key == &next) {
            return Ok(true);
        }
        seen.push(next.clone());
        cursor = next;
    }
    Ok(true)
}

fn predecessor(conn: &Connection, key: &str) -> Result<Option<TraceRow>, SupersedeError> {
    let sql = format!(
        "SELECT {TRACE_COLS} FROM memory_traces WHERE superseded_by = ?1 ORDER BY nat_key ASC LIMIT 1"
    );
    conn.query_row(&sql, [key], trace_row)
        .optional()
        .map_err(store)
}

fn nat_key_for_id(conn: &Connection, id: i64) -> Result<String, SupersedeError> {
    conn.query_row(
        "SELECT nat_key FROM memory_traces WHERE id = ?1",
        [id],
        |row| row.get(0),
    )
    .optional()
    .map_err(store)?
    .ok_or_else(|| refused(format!("unknown key for row {id}")))
}

fn load_trace(conn: &Connection, key: &str) -> Result<TraceRow, SupersedeError> {
    load_trace_optional(conn, key)?.ok_or_else(|| refused(format!("unknown key: {key}")))
}

fn load_trace_optional(conn: &Connection, key: &str) -> Result<Option<TraceRow>, SupersedeError> {
    let sql = format!("SELECT {TRACE_COLS} FROM memory_traces WHERE nat_key = ?1");
    conn.query_row(&sql, [key], trace_row)
        .optional()
        .map_err(store)
}

fn trace_row(row: &rusqlite::Row) -> rusqlite::Result<TraceRow> {
    Ok(TraceRow {
        nat_key: row.get(0)?,
        snippet: row.get(1)?,
        source: row.get(2)?,
        status: row.get(3)?,
        kind: row.get(4)?,
        lock: row.get(5)?,
        seen_count: row.get(6)?,
        superseded_by: row.get(7)?,
        first_seen: row.get(8)?,
        last_seen: row.get(9)?,
    })
}

fn in_tx<T>(
    conn: &mut Connection,
    body: impl FnOnce(&rusqlite::Transaction) -> Result<T, SupersedeError>,
) -> Result<T, SupersedeError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store)?;
    let value = body(&tx)?;
    tx.commit().map_err(store)?;
    Ok(value)
}

fn open_existing(path: &Path) -> Result<Connection, SupersedeError> {
    if !path.is_file() {
        return Err(SupersedeError::Store("pulse db missing".into()));
    }
    let conn = Connection::open(path).map_err(store)?;
    conn.busy_timeout(Duration::from_secs(5)).map_err(store)?;
    // Accident guard only. A local process can still register the SQL
    // function or drop the trigger; crate users must go through KernelWriter.
    crate::write_guard::install_triggers(&conn).map_err(store)?;
    crate::write_guard::grant_session(&conn).map_err(store)?;
    Ok(conn)
}

fn require_writer(writer: &str) -> Result<String, SupersedeError> {
    ensembly_schema::require_configured_writer(writer).map_err(refused)
}

fn require_key(key: &str) -> Result<String, SupersedeError> {
    let key = require_text("nat_key", key, 200)?;
    Ok(key)
}

fn require_text(label: &str, raw: &str, max: usize) -> Result<String, SupersedeError> {
    let text = raw.trim();
    if text.is_empty() || text.chars().count() > max {
        return Err(refused(format!("{label} is required")));
    }
    if text.contains('@') || text.contains("/home/") || text.contains("/Users/") {
        return Err(refused(format!("{label} is not public")));
    }
    Ok(text.to_string())
}

fn require_time(raw: &str) -> Result<String, SupersedeError> {
    let text = raw.trim();
    if pulse_core::parse_ts(text).is_none() {
        return Err(refused(format!("event time cannot be parsed: {text}")));
    }
    Ok(text.to_string())
}

fn require_enum(label: &str, raw: &str, allowed: &[&str]) -> Result<String, SupersedeError> {
    let text = raw.trim();
    if allowed.contains(&text) {
        Ok(text.to_string())
    } else {
        Err(refused(format!("{label} is not allowed")))
    }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(text)
        }
    })
}

fn receipt(req: &Prepared, phase: SupersedePhase, idempotent: bool) -> SupersedeReceipt {
    SupersedeReceipt {
        old_key: req.old_key.clone(),
        new_key: req.new_key.clone(),
        phase: phase.as_str().to_string(),
        idempotent,
        conflict_id: req.conflict_id,
    }
}

fn snippet_refusal(lock: &str) -> String {
    let which = if lock == "open" {
        "snippet changed"
    } else {
        "locked snippet changed"
    };
    format!("{which}; pass --supersedes to archive the old fact")
}

fn refused(message: impl Into<String>) -> SupersedeError {
    SupersedeError::Refused(message.into())
}

fn store(err: impl ToString) -> SupersedeError {
    SupersedeError::Store(err.to_string())
}
