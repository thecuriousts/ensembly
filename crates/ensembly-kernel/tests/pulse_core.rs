//! Pulse core first slice: fixture-only tests. Nothing here touches the live
//! pulse-memory DB, premflow data, or any real G source.

use chrono::{DateTime, Utc};
use ensembly_kernel::pulse_core as pc;
use rusqlite::Connection;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

const SCHEMA: &str = "
CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
CREATE TABLE memory_traces (
  id INTEGER PRIMARY KEY, nat_key TEXT NOT NULL UNIQUE, snippet TEXT NOT NULL, source TEXT NOT NULL,
  time TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('user-stated','tool-verified','inferred','unknown')),
  kind TEXT NOT NULL CHECK (kind IN ('data','context','fact','memory')),
  lock TEXT NOT NULL DEFAULT 'open' CHECK (lock IN ('open','locked')),
  verify_hash TEXT, prefetch_key TEXT, seen_count INTEGER NOT NULL DEFAULT 1, superseded_by TEXT,
  first_seen TEXT NOT NULL, last_seen TEXT NOT NULL);
CREATE TABLE conflicts (id INTEGER PRIMARY KEY, subject TEXT NOT NULL, left_id INTEGER NOT NULL,
  right_id INTEGER NOT NULL, resolution TEXT NOT NULL, ts TEXT NOT NULL);
CREATE VIRTUAL TABLE memory_fts USING fts5(nat_key, snippet, source, content='memory_traces', content_rowid='id');
INSERT INTO schema_migrations VALUES (1, '2026-01-01T00:00:00+00:00');
";

struct Fx {
    _dir: TempDir,
    root: PathBuf,
    cfg: pc::PulseCfg,
}

fn now(s: &str) -> DateTime<Utc> {
    pc::parse_ts(s).unwrap()
}

fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn add_trace(
    c: &Connection,
    key: &str,
    snippet: &str,
    last_seen: &str,
    superseded_by: Option<&str>,
) {
    c.execute(
        "INSERT INTO memory_traces (nat_key, snippet, source, time, status, kind, superseded_by, first_seen, last_seen)
         VALUES (?1, ?2, 'fixture', ?3, 'tool-verified', 'memory', ?4, ?3, ?3)",
        rusqlite::params![key, snippet, last_seen, superseded_by],
    )
    .unwrap();
    let id = c.last_insert_rowid();
    c.execute(
        "INSERT INTO memory_fts(rowid, nat_key, snippet, source) VALUES (?1, ?2, ?3, 'fixture')",
        rusqlite::params![id, key, snippet],
    )
    .unwrap();
}

/// A fixture with one trace last seen 2026-10-18, an ok premflow stub, and a
/// premflow log. `now` is 2026-10-29, so the DB is 11 days stale.
fn fixture() -> Fx {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let db = root.join("pulse.sqlite");
    let c = Connection::open(&db).unwrap();
    c.execute_batch(SCHEMA).unwrap();
    add_trace(
        &c,
        "demo/alpha",
        "alpha snippet about invoices",
        "2026-10-18T14:14:26+00:00",
        None,
    );
    add_trace(
        &c,
        "demo/beta",
        "beta snippet 100% done_ok",
        "2026-10-10T09:00:00+00:00",
        None,
    );
    drop(c);
    fs::create_dir_all(root.join("home/.premflow")).unwrap();
    fs::write(
        root.join("home/.premflow/log.txt"),
        "[2026-10-27 23:36] [NOTE] fixture note\n[2026-10-20 10:00] [WIN] fixture win\nnot a log line\n",
    )
    .unwrap();
    script(&root.join("premflow"), "echo stats; exit 0");
    let mut cfg = pc::PulseCfg::from_env(now("2026-10-29T16:00:00+00:00"));
    cfg.db = db;
    cfg.premflow_bin = root.join("premflow");
    cfg.premflow_home = root.join("home");
    cfg.premflow_timeout = Duration::from_secs(5);
    cfg.missions_dir = root.join("missions");
    cfg.notes_dir = None;
    cfg.g_roots = vec![root.join("g")];
    fs::create_dir_all(root.join("g")).unwrap();
    Fx {
        _dir: dir,
        root,
        cfg,
    }
}

fn finding<'a>(r: &'a pc::CheckReport, id: &str) -> Option<&'a pc::Finding> {
    r.findings.iter().find(|f| f.id == id)
}

fn sha(p: &Path) -> String {
    pc::sha256_hex(&fs::read(p).unwrap())
}

const SPEC: &str = "# Fixture spec\n\n| Mission | Owner | Horizon | Current G text source | Links |\n|---|---|---|---|---|\n\
| Alpha plan (first) | Desk One | short | `g-alpha.md`, ledger notes | feeds beta |\n\
| Beta plan | Desk Two (guess) | long (Parked) | `gdir/` | depends on alpha |\n\
| Gamma plan | Desk Three | event | free text only | none |\n\nafter the table\n";

fn write_g(fx: &Fx) {
    fs::write(fx.root.join("g/g-alpha.md"), "alpha goal v1\n").unwrap();
    fs::create_dir_all(fx.root.join("g/gdir")).unwrap();
    fs::write(fx.root.join("g/gdir/one.md"), "one\n").unwrap();
    fs::write(fx.root.join("spec.md"), SPEC).unwrap();
}

// ------------------------------------------------------------------ staleness

#[test]
fn stale_detection_on_fixture_copy() {
    let fx = fixture();
    let r = pc::run_check(&fx.cfg);
    assert_eq!(r.days_stale, Some(11));
    assert_eq!(
        finding(&r, "db.stale").unwrap().severity,
        pc::Severity::Warn
    );
    assert_eq!((r.verdict.as_str(), r.exit_code), ("warn", 1));

    let mut c = fx.cfg.clone();
    c.now = now("2026-10-20");
    let r = pc::run_check(&c);
    assert!(
        finding(&r, "db.stale").is_none(),
        "2 days old must not warn"
    );
    assert_eq!(r.exit_code, 0);

    // Decision D4: staleness never fails, however old.
    let mut c = fx.cfg.clone();
    c.now = now("2027-12-31");
    let r = pc::run_check(&c);
    assert_eq!(
        finding(&r, "db.stale").unwrap().severity,
        pc::Severity::Warn
    );
    assert_eq!(r.exit_code, 1);
}

#[test]
fn counts_conflicts_and_dangling_superseded() {
    let fx = fixture();
    let c = Connection::open(&fx.cfg.db).unwrap();
    add_trace(
        &c,
        "demo/old",
        "old",
        "2026-10-28T00:00:00+00:00",
        Some("demo/missing"),
    );
    add_trace(
        &c,
        "demo/ok",
        "ok",
        "2026-10-28T00:00:00+00:00",
        Some("demo/alpha"),
    );
    c.execute("INSERT INTO conflicts (subject,left_id,right_id,resolution,ts) VALUES ('s',1,2,'open','2026-10-28')", [])
        .unwrap();
    c.execute("INSERT INTO conflicts (subject,left_id,right_id,resolution,ts) VALUES ('s',1,2,'kept left','2026-10-28')", [])
        .unwrap();
    drop(c);
    let r = pc::run_check(&fx.cfg);
    let db = r.db.as_ref().unwrap();
    assert_eq!(db.traces, 4);
    assert_eq!(db.by_status.get("tool-verified"), Some(&4));
    assert_eq!(db.unresolved_conflicts, 1);
    assert_eq!(db.dangling_superseded, vec!["demo/old".to_string()]);
    assert_eq!(
        finding(&r, "db.conflicts").unwrap().severity,
        pc::Severity::Fail
    );
    assert_eq!(
        finding(&r, "db.superseded").unwrap().severity,
        pc::Severity::Warn
    );
    assert_eq!(r.exit_code, 2);
}

#[test]
fn missing_db_fails_instead_of_creating_it() {
    let fx = fixture();
    let mut c = fx.cfg.clone();
    c.db = fx.root.join("nope.sqlite");
    let r = pc::run_check(&c);
    assert_eq!(r.exit_code, 2);
    assert!(finding(&r, "db").is_some());
    assert!(!c.db.exists(), "a read-only check must not create the DB");
}

// ------------------------------------------------------------------ read-only

#[test]
fn db_is_opened_read_only_and_never_modified() {
    let fx = fixture();
    let ro = pc::open_ro(&fx.cfg.db).unwrap();
    // Both layers must be on: the read-only open flag and the query_only pragma.
    assert!(ro.is_readonly(rusqlite::DatabaseName::Main).unwrap());
    let q: i64 = ro.query_row("PRAGMA query_only", [], |r| r.get(0)).unwrap();
    assert_eq!(q, 1);
    let err = ro
        .execute("INSERT INTO memory_traces (nat_key,snippet,source,time,status,kind,first_seen,last_seen) VALUES ('x','x','x','x','unknown','data','x','x')", [])
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("readonly")
            || err.to_string().to_lowercase().contains("read-only")
            || err.to_string().to_lowercase().contains("query_only")
            || err.to_string().to_lowercase().contains("attempt to write"),
        "{err}"
    );
    assert!(ro.execute("DELETE FROM memory_traces", []).is_err());
    drop(ro);

    let before = sha(&fx.cfg.db);
    let files_before: Vec<_> = fs::read_dir(&fx.root)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let _ = pc::run_check(&fx.cfg);
    let conn = pc::open_ro(&fx.cfg.db).unwrap();
    let _ = pc::read_prefix(&conn, "demo/", 10).unwrap();
    let _ = pc::read_search(&conn, "alpha", 10).unwrap();
    drop(conn);
    assert_eq!(
        sha(&fx.cfg.db),
        before,
        "check and read must leave the DB bytes unchanged"
    );
    let mut files_after: Vec<_> = fs::read_dir(&fx.root)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let mut fb = files_before;
    fb.sort();
    files_after.sort();
    assert_eq!(fb, files_after, "no journal, wal or temp files may appear");
}

// ------------------------------------------------------------------ premflow

#[test]
fn premflow_ok_reads_log() {
    let fx = fixture();
    let r = pc::run_check(&fx.cfg);
    assert_eq!(r.premflow.stats, "ok");
    assert_eq!(r.premflow.log_entries, 2);
    assert_eq!(
        r.premflow.log_last_entry.as_deref(),
        Some("2026-10-27 23:36")
    );
    assert!(finding(&r, "premflow.stats").is_none());
}

#[test]
fn premflow_missing_is_a_warning() {
    let fx = fixture();
    let mut c = fx.cfg.clone();
    c.premflow_bin = fx.root.join("does-not-exist");
    let r = pc::run_check(&c);
    assert_eq!(r.premflow.stats, "missing");
    assert_eq!(
        finding(&r, "premflow.stats").unwrap().severity,
        pc::Severity::Warn
    );
    assert_ne!(
        r.exit_code, 2,
        "a missing premflow is a warning, not a crash"
    );
}

#[test]
fn premflow_nonzero_is_reported() {
    let fx = fixture();
    script(&fx.root.join("pf-bad"), "exit 3");
    let mut c = fx.cfg.clone();
    c.premflow_bin = fx.root.join("pf-bad");
    let r = pc::run_check(&c);
    assert_eq!(
        (r.premflow.stats.as_str(), r.premflow.exit_code),
        ("nonzero", Some(3))
    );
}

#[test]
fn premflow_timeout_cuts_the_hang() {
    let fx = fixture();
    script(&fx.root.join("pf-hang"), "exec sleep 30");
    let mut c = fx.cfg.clone();
    c.premflow_bin = fx.root.join("pf-hang");
    c.premflow_timeout = Duration::from_millis(300);
    let t = std::time::Instant::now();
    let r = pc::run_check(&c);
    assert!(
        t.elapsed() < Duration::from_secs(5),
        "timeout must cut the hang short"
    );
    assert_eq!(r.premflow.stats, "timeout");
    assert_eq!(
        finding(&r, "premflow.stats").unwrap().severity,
        pc::Severity::Warn
    );
}

#[test]
fn premflow_gets_the_configured_home() {
    let fx = fixture();
    script(
        &fx.root.join("pf-home"),
        "[ \"$HOME\" = \"$EXPECT_HOME\" ] || exit 9; exit 0",
    );
    // The stub compares against a path baked into the script instead of env, to avoid env races.
    script(
        &fx.root.join("pf-home"),
        &format!(
            "[ \"$HOME\" = \"{}\" ] || exit 9; exit 0",
            fx.root.join("home").display()
        ),
    );
    let (st, code) = pc::run_premflow_stats(
        &fx.root.join("pf-home"),
        &fx.root.join("home"),
        Duration::from_secs(5),
    );
    assert_eq!((st.as_str(), code), ("ok", Some(0)));
}

// ------------------------------------------------------------------ G unchanged

#[test]
fn g_sources_are_hashed_and_left_untouched_by_check_and_read() {
    let fx = fixture();
    write_g(&fx);
    pc::import_missions(&fx.cfg, &fx.root.join("spec.md"), false).unwrap();
    let g_files = [
        fx.root.join("g/g-alpha.md"),
        fx.root.join("g/gdir/one.md"),
        fx.root.join("spec.md"),
    ];
    let before: Vec<_> = g_files.iter().map(|p| sha(p)).collect();
    let r = pc::run_check(&fx.cfg);
    let conn = pc::open_ro(&fx.cfg.db).unwrap();
    let _ = pc::read_search(&conn, "alpha", 5).unwrap();
    let after: Vec<_> = g_files.iter().map(|p| sha(p)).collect();
    assert_eq!(
        before, after,
        "G sources must be byte-identical after check and read"
    );
    assert_eq!(r.g_sources_hashed, 2);
    assert!(finding(&r, "g.unchanged").is_none());
    assert!(finding(&r, "g.drift").is_none());
}

#[test]
fn owner_edit_after_import_is_only_a_note() {
    let fx = fixture();
    write_g(&fx);
    pc::import_missions(&fx.cfg, &fx.root.join("spec.md"), false).unwrap();
    fs::write(fx.root.join("g/g-alpha.md"), "alpha goal v2 (owner edit)\n").unwrap();
    let r = pc::run_check(&fx.cfg);
    let f = finding(&r, "g.drift").expect("drift reported");
    assert_eq!(f.severity, pc::Severity::Note);
    assert!(f.message.contains("alpha-plan:g-alpha.md"), "{}", f.message);
    assert!(finding(&r, "g.unchanged").is_none());
}

#[test]
fn g_changing_during_a_check_fails_it() {
    let fx = fixture();
    write_g(&fx);
    pc::import_missions(&fx.cfg, &fx.root.join("spec.md"), false).unwrap();
    // A premflow stand-in that writes to G while the check runs: the check must catch it.
    script(
        &fx.root.join("pf-writer"),
        &format!(
            "echo tampered >> {}; exit 0",
            fx.root.join("g/g-alpha.md").display()
        ),
    );
    let mut c = fx.cfg.clone();
    c.premflow_bin = fx.root.join("pf-writer");
    let r = pc::run_check(&c);
    assert_eq!(
        finding(&r, "g.unchanged").unwrap().severity,
        pc::Severity::Fail
    );
    assert_eq!(r.exit_code, 2);
}

// ------------------------------------------------------------------ missions

#[test]
fn mission_table_parse_and_flags() {
    let m = pc::parse_mission_table(SPEC).unwrap();
    assert_eq!(m.len(), 3);
    assert_eq!(m[0].id, "alpha-plan");
    assert!(m[0].owner_confirmed);
    assert_eq!(m[0].horizon, "short");
    assert_eq!(m[0].status, "active");
    assert_eq!(
        m[0].g_files
            .iter()
            .map(|g| g.spec_path.as_str())
            .collect::<Vec<_>>(),
        vec!["g-alpha.md"]
    );
}

#[test]
fn mission_table_guess_and_parked_flags() {
    let m = pc::parse_mission_table(SPEC).unwrap();
    assert_eq!(m[1].owner, "Desk Two");
    assert!(!m[1].owner_confirmed, "(guess) owners stay unconfirmed");
    assert_eq!(
        (m[1].horizon.as_str(), m[1].status.as_str()),
        ("long", "parked")
    );
    assert!(m[2].g_files.is_empty());
}

#[test]
fn mission_table_rejects_bad_input() {
    assert!(pc::parse_mission_table("| Mission | Owner | Horizon | G | Links |\n|--|--|--|--|--|\n| X | o | someday | g | l |\n").is_err());
    assert!(pc::parse_mission_table("no table here").is_err());
}

#[test]
fn import_writes_only_the_new_store_and_refuses_overwrite() {
    let fx = fixture();
    write_g(&fx);
    let spec = fx.root.join("spec.md");
    let spec_sha = sha(&spec);
    let st = pc::import_missions(&fx.cfg, &spec, false).unwrap();
    assert_eq!(st.missions.len(), 3);
    assert!(st.missions[0].g_files[0].sha256.is_some());
    assert_eq!(sha(&spec), spec_sha, "spec must not change");
    assert_eq!(fs::read_dir(&fx.cfg.missions_dir).unwrap().count(), 1);

    let again = pc::import_missions(&fx.cfg, &spec, false);
    assert!(again.is_err(), "second import without --force must refuse");
    pc::import_missions(&fx.cfg, &spec, true).unwrap();
    let names: Vec<String> = fs::read_dir(&fx.cfg.missions_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().any(|n| n.starts_with("missions.json.bak-")),
        "{names:?}"
    );

    let loaded = pc::load_store(&fx.cfg.missions_dir).unwrap().unwrap();
    assert_eq!(loaded.missions, st.missions);
}

#[test]
fn check_flags_unconfirmed_owner_missing_owner_and_past_review() {
    let fx = fixture();
    write_g(&fx);
    pc::import_missions(&fx.cfg, &fx.root.join("spec.md"), false).unwrap();
    let r = pc::run_check(&fx.cfg);
    let f = finding(&r, "missions.owner").unwrap();
    assert_eq!(f.severity, pc::Severity::Note);
    assert!(f.message.contains("beta-plan"));

    let p = fx.cfg.missions_dir.join("missions.json");
    let mut st = pc::load_store(&fx.cfg.missions_dir).unwrap().unwrap();
    st.missions[0].review_date = Some("2026-10-01".into());
    st.missions[2].owner = " ".into();
    fs::write(&p, serde_json::to_string(&st).unwrap()).unwrap();
    let r = pc::run_check(&fx.cfg);
    assert_eq!(
        finding(&r, "missions.review").unwrap().severity,
        pc::Severity::Warn
    );
    assert!(r
        .findings
        .iter()
        .any(|f| f.id == "missions.owner" && f.severity == pc::Severity::Warn));
}

// ------------------------------------------------------------------ read

#[test]
fn read_by_prefix_and_search() {
    let fx = fixture();
    let c = pc::open_ro(&fx.cfg.db).unwrap();
    let rows = pc::read_prefix(&c, "demo/", 10).unwrap();
    assert_eq!(
        rows.iter().map(|r| r.nat_key.as_str()).collect::<Vec<_>>(),
        vec!["demo/alpha", "demo/beta"]
    );
    assert!(pc::read_prefix(&c, "demo/a", 10).unwrap().len() == 1);
    assert!(
        pc::read_prefix(&c, "%", 10).unwrap().is_empty(),
        "prefix is literal, not a pattern"
    );
    let hits = pc::read_search(&c, "invoices", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].nat_key, "demo/alpha");
    assert!(pc::read_search(&c, "nonexistentword", 10)
        .unwrap()
        .is_empty());
    // FTS-hostile input falls back to a literal match instead of erroring.
    assert_eq!(pc::read_search(&c, "100% done_ok", 10).unwrap().len(), 1);
    assert_eq!(
        pc::read_search(&c, "\"quoted\" AND (", 10).unwrap().len(),
        0
    );
}

// ------------------------------------------------------------------ CLI

fn bin(fx: &Fx, args: &[&str]) -> std::process::Output {
    bin_at(fx, "2026-10-29T16:00:00+00:00", args)
}

fn bin_at(fx: &Fx, now: &str, args: &[&str]) -> std::process::Output {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_ensembly"));
    cmd.arg("pulse")
        .arg("--pulse-db")
        .arg(&fx.cfg.db)
        .arg("--premflow-bin")
        .arg(&fx.cfg.premflow_bin)
        .arg("--missions-dir")
        .arg(&fx.cfg.missions_dir)
        .args(["--now", now])
        .args(args)
        .env("HOME", fx.root.join("home"))
        .env("ENSEMBLY_PULSE_NOTES", fx.root.join("no-notes"))
        .env("ENSEMBLY_PULSE_G_ROOTS", fx.root.join("g"));
    cmd.output().unwrap()
}

#[test]
fn cli_exit_codes_and_json() {
    let fx = fixture();
    let out = bin(&fx, &["check", "--json"]);
    assert_eq!(out.status.code(), Some(1), "stale fixture is a warning");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["verdict"], "warn");
    assert_eq!(v["days_stale"], 11);
}

#[test]
fn cli_ok_when_fresh() {
    let fx = fixture();
    let out = bin_at(&fx, "2026-10-20", &["check"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn cli_read_and_refused_write() {
    let fx = fixture();
    let out = bin(&fx, &["read", "demo/a"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("demo/alpha"));
    // Refused write: a second import without --force exits REFUSED_WRITE_EXIT.
    fs::write(fx.root.join("spec.md"), SPEC).unwrap();
    let spec = fx.root.join("spec.md");
    let spec = spec.to_str().unwrap();
    let first = bin(&fx, &["missions", "import", "--spec", spec]);
    assert_eq!(first.status.code(), Some(0));
    let second = bin(&fx, &["missions", "import", "--spec", spec]);
    assert_eq!(second.status.code(), Some(7));
    assert_eq!(second.status.code(), Some(pc::REFUSED_WRITE_EXIT));
    assert_ne!(
        bin(&fx, &["read"]).status.code(),
        Some(0),
        "read needs a prefix or --search"
    );
    assert_ne!(bin_at(&fx, "garbage", &["check"]).status.code(), Some(0));
}

/// No exit code may mean two different things across `pulse` and `apps audit status`:
/// pulse 0 ok / 1 warn / 2 fail / REFUSED_WRITE_EXIT; audit 0 pass / 3 flag / 4 block /
/// 5 unaudited or stale (1/2 errors). 0 is success in both.
#[test]
fn pulse_and_audit_exit_codes_do_not_clash() {
    use ensembly_kernel::company_audit::{exit_code, GateStatus};
    let audit: Vec<i32> = [
        GateStatus::Flag,
        GateStatus::Block,
        GateStatus::Unaudited,
        GateStatus::Stale,
    ]
    .iter()
    .map(|s| exit_code(*s))
    .collect();
    assert_eq!(audit, [3, 4, 5, 5], "audit codes are a contract with callers");
    assert_eq!(exit_code(GateStatus::Pass), 0);
    let pulse = [1, 2, pc::REFUSED_WRITE_EXIT];
    for c in pulse {
        assert!(!audit.contains(&c), "pulse exit {c} is also an audit status");
    }
}
