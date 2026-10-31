//! Pulse core, first slice: read-only health check and read over the live
//! pulse-memory SQLite store, plus a one-way import of the mission table.
//!
//! Law: this module never writes to the pulse DB, to premflow data, or to any
//! mission goal graph (G) source. The only writes are the new mission store
//! under `missions_dir`, created by [`import_missions`].

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const STALE_WARN_DAYS: i64 = 7;
pub const PREMFLOW_TIMEOUT_SECS: u64 = 5;
pub const MISSION_STORE_FORMAT: &str = "ensembly-missions-v1";
pub const MISSIONS_FILE: &str = "missions.json";
const DIR_HASH_MAX_FILES: usize = 2000;
const FILE_HASH_MAX_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Ok,
    Note,
    Warn,
    Fail,
}

/// A write the pulse core refuses to make; the CLI maps it to exit code 3.
#[derive(Debug)]
pub struct RefusedWrite(pub String);

/// CLI exit code for a refused write (`missions import` without `--force`).
/// 7, not 3: `apps audit status` uses 3 for flag (callers depend on it), and
/// 4/5/6 are audit or reserved; 7 means nothing else in the CLI.
pub use ensembly_schema::EXIT_PULSE_REFUSED_WRITE as REFUSED_WRITE_EXIT;

impl std::fmt::Display for RefusedWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "refused write: {}", self.0)
    }
}

impl std::error::Error for RefusedWrite {}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct PulseCfg {
    pub db: PathBuf,
    pub premflow_bin: PathBuf,
    /// HOME handed to premflow; its data lives in `<home>/.premflow`.
    pub premflow_home: PathBuf,
    pub premflow_timeout: Duration,
    pub missions_dir: PathBuf,
    /// Folder holding `pulse-YYYY-MM-DD.md` notes (optional freshness signal).
    pub notes_dir: Option<PathBuf>,
    /// Roots tried, in order, to resolve relative G source paths.
    pub g_roots: Vec<PathBuf>,
    pub now: DateTime<Utc>,
    pub stale_warn_days: i64,
}

impl PulseCfg {
    pub fn from_env(now: DateTime<Utc>) -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"));
        let env_path = |k: &str, d: &str| {
            std::env::var_os(k)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(d))
        };
        let g_roots = match std::env::var("ENSEMBLY_PULSE_G_ROOTS") {
            Ok(v) if !v.is_empty() => v.split(':').map(PathBuf::from).collect(),
            _ => vec![
                PathBuf::from("/workspace/ops"),
                PathBuf::from("/workspace"),
                PathBuf::from("/workspace/dev/products/ensembly"),
                PathBuf::from("/workspace/ops/ensembly-pulse"),
            ],
        };
        Self {
            db: data.join("pulse-memory/pulse.sqlite"),
            premflow_bin: find_in_path("premflow")
                .unwrap_or_else(|| home.join(".local/bin/premflow")),
            premflow_home: home,
            premflow_timeout: Duration::from_secs(PREMFLOW_TIMEOUT_SECS),
            missions_dir: env_path(
                "ENSEMBLY_PULSE_MISSIONS",
                "/workspace/ops/ensembly-pulse/missions",
            ),
            notes_dir: Some(env_path(
                "ENSEMBLY_PULSE_NOTES",
                "/workspace/ops/ensembly-pulse",
            )),
            g_roots,
            now,
            stale_warn_days: STALE_WARN_DAYS,
        }
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// Parse an RFC 3339 timestamp, a `YYYY-MM-DD HH:MM` stamp, or a bare date.
pub fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(d.with_timezone(&Utc));
    }
    if let Ok(n) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        return Some(n.and_utc());
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|n| n.and_utc())
}

// ---------------------------------------------------------------- read-only DB

/// Open the pulse DB read-only. `query_only` is a second belt: any write errors.
pub fn open_ro(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("open read-only {}", path.display()))?;
    conn.execute_batch("PRAGMA query_only = ON;")?;
    Ok(conn)
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DbStats {
    pub integrity: String,
    pub schema_version: Option<i64>,
    pub traces: i64,
    pub newest_last_seen: Option<String>,
    pub by_status: BTreeMap<String, i64>,
    pub by_kind: BTreeMap<String, i64>,
    pub locked: i64,
    pub unresolved_conflicts: i64,
    pub dangling_superseded: Vec<String>,
}

pub fn db_stats(conn: &Connection) -> Result<DbStats> {
    let mut s = DbStats {
        integrity: conn.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))?,
        ..Default::default()
    };
    s.schema_version = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
            r.get(0)
        })
        .ok()
        .flatten();
    s.traces = conn.query_row("SELECT COUNT(*) FROM memory_traces", [], |r| r.get(0))?;
    s.newest_last_seen =
        conn.query_row("SELECT MAX(last_seen) FROM memory_traces", [], |r| r.get(0))?;
    for (col, map) in [("status", &mut s.by_status), ("kind", &mut s.by_kind)] {
        let mut st = conn.prepare(&format!(
            "SELECT {col}, COUNT(*) FROM memory_traces GROUP BY 1"
        ))?;
        for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            let (k, n) = row?;
            map.insert(k, n);
        }
    }
    s.locked = conn.query_row(
        "SELECT COUNT(*) FROM memory_traces WHERE lock='locked'",
        [],
        |r| r.get(0),
    )?;
    s.unresolved_conflicts = conn.query_row(
        "SELECT COUNT(*) FROM conflicts WHERE LOWER(TRIM(resolution)) IN ('', 'open', 'unresolved', 'pending')",
        [],
        |r| r.get(0),
    )?;
    let mut st = conn.prepare(
        "SELECT nat_key FROM memory_traces t WHERE superseded_by IS NOT NULL AND superseded_by <> '' \
         AND NOT EXISTS (SELECT 1 FROM memory_traces u WHERE u.nat_key = t.superseded_by) ORDER BY nat_key",
    )?;
    s.dangling_superseded = st
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(s)
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceRow {
    pub nat_key: String,
    pub snippet: String,
    pub source: String,
    pub status: String,
    pub kind: String,
    pub lock: String,
    pub seen_count: i64,
    pub superseded_by: Option<String>,
    pub first_seen: String,
    pub last_seen: String,
}

const TRACE_COLS: &str =
    "nat_key, snippet, source, status, kind, lock, seen_count, superseded_by, first_seen, last_seen";

fn trace_row(r: &rusqlite::Row) -> rusqlite::Result<TraceRow> {
    Ok(TraceRow {
        nat_key: r.get(0)?,
        snippet: r.get(1)?,
        source: r.get(2)?,
        status: r.get(3)?,
        kind: r.get(4)?,
        lock: r.get(5)?,
        seen_count: r.get(6)?,
        superseded_by: r.get(7)?,
        first_seen: r.get(8)?,
        last_seen: r.get(9)?,
    })
}

/// Traces whose `nat_key` starts with `prefix` (literal match, no wildcards).
pub fn read_prefix(conn: &Connection, prefix: &str, limit: usize) -> Result<Vec<TraceRow>> {
    let sql = format!(
        "SELECT {TRACE_COLS} FROM memory_traces WHERE substr(nat_key, 1, length(?1)) = ?1 \
         ORDER BY nat_key LIMIT ?2"
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st
        .query_map(params![prefix, limit as i64], trace_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Full-text search (FTS5, every word must match), falling back to a literal
/// substring match when the FTS query is rejected or the index is missing.
pub fn read_search(conn: &Connection, text: &str, limit: usize) -> Result<Vec<TraceRow>> {
    let fts_q: String = text
        .split_whitespace()
        .map(|w| format!("\"{}\"", w.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ");
    if !fts_q.is_empty() {
        let sql = format!(
            "SELECT {} FROM memory_traces t JOIN memory_fts f ON f.rowid = t.id \
             WHERE memory_fts MATCH ?1 ORDER BY t.last_seen DESC LIMIT ?2",
            TRACE_COLS
                .split(", ")
                .map(|c| format!("t.{c}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let attempt = (|| -> rusqlite::Result<Vec<TraceRow>> {
            let mut st = conn.prepare(&sql)?;
            let v = st
                .query_map(params![fts_q, limit as i64], trace_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(v)
        })();
        if let Ok(v) = attempt {
            return Ok(v);
        }
    }
    let like = format!(
        "%{}%",
        text.replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let sql = format!(
        "SELECT {TRACE_COLS} FROM memory_traces WHERE nat_key LIKE ?1 ESCAPE '\\' OR snippet LIKE ?1 ESCAPE '\\' \
         ORDER BY last_seen DESC LIMIT ?2"
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st
        .query_map(params![like, limit as i64], trace_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

// -------------------------------------------------------------------- premflow

#[derive(Debug, Clone, Serialize, Default)]
pub struct PremflowHealth {
    /// ok | missing | timeout | nonzero | error
    pub stats: String,
    pub exit_code: Option<i32>,
    pub log_entries: usize,
    pub log_tags: BTreeMap<String, usize>,
    pub log_last_entry: Option<String>,
    pub log_readable: bool,
}

/// Run `premflow stats` with `HOME` set and a hard timeout. Never panics; every
/// failure mode is a status string, not an error.
pub fn run_premflow_stats(bin: &Path, home: &Path, timeout: Duration) -> (String, Option<i32>) {
    if !bin.is_file() {
        return ("missing".into(), None);
    }
    let mut child = match Command::new(bin)
        .arg("stats")
        .env("HOME", home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return ("error".into(), None),
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Drain so a chatty child can't be mistaken for a hang.
                if let Some(mut o) = child.stdout.take() {
                    let mut b = Vec::new();
                    let _ = o.read_to_end(&mut b);
                }
                let code = status.code();
                return (if status.success() { "ok" } else { "nonzero" }.into(), code);
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return ("timeout".into(), None);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return ("error".into(), None),
        }
    }
}

/// Parse `[YYYY-MM-DD HH:MM] [TAG] text` lines. Unparseable lines are skipped.
pub fn read_premflow_log(home: &Path) -> PremflowHealth {
    let mut h = PremflowHealth::default();
    let path = home.join(".premflow/log.txt");
    let Ok(text) = fs::read_to_string(&path) else {
        return h;
    };
    h.log_readable = true;
    let mut last: Option<DateTime<Utc>> = None;
    for line in text.lines() {
        let Some(rest) = line.strip_prefix('[') else {
            continue;
        };
        let Some((stamp, rest)) = rest.split_once("] [") else {
            continue;
        };
        let Some((tag, _)) = rest.split_once(']') else {
            continue;
        };
        let Some(ts) = parse_ts(stamp) else { continue };
        h.log_entries += 1;
        *h.log_tags.entry(tag.to_string()).or_default() += 1;
        if last.is_none_or(|l| ts > l) {
            last = Some(ts);
            h.log_last_entry = Some(stamp.to_string());
        }
    }
    h
}

// ---------------------------------------------------------------- mission store

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GFile {
    /// Path as written in the spec (backticked).
    pub spec_path: String,
    /// Absolute path it resolved to at import time, if any.
    pub resolved: Option<String>,
    /// sha256 of the file, or of the sorted directory listing (`d:` prefix).
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mission {
    pub id: String,
    pub name: String,
    pub owner: String,
    /// False when the source marked the owner as a guess (decision D7).
    pub owner_confirmed: bool,
    /// long | short | event
    pub horizon: String,
    /// active unless the horizon cell carried a qualifier such as "(Parked)".
    pub status: String,
    pub g_source: String,
    pub g_files: Vec<GFile>,
    pub links: String,
    pub review_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionStore {
    pub format: String,
    pub imported_at: String,
    pub source_path: String,
    pub source_sha256: String,
    pub missions: Vec<Mission>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn slug(s: &str) -> String {
    let base = s.split(" (").next().unwrap_or(s);
    let mut out = String::new();
    for c in base.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-')
        .chars()
        .take(40)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

/// G source paths named in a table cell: backticked paths first, then bare
/// tokens that look like files (`STANDING.md`, `care/open-loops.md`).
fn g_paths_in(cell: &str) -> Vec<String> {
    let mut out: Vec<String> = cell
        .split('`')
        .skip(1)
        .step_by(2)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let bare: String = cell.split('`').step_by(2).collect::<Vec<_>>().join(" ");
    for tok in bare.split_whitespace() {
        let t = tok.trim_matches(|c: char| ",.;:()\"'".contains(c));
        let is_file = [".md", ".jsonl", ".json"].iter().any(|e| t.ends_with(e));
        if is_file && !t.contains("://") && !out.iter().any(|o| o == t) {
            out.push(t.to_string());
        }
    }
    out
}

/// Parse the mission table (header starts `| Mission |`) out of spec markdown.
pub fn parse_mission_table(md: &str) -> Result<Vec<Mission>> {
    let mut lines = md
        .lines()
        .skip_while(|l| !l.trim_start().starts_with("| Mission |"));
    lines
        .next()
        .ok_or_else(|| anyhow!("no `| Mission |` table header found"))?;
    let mut out = Vec::new();
    for line in lines {
        let t = line.trim();
        if !t.starts_with('|') {
            break;
        }
        if t.trim_matches(|c| c == '|' || c == '-' || c == ' ' || c == ':')
            .is_empty()
        {
            continue;
        }
        let cells: Vec<&str> = t.trim_matches('|').split('|').map(|c| c.trim()).collect();
        if cells.len() < 5 {
            bail!("mission row has {} cells, need 5: {t}", cells.len());
        }
        let guess = cells[1].to_lowercase().contains("(guess)");
        let owner = cells[1]
            .replace("(guess)", "")
            .replace("  ", " ")
            .trim()
            .to_string();
        let hz = cells[2].to_lowercase();
        let horizon = hz.split_whitespace().next().unwrap_or("").to_string();
        if !["long", "short", "event"].contains(&horizon.as_str()) {
            bail!("unknown horizon `{}` for mission `{}`", cells[2], cells[0]);
        }
        let status = match (hz.find('('), hz.find(')')) {
            (Some(a), Some(b)) if b > a => hz[a + 1..b].trim().to_string(),
            _ => "active".to_string(),
        };
        let g_files = g_paths_in(cells[3])
            .into_iter()
            .map(|p| GFile {
                spec_path: p,
                resolved: None,
                sha256: None,
            })
            .collect();
        out.push(Mission {
            id: slug(cells[0]),
            name: cells[0].to_string(),
            owner,
            owner_confirmed: !guess,
            horizon,
            status: status.to_lowercase(),
            g_source: cells[3].to_string(),
            g_files,
            links: cells[4].to_string(),
            review_date: None,
        });
    }
    if out.is_empty() {
        bail!("mission table has no rows");
    }
    let mut seen = std::collections::BTreeSet::new();
    for m in &out {
        if !seen.insert(m.id.clone()) {
            bail!("duplicate mission id `{}`", m.id);
        }
    }
    Ok(out)
}

/// Hash a G source without following symlinks. Files: sha256 of the bytes.
/// Directories: sha256 over sorted `relpath\0filehash\n` lines (capped).
pub fn hash_g_path(p: &Path) -> Option<String> {
    let md = fs::symlink_metadata(p).ok()?;
    if md.file_type().is_symlink() {
        return None;
    }
    if md.is_file() {
        return hash_file(p).map(|h| format!("f:{h}"));
    }
    if md.is_dir() {
        let mut entries = Vec::new();
        collect_files(p, p, &mut entries);
        entries.sort();
        let mut hasher = Sha256::new();
        for rel in entries.iter().take(DIR_HASH_MAX_FILES) {
            let h = hash_file(&p.join(rel)).unwrap_or_else(|| "unreadable".into());
            hasher.update(rel.as_bytes());
            hasher.update([0]);
            hasher.update(h.as_bytes());
            hasher.update(b"\n");
        }
        return Some(format!("d:{}", hex::encode(hasher.finalize())));
    }
    None
}

fn hash_file(p: &Path) -> Option<String> {
    let md = fs::metadata(p).ok()?;
    if md.len() > FILE_HASH_MAX_BYTES {
        return Some(format!("size:{}", md.len()));
    }
    let mut f = fs::File::open(p).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hex::encode(hasher.finalize()))
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if out.len() >= DIR_HASH_MAX_FILES * 2 {
            return;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let path = e.path();
        if ft.is_dir() {
            collect_files(root, &path, out);
        } else if ft.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().into_owned());
            }
        }
    }
}

fn resolve_g(spec_path: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    let p = Path::new(spec_path);
    if p.is_absolute() {
        return p.exists().then(|| p.to_path_buf());
    }
    if spec_path.contains("..") {
        return None;
    }
    roots.iter().map(|r| r.join(p)).find(|c| c.exists())
}

pub fn load_store(missions_dir: &Path) -> Result<Option<MissionStore>> {
    let p = missions_dir.join(MISSIONS_FILE);
    if !p.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
    Ok(Some(
        serde_json::from_str(&text).with_context(|| format!("parse {}", p.display()))?,
    ))
}

/// Import the mission table from `spec` into a NEW store under
/// `cfg.missions_dir`. Refuses to overwrite an existing store unless `force`
/// (then the old file is kept as `missions.json.bak-<utc>`). The spec and every
/// G source are only read.
pub fn import_missions(cfg: &PulseCfg, spec: &Path, force: bool) -> Result<MissionStore> {
    let bytes = fs::read(spec).with_context(|| format!("read spec {}", spec.display()))?;
    let md = String::from_utf8(bytes.clone()).context("spec is not UTF-8")?;
    let mut missions = parse_mission_table(&md)?;
    for m in &mut missions {
        for g in &mut m.g_files {
            if let Some(p) = resolve_g(&g.spec_path, &cfg.g_roots) {
                g.sha256 = hash_g_path(&p);
                g.resolved = Some(p.to_string_lossy().into_owned());
            }
        }
    }
    let store = MissionStore {
        format: MISSION_STORE_FORMAT.into(),
        imported_at: cfg.now.to_rfc3339(),
        source_path: spec.to_string_lossy().into_owned(),
        source_sha256: sha256_hex(&bytes),
        missions,
    };
    fs::create_dir_all(&cfg.missions_dir)?;
    let target = cfg.missions_dir.join(MISSIONS_FILE);
    if target.exists() {
        if !force {
            return Err(RefusedWrite(format!(
                "{} exists; use --force to replace (old copy is kept)",
                target.display()
            ))
            .into());
        }
        let bak = cfg.missions_dir.join(format!(
            "{MISSIONS_FILE}.bak-{}",
            cfg.now.format("%Y%m%dT%H%M%SZ")
        ));
        fs::copy(&target, &bak).context("back up existing store")?;
    }
    let tmp = cfg.missions_dir.join(format!(".{MISSIONS_FILE}.tmp"));
    fs::write(&tmp, serde_json::to_string_pretty(&store)? + "\n")?;
    fs::rename(&tmp, &target)?;
    Ok(store)
}

fn g_snapshot(store: &MissionStore, roots: &[PathBuf]) -> BTreeMap<String, Option<String>> {
    let mut m = BTreeMap::new();
    for ms in &store.missions {
        for g in &ms.g_files {
            if let Some(res) = &g.resolved {
                let p = PathBuf::from(res);
                let p = if p.exists() {
                    p
                } else {
                    resolve_g(&g.spec_path, roots).unwrap_or(p)
                };
                m.insert(format!("{}:{}", ms.id, g.spec_path), hash_g_path(&p));
            }
        }
    }
    m
}

// ----------------------------------------------------------------------- check

#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    pub verdict: String,
    pub exit_code: i32,
    pub checked_at: String,
    pub db: Option<DbStats>,
    pub days_stale: Option<i64>,
    pub newest_note: Option<String>,
    pub premflow: PremflowHealth,
    pub missions: usize,
    pub g_sources_hashed: usize,
    pub findings: Vec<Finding>,
}

fn days_between(now: DateTime<Utc>, then: DateTime<Utc>) -> i64 {
    (now - then).num_days()
}

pub fn newest_note_date(dir: &Path) -> Option<NaiveDate> {
    let mut best: Option<NaiveDate> = None;
    for e in fs::read_dir(dir).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(d) = name
            .strip_prefix("pulse-")
            .and_then(|r| r.strip_suffix(".md"))
        {
            if let Ok(d) = NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                if best.is_none_or(|b| d > b) {
                    best = Some(d);
                }
            }
        }
    }
    best
}

struct Findings(Vec<Finding>);

impl Findings {
    fn add(&mut self, id: &str, severity: Severity, message: String) {
        self.0.push(Finding {
            id: id.into(),
            severity,
            message,
        });
    }
}

/// Staleness is a nudge (decision D4): warn from `stale_warn_days`, never fail.
fn check_stale(cfg: &PulseCfg, f: &mut Findings, id: &str, what: &str, days: i64) {
    if days >= cfg.stale_warn_days {
        f.add(id, Severity::Warn, format!("{what} is {days} days old"));
    }
}

fn check_db_stats(cfg: &PulseCfg, f: &mut Findings, s: &DbStats) -> Option<i64> {
    if s.integrity != "ok" {
        f.add(
            "db.integrity",
            Severity::Fail,
            format!("quick_check says: {}", s.integrity),
        );
    }
    if s.unresolved_conflicts > 0 {
        f.add(
            "db.conflicts",
            Severity::Fail,
            format!("{} unresolved conflict row(s)", s.unresolved_conflicts),
        );
    }
    if !s.dangling_superseded.is_empty() {
        f.add(
            "db.superseded",
            Severity::Warn,
            format!(
                "superseded_by points nowhere for: {}",
                s.dangling_superseded.join(", ")
            ),
        );
    }
    let Some(t) = s.newest_last_seen.as_deref().and_then(parse_ts) else {
        f.add(
            "db.stale",
            Severity::Warn,
            "no parseable last_seen in any trace".into(),
        );
        return None;
    };
    let d = days_between(cfg.now, t);
    check_stale(cfg, f, "db.stale", "newest trace", d);
    Some(d)
}

fn check_db(cfg: &PulseCfg, f: &mut Findings) -> (Option<DbStats>, Option<i64>) {
    match open_ro(&cfg.db).and_then(|c| db_stats(&c)) {
        Err(e) => {
            f.add("db", Severity::Fail, format!("pulse DB unreadable: {e:#}"));
            (None, None)
        }
        Ok(s) => {
            let days = check_db_stats(cfg, f, &s);
            (Some(s), days)
        }
    }
}

fn check_notes(cfg: &PulseCfg, f: &mut Findings) -> Option<String> {
    let d = newest_note_date(cfg.notes_dir.as_deref()?)?;
    let age = days_between(cfg.now, d.and_hms_opt(0, 0, 0)?.and_utc());
    check_stale(cfg, f, "notes.stale", "newest pulse note", age);
    Some(d.to_string())
}

fn check_premflow(cfg: &PulseCfg, f: &mut Findings) -> PremflowHealth {
    let (status, code) =
        run_premflow_stats(&cfg.premflow_bin, &cfg.premflow_home, cfg.premflow_timeout);
    let mut pf = read_premflow_log(&cfg.premflow_home);
    pf.stats = status;
    pf.exit_code = code;
    if pf.stats != "ok" {
        f.add(
            "premflow.stats",
            Severity::Warn,
            format!("`premflow stats` is {} (exit {:?})", pf.stats, pf.exit_code),
        );
    }
    if !pf.log_readable {
        f.add(
            "premflow.log",
            Severity::Warn,
            "premflow log.txt missing or unreadable".into(),
        );
    } else if let Some(t) = pf.log_last_entry.as_deref().and_then(parse_ts) {
        let d = days_between(cfg.now, t);
        if d >= cfg.stale_warn_days {
            f.add(
                "premflow.log",
                Severity::Note,
                format!("last premflow log entry is {d} days old"),
            );
        }
    }
    pf
}

fn check_mission(cfg: &PulseCfg, f: &mut Findings, m: &Mission) {
    if m.owner.trim().is_empty() {
        f.add(
            "missions.owner",
            Severity::Warn,
            format!("mission `{}` has no owner", m.id),
        );
    }
    let past = m
        .review_date
        .as_deref()
        .and_then(parse_ts)
        .is_some_and(|rd| rd < cfg.now);
    if past {
        f.add(
            "missions.review",
            Severity::Warn,
            format!(
                "mission `{}` is past its review date {}",
                m.id,
                m.review_date.as_deref().unwrap_or("")
            ),
        );
    }
}

fn check_missions(cfg: &PulseCfg, f: &mut Findings, s: &MissionStore) {
    let unconfirmed: Vec<&str> = s
        .missions
        .iter()
        .filter(|m| !m.owner_confirmed)
        .map(|m| m.id.as_str())
        .collect();
    if !unconfirmed.is_empty() {
        f.add(
            "missions.owner",
            Severity::Note,
            format!(
                "{} mission owner(s) are guesses and unconfirmed: {}",
                unconfirmed.len(),
                unconfirmed.join(", ")
            ),
        );
    }
    for m in &s.missions {
        check_mission(cfg, f, m);
    }
    let unresolved = s
        .missions
        .iter()
        .flat_map(|m| &m.g_files)
        .filter(|g| g.resolved.is_none())
        .count();
    if unresolved > 0 {
        f.add(
            "missions.g_source",
            Severity::Note,
            format!("{unresolved} G source path(s) did not resolve at import"),
        );
    }
}

/// Keys whose hash differs between two snapshots.
fn changed_keys(
    a: &BTreeMap<String, Option<String>>,
    b: &BTreeMap<String, Option<String>>,
) -> Vec<String> {
    a.iter()
        .filter(|(k, v)| b.get(*k) != Some(*v))
        .map(|(k, _)| k.clone())
        .collect()
}

/// G sources that differ from the hash taken at import time.
fn drift_from_baseline(s: &MissionStore, now: &BTreeMap<String, Option<String>>) -> Vec<String> {
    let mut drift = Vec::new();
    for m in &s.missions {
        for g in &m.g_files {
            let key = format!("{}:{}", m.id, g.spec_path);
            if now.get(&key).is_some_and(|h| h != &g.sha256) {
                drift.push(key);
            }
        }
    }
    drift
}

fn check_g(
    cfg: &PulseCfg,
    f: &mut Findings,
    s: &MissionStore,
    before: &BTreeMap<String, Option<String>>,
) {
    let after = g_snapshot(s, &cfg.g_roots);
    let changed = changed_keys(before, &after);
    if !changed.is_empty() {
        f.add(
            "g.unchanged",
            Severity::Fail,
            format!("G source changed while the check ran (pulse never writes G): {changed:?}"),
        );
    }
    let drift = drift_from_baseline(s, before);
    if !drift.is_empty() {
        f.add(
            "g.drift",
            Severity::Note,
            format!(
                "{} G source(s) differ from the import baseline (owners may edit their own G): {}",
                drift.len(),
                drift.join(", ")
            ),
        );
    }
}

fn verdict_of(f: &[Finding]) -> (&'static str, i32) {
    match f.iter().map(|x| x.severity).max().unwrap_or(Severity::Ok) {
        Severity::Fail => ("fail", 2),
        Severity::Warn => ("warn", 1),
        _ => ("ok", 0),
    }
}

pub fn run_check(cfg: &PulseCfg) -> CheckReport {
    let mut f = Findings(Vec::new());
    // G sources are hashed before and after everything else: pulse is read-only on G.
    let store = match load_store(&cfg.missions_dir) {
        Ok(s) => s,
        Err(e) => {
            f.add(
                "missions",
                Severity::Fail,
                format!("mission store unreadable: {e:#}"),
            );
            None
        }
    };
    let g_before = store.as_ref().map(|s| g_snapshot(s, &cfg.g_roots));
    let (db, days_stale) = check_db(cfg, &mut f);
    let newest_note = check_notes(cfg, &mut f);
    let premflow = check_premflow(cfg, &mut f);
    match (&store, &g_before) {
        (Some(s), Some(before)) => {
            check_missions(cfg, &mut f, s);
            check_g(cfg, &mut f, s, before);
        }
        _ if f.0.iter().all(|x| x.id != "missions") => f.add(
            "missions",
            Severity::Note,
            "no mission store yet; run `ensembly pulse missions import --spec <file>`".into(),
        ),
        _ => {}
    }
    let (verdict, exit_code) = verdict_of(&f.0);
    CheckReport {
        verdict: verdict.into(),
        exit_code,
        checked_at: cfg.now.to_rfc3339(),
        db,
        days_stale,
        newest_note,
        premflow,
        missions: store.as_ref().map_or(0, |s| s.missions.len()),
        g_sources_hashed: g_before.as_ref().map_or(0, |b| b.len()),
        findings: f.0,
    }
}

pub fn render_check(r: &CheckReport) -> String {
    let mut s = format!(
        "pulse check: {} (exit {})  {}\n",
        r.verdict.to_uppercase(),
        r.exit_code,
        r.checked_at
    );
    if let Some(d) = &r.db {
        let by: Vec<String> = d
            .by_status
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect();
        s += &format!(
            "  traces: {} ({}); newest {}{}; conflicts open {}; schema v{}; integrity {}\n",
            d.traces,
            by.join(", "),
            d.newest_last_seen.as_deref().unwrap_or("none"),
            r.days_stale
                .map(|n| format!(" = {n} d ago"))
                .unwrap_or_default(),
            d.unresolved_conflicts,
            d.schema_version
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".into()),
            d.integrity
        );
    }
    if let Some(n) = &r.newest_note {
        s += &format!("  newest pulse note: {n}\n");
    }
    let tags: Vec<String> = r
        .premflow
        .log_tags
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    s += &format!(
        "  premflow: stats {}; log {} entries ({}), last {}\n",
        r.premflow.stats,
        r.premflow.log_entries,
        tags.join(", "),
        r.premflow.log_last_entry.as_deref().unwrap_or("none")
    );
    s += &format!(
        "  missions: {}; G sources hashed: {}\n",
        r.missions, r.g_sources_hashed
    );
    for x in &r.findings {
        s += &format!("  [{:?}] {}: {}\n", x.severity, x.id, x.message);
    }
    s
}

pub fn render_traces(rows: &[TraceRow]) -> String {
    let mut s = String::new();
    for r in rows {
        s += &format!(
            "{}  [{} / {} / {}] seen {}x, last {}{}\n  {}\n",
            r.nat_key,
            r.status,
            r.kind,
            r.lock,
            r.seen_count,
            r.last_seen,
            r.superseded_by
                .as_deref()
                .map(|k| format!(", superseded by {k}"))
                .unwrap_or_default(),
            r.snippet
        );
    }
    if rows.is_empty() {
        s += "no matching traces\n";
    }
    s
}
