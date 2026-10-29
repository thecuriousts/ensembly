//! CLI bytes captured before types moved onto ensembly-schema.
//! Ledger list, audit status, export, and pulse refusal (exit 7).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LEDGER_LIST: &str = include_str!("golden/ledger-list.md");
const LEDGER_EXPORT: &str = include_str!("golden/ledger-export.csv");
const AUDIT_STATUS: &str = include_str!("golden/audit-status.txt");
const AUDIT_CSV: &str = include_str!("golden/audit-export.csv");
const AUDIT_MD: &str = include_str!("golden/audit-export.md");
const PULSE_REFUSED: &str = include_str!("golden/pulse-refused.txt");
const SPEC: &str = "\
# Fixture spec

| Mission | Owner | Horizon | Current G text source | Links |
|---|---|---|---|---|
| Acme plan | Desk One | short | notes only | none |
";

fn fix(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ensembly-schema/fixtures")
        .join(name)
}

fn cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ensembly"));
    c.arg("--no-memory").arg("--db").arg(dir.join("ops.sqlite"));
    c
}

fn run(cmd: &mut Command) -> Output {
    cmd.output().expect("run ensembly")
}

fn ok(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn captured_cli_bytes() {
    let dir = tempfile::tempdir().expect("temp");
    ledger_and_audit(dir.path());
    pulse(dir.path());
}

fn ledger_and_audit(dir: &Path) {
    let ledger = fix("ledger-v0.jsonl");
    let audit = fix("audit-v0.jsonl");
    let exp = dir.join("exp");
    let aud = dir.join("aud");
    ok(&run(cmd(dir)
        .args(["apps", "--ledger"])
        .arg(&ledger)
        .args(["export", "--out-dir"])
        .arg(&exp)));
    assert_eq!(read(&exp.join("applications.md")), LEDGER_LIST);
    assert_eq!(read(&exp.join("applications.csv")), LEDGER_EXPORT);
    ok(&run(cmd(dir)
        .args(["apps", "audit", "--store"])
        .arg(&audit)
        .args(["export", "--out-dir"])
        .arg(&aud)));
    assert_eq!(read(&aud.join("company-audits.csv")), AUDIT_CSV);
    assert_eq!(read(&aud.join("company-audits.md")), AUDIT_MD);
    assert_eq!(transcript(dir, &audit), AUDIT_STATUS);
}

fn transcript(dir: &Path, store: &Path) -> String {
    let mut buf = String::new();
    for line in AUDIT_STATUS.lines() {
        let col: Vec<&str> = line.splitn(4, '\t').collect();
        let (query, now) = (col[1], col[2]);
        let got = run(cmd(dir)
            .args(["apps", "audit", "--store"])
            .arg(store)
            .arg("status")
            .arg(query)
            .args(["--now", now]));
        let body = String::from_utf8(got.stdout).expect("utf8");
        let code = got.status.code().expect("code");
        buf.push_str(&format!(
            "{code}\t{query}\t{now}\t{}\n",
            body.trim_end_matches('\n')
        ));
    }
    buf
}

fn pulse(dir: &Path) {
    let spec = dir.join("spec.md");
    fs::write(&spec, SPEC).unwrap();
    fs::create_dir_all(dir.join("g")).unwrap();
    fs::create_dir_all(dir.join("home")).unwrap();
    fs::create_dir_all(dir.join("notes")).unwrap();
    let missions = dir.join("missions");
    ok(&run(&mut pulse_cmd(dir, &spec, &missions)));
    let second = run(&mut pulse_cmd(dir, &spec, &missions));
    let err = String::from_utf8(second.stderr).expect("utf8");
    let path = missions.join("missions.json").display().to_string();
    let err = err.replace(&path, "<missions>");
    let code = second.status.code().expect("code");
    assert_eq!(format!("{code}\n{err}"), PULSE_REFUSED);
}

fn pulse_cmd(dir: &Path, spec: &Path, missions: &Path) -> Command {
    let mut c = cmd(dir);
    c.args(["pulse", "--pulse-db"])
        .arg(dir.join("pulse.sqlite"))
        .args(["--premflow-bin"])
        .arg(dir.join("premflow"))
        .args(["--missions-dir"])
        .arg(missions)
        .args([
            "--now",
            "2026-10-27T12:00:00+01:00",
            "missions",
            "import",
            "--spec",
        ])
        .arg(spec)
        .env("HOME", dir.join("home"))
        .env("ENSEMBLY_PULSE_G_ROOTS", dir.join("g"))
        .env("ENSEMBLY_PULSE_NOTES", dir.join("notes"));
    c
}
