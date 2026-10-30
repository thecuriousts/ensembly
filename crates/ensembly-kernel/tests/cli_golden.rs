//! CLI bytes for ledger export, audit export, and pulse refusal.
//! Versioned lines: v0 and v1 pass; v2 and the older refusals do not.

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

fn text_of(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{e}"))
}

fn see(out: &Output, ok: bool, needle: &str) {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.success(), ok, "{stdout}{stderr}");
    let msg = if ok { stdout } else { stderr };
    assert!(msg.contains(needle), "{msg}");
}

fn call(dir: &Path, args: &[&str]) -> Output {
    run(cmd(dir).args(args))
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
    see(
        &run(cmd(dir)
            .args(["apps", "--ledger"])
            .arg(&ledger)
            .args(["export", "--out-dir"])
            .arg(&exp)),
        true,
        "exported",
    );
    assert_eq!(text_of(&exp.join("applications.md")), LEDGER_LIST);
    assert_eq!(text_of(&exp.join("applications.csv")), LEDGER_EXPORT);
    see(
        &run(cmd(dir)
            .args(["apps", "audit", "--store"])
            .arg(&audit)
            .args(["export", "--out-dir"])
            .arg(&aud)),
        true,
        "exported",
    );
    assert_eq!(text_of(&aud.join("company-audits.csv")), AUDIT_CSV);
    assert_eq!(text_of(&aud.join("company-audits.md")), AUDIT_MD);
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
    let first = run(&mut pulse_cmd(dir, &spec, &missions));
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
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

fn put(dir: &Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    path.into_os_string().into_string().unwrap()
}

fn slurp(name: &str) -> String {
    fs::read_to_string(fix(name)).unwrap()
}

fn ledger(dir: &Path, file: &str, ok: bool, needle: &str) {
    see(
        &call(dir, &["apps", "--ledger", file, "validate"]),
        ok,
        needle,
    );
}

fn audit_file(dir: &Path, file: &str, ok: bool, needle: &str) {
    see(&call(dir, &["apps", "audit", "validate", file]), ok, needle);
}

fn append_to(dir: &Path, store: &str, file: &str, ok: bool, needle: &str) {
    let args = ["apps", "audit", "--store", store, "append", file];
    see(&call(dir, &args), ok, needle);
}

#[test]
fn versioned_lines_and_old_refusals() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let owned = |n: &str| fix(n).into_os_string().into_string().unwrap();
    let lv0 = owned("ledger-v0.jsonl");
    let lv1 = owned("ledger-v1.jsonl");
    let av0 = owned("audit-v0.jsonl");
    let av1 = owned("audit-v1.jsonl");
    ledger(dir, &lv0, true, "ok: 6 rows");
    ledger(dir, &lv1, true, "ok: 6 rows");
    audit_file(dir, &av0, true, "ok: 4 audits");
    audit_file(dir, &av1, true, "ok: 4 audits");
    let store = dir.join("a.jsonl").into_os_string().into_string().unwrap();
    let one0 = put(
        dir,
        "one0.jsonl",
        slurp("audit-v0.jsonl").lines().next().unwrap(),
    );
    let one1 = put(
        dir,
        "one1.jsonl",
        slurp("audit-v1.jsonl").lines().next().unwrap(),
    );
    append_to(dir, &store, &one0, true, "appended 1");
    append_to(dir, &store, &one1, true, "skipped 1");
    let led2 = put(
        dir,
        "led2.jsonl",
        &slurp("ledger-v1.jsonl").replace("\"v\":1", "\"v\":2"),
    );
    let aud2 = put(
        dir,
        "aud2.jsonl",
        &slurp("audit-v1.jsonl").replace("\"v\":1", "\"v\":2"),
    );
    ledger(dir, &led2, false, "unsupported v 2");
    audit_file(dir, &aud2, false, "unsupported v 2");
    let empty = dir.join("e.jsonl").into_os_string().into_string().unwrap();
    append_to(dir, &empty, &aud2, false, "unsupported v 2");
    assert!(!Path::new(&empty).exists());
    let row = r#"{"company":"Acme Fictional","role":"Widget Engineer","link":"https://jobs.example.test/acme/1","status":"prepared","submit_time":null,"cv_version":"cv-sample-1"}"#;
    let hired = put(
        dir,
        "hired.jsonl",
        &row.replace("\"prepared\"", "\"hired\""),
    );
    let wrong = put(
        dir,
        "case.jsonl",
        &row.replace("\"prepared\"", "\"Prepared\""),
    );
    let dup = put(
        dir,
        "dup.jsonl",
        &row.replacen("{", r#"{"company":"Bramble Fictional","#, 1),
    );
    ledger(dir, &hired, false, "unknown variant");
    ledger(dir, &wrong, false, "unknown variant");
    ledger(dir, &dup, false, "duplicate key");
    let line = slurp("audit-v0.jsonl").lines().next().unwrap().to_string();
    let adup = put(
        dir,
        "adup.jsonl",
        &line.replacen("{", r#"{"domain":"other.example.test","#, 1),
    );
    audit_file(dir, &adup, false, "duplicate key");
}
