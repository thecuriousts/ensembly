//! CLI bytes captured before ledger and audit types moved onto ensembly-schema.
//! Inputs are the fictional schema fixtures. Pulse refusal is exit code 7.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LEDGER_LIST: &str = include_str!("golden/ledger-list.md");
const LEDGER_EXPORT: &str = include_str!("golden/ledger-export.csv");
const AUDIT_STATUS: &str = include_str!("golden/audit-status.txt");
const AUDIT_EXPORT_CSV: &str = include_str!("golden/audit-export.csv");
const AUDIT_EXPORT_MD: &str = include_str!("golden/audit-export.md");
const PULSE_REFUSED: &str = include_str!("golden/pulse-refused.txt");

const STATUS_CASES: &[(&str, &str)] = &[
    ("acme.example.test", "2026-10-27T12:00:00+01:00"),
    ("bramble.example.test", "2026-10-27T12:00:00+01:00"),
    ("cobalt.example.test", "2026-10-27T12:00:00+01:00"),
    ("missing.example.test", "2026-10-27T12:00:00+01:00"),
    ("acme.example.test", "2027-01-01T12:00:00+01:00"),
];

const SPEC: &str = "\
# Fixture spec

| Mission | Owner | Horizon | Current G text source | Links |
|---|---|---|---|---|
| Acme plan | Desk One | short | notes only | none |
";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ensembly-schema/fixtures")
        .join(name)
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .args(args)
        .output()
        .expect("run ensembly")
}

fn assert_ok(out: &Output) {
    assert!(
        out.status.success(),
        "exit {:?}\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn ledger_list_and_export_match_captured_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = fixture("ledger-v0.jsonl");
    let db = dir.path().join("ops.sqlite");
    let out_dir = dir.path().join("out");
    let out = run(&[
        "--no-memory",
        "--db",
        db.to_str().unwrap(),
        "apps",
        "--ledger",
        ledger.to_str().unwrap(),
        "export",
        "--out-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert_ok(&out);
    assert_eq!(read(&out_dir.join("applications.md")), LEDGER_LIST);
    assert_eq!(read(&out_dir.join("applications.csv")), LEDGER_EXPORT);
}

#[test]
fn audit_status_and_export_match_captured_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = fixture("audit-v0.jsonl");
    let db = dir.path().join("ops.sqlite");
    let out_dir = dir.path().join("out");
    let exported = run(&[
        "--no-memory",
        "--db",
        db.to_str().unwrap(),
        "apps",
        "audit",
        "--store",
        store.to_str().unwrap(),
        "export",
        "--out-dir",
        out_dir.to_str().unwrap(),
    ]);
    assert_ok(&exported);
    assert_eq!(read(&out_dir.join("company-audits.csv")), AUDIT_EXPORT_CSV);
    assert_eq!(read(&out_dir.join("company-audits.md")), AUDIT_EXPORT_MD);
    assert_eq!(
        status_transcript(db.to_str().unwrap(), store.to_str().unwrap()),
        AUDIT_STATUS
    );
}

fn status_transcript(db: &str, store: &str) -> String {
    let mut out = String::new();
    for (query, now) in STATUS_CASES {
        let got = run(&[
            "--no-memory",
            "--db",
            db,
            "apps",
            "audit",
            "--store",
            store,
            "status",
            query,
            "--now",
            now,
        ]);
        let code = got.status.code().expect("exit code");
        let body = String::from_utf8(got.stdout).expect("utf8");
        let body = body.trim_end_matches('\n');
        out.push_str(&format!("{code}\t{query}\t{now}\t{body}\n"));
    }
    out
}

#[test]
fn pulse_refusal_matches_captured_exit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let spec = dir.path().join("spec.md");
    fs::write(&spec, SPEC).unwrap();
    let missions = dir.path().join("missions");
    let g = dir.path().join("g");
    let home = dir.path().join("home");
    let notes = dir.path().join("notes");
    fs::create_dir_all(&g).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&notes).unwrap();
    let first = pulse_import(dir.path(), &spec, &missions, &g, &home, &notes);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = pulse_import(dir.path(), &spec, &missions, &g, &home, &notes);
    let err = String::from_utf8(second.stderr).expect("utf8");
    let missions_file = missions.join("missions.json");
    let err = err.replace(&missions_file.display().to_string(), "<missions>");
    let got = format!("{}\n{err}", second.status.code().expect("exit"));
    assert_eq!(got, PULSE_REFUSED);
}

fn pulse_import(
    root: &Path,
    spec: &Path,
    missions: &Path,
    g: &Path,
    home: &Path,
    notes: &Path,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .arg("--no-memory")
        .arg("--db")
        .arg(root.join("ops.sqlite"))
        .arg("pulse")
        .arg("--pulse-db")
        .arg(root.join("pulse.sqlite"))
        .arg("--premflow-bin")
        .arg(root.join("premflow"))
        .arg("--missions-dir")
        .arg(missions)
        .arg("--now")
        .arg("2026-10-27T12:00:00+01:00")
        .arg("missions")
        .arg("import")
        .arg("--spec")
        .arg(spec)
        .env("HOME", home)
        .env("ENSEMBLY_PULSE_G_ROOTS", g)
        .env("ENSEMBLY_PULSE_NOTES", notes)
        .output()
        .expect("run ensembly")
}
