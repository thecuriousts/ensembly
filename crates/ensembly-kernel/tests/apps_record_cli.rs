//! CLI import appends record events. Compare only reads.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn ensembly(dir: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ensembly"));
    cmd.arg("--no-memory")
        .arg("--db")
        .arg(dir.join("ops.sqlite"))
        .current_dir(dir);
    cmd
}

fn run(cmd: &mut Command) -> Output {
    cmd.output().expect("run kernel bin")
}

fn ledger_body() -> &'static str {
    r#"{"company":"Birch Fictional","role":"Ledger Clerk","link":"https://jobs.example.test/birch/2","status":"prepared","submit_time":null,"cv_version":"cv-sample-1"}
{"company":"Birch Fictional","role":"Ledger Clerk","link":"https://jobs.example.test/birch/2","status":"submitted","submit_time":"2026-10-26T18:59:00+01:00","cv_version":"cv-sample-1"}
{"company":"Cedar Example","role":"Stage Hand","link":"https://jobs.example.test/cedar/3","status":"interview","submit_time":"2026-10-27T09:00:00+01:00","cv_version":"cv-sample-1"}
{"company":"Drift Sample","role":"Signal Reader","link":"https://jobs.example.test/drift/4","status":"rejected","submit_time":null,"submit_time_unknown":true,"cv_version":"cv-sample-1"}
{"company":"Elm Fictional","role":"Path Finder","link":"https://jobs.example.test/elm/5","status":"withdrawn","submit_time":null,"cv_version":"cv-sample-1"}
{"company":"Fern Example","role":"Quiet Desk","link":"https://jobs.example.test/fern/6","status":"silent-closed","submit_time":"2026-10-28T15:00:00+01:00","cv_version":"cv-sample-1"}
{"company":"Amber Sample Works","role":"Widget Binder","link":"https://jobs.example.test/amber/1","status":"prepared","submit_time":null,"cv_version":"cv-sample-1"}
"#
}

fn write_ledger(dir: &Path) -> PathBuf {
    let data = dir.join("data");
    fs::create_dir_all(&data).unwrap();
    let ledger = data.join("ledger.jsonl");
    fs::write(&ledger, ledger_body()).unwrap();
    ledger
}

fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut paths = Vec::new();
    collect(dir, &mut paths);
    paths.sort();
    for path in paths {
        let rel = path
            .strip_prefix(dir)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        out.push((rel, fs::read(&path).unwrap_or_default()));
    }
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for ent in fs::read_dir(dir).unwrap() {
        let path = ent.unwrap().path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn import_is_idempotent_and_leaves_ledger_bytes_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = write_ledger(dir.path());
    let log = dir.path().join("data").join("events.jsonl");
    let before = fs::read(&ledger).unwrap();
    let first = run(ensembly(dir.path()).args([
        "record",
        "apps",
        "import",
        "--ledger",
        ledger.to_str().unwrap(),
        "--log",
        log.to_str().unwrap(),
        "--writer",
        "laptop-1",
        "--at",
        "2026-11-01T12:00:00+01:00",
    ]));
    let stdout = String::from_utf8_lossy(&first.stdout);
    let stderr = String::from_utf8_lossy(&first.stderr);
    assert!(first.status.success(), "{stdout}{stderr}");
    assert_eq!(fs::read(&ledger).unwrap(), before);
    assert!(before.starts_with(br#"{"company":"Birch Fictional""#));
    assert!(stdout.contains("\"appended\":6"), "{stdout}");
    let log_once = fs::read(&log).unwrap();
    assert_eq!(log_once.iter().filter(|b| **b == b'\n').count(), 6);

    let second = run(ensembly(dir.path()).args([
        "record",
        "apps",
        "import",
        "--ledger",
        ledger.to_str().unwrap(),
        "--log",
        log.to_str().unwrap(),
        "--writer",
        "laptop-1",
        "--at",
        "2026-11-01T12:00:00+01:00",
    ]));
    let stdout = String::from_utf8_lossy(&second.stdout);
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(second.status.success(), "{stdout}{stderr}");
    assert_eq!(fs::read(&log).unwrap(), log_once);
    assert_eq!(fs::read(&ledger).unwrap(), before);
    assert!(stdout.contains("\"appended\":0"), "{stdout}");
    assert!(stdout.contains("\"skipped\":6"), "{stdout}");
}

#[test]
fn unknown_status_writes_neither_ledger_nor_log() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    fs::create_dir_all(&data).unwrap();
    let ledger = data.join("ledger.jsonl");
    let body = b"{\"company\":\"Birch Fictional\",\"role\":\"Ledger Clerk\",\"link\":\"https://jobs.example.test/birch/2\",\"status\":\"offered\",\"submit_time\":\"2026-10-26T18:59:00+01:00\",\"cv_version\":\"cv-sample-1\"}\n";
    fs::write(&ledger, body).unwrap();
    let log = data.join("events.jsonl");
    let out = run(ensembly(dir.path()).args([
        "record",
        "apps",
        "import",
        "--ledger",
        ledger.to_str().unwrap(),
        "--log",
        log.to_str().unwrap(),
        "--writer",
        "laptop-1",
        "--at",
        "2026-11-01T12:00:00+01:00",
    ]));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.status.success(), "{text}");
    assert!(
        text.contains("offered") || text.contains("unknown"),
        "{text}"
    );
    assert_eq!(fs::read(&ledger).unwrap(), body);
    assert!(!log.exists());
}

#[test]
fn compare_is_read_only_and_exits_nonzero_on_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = write_ledger(dir.path());
    let log = dir.path().join("data").join("events.jsonl");
    let imported = run(ensembly(dir.path()).args([
        "record",
        "apps",
        "import",
        "--ledger",
        ledger.to_str().unwrap(),
        "--log",
        log.to_str().unwrap(),
        "--writer",
        "mac-mini",
        "--at",
        "2026-11-01T12:00:00+01:00",
    ]));
    assert!(
        imported.status.success(),
        "{}{}",
        String::from_utf8_lossy(&imported.stdout),
        String::from_utf8_lossy(&imported.stderr)
    );

    let data = dir.path().join("data");
    let before = snapshot(&data);
    let mut mode = fs::metadata(&data).unwrap().permissions();
    mode.set_mode(0o555);
    fs::set_permissions(&data, mode).unwrap();
    let compared = run(ensembly(dir.path()).args([
        "record",
        "apps",
        "compare",
        "--ledger",
        ledger.to_str().unwrap(),
        "--log",
        log.to_str().unwrap(),
    ]));
    let mut restore = fs::metadata(&data).unwrap().permissions();
    restore.set_mode(0o755);
    fs::set_permissions(&data, restore).unwrap();
    let stdout = String::from_utf8_lossy(&compared.stdout);
    let stderr = String::from_utf8_lossy(&compared.stderr);
    assert!(compared.status.success(), "{stdout}{stderr}");
    assert!(stdout.contains("jobs.example.test/birch/2"), "{stdout}");
    assert_eq!(snapshot(&data), before);

    let bad = data.join("events-bad.jsonl");
    let mutated = fs::read_to_string(&log).unwrap().replacen(
        "\"status\":\"submitted\"",
        "\"status\":\"withdrawn\"",
        1,
    );
    fs::write(&bad, mutated).unwrap();
    let ledger_bytes = fs::read(&ledger).unwrap();
    let log_bytes = fs::read(&log).unwrap();
    let bad_bytes = fs::read(&bad).unwrap();
    let mismatched = run(ensembly(dir.path()).args([
        "record",
        "apps",
        "compare",
        "--ledger",
        ledger.to_str().unwrap(),
        "--log",
        bad.to_str().unwrap(),
    ]));
    let stdout = String::from_utf8_lossy(&mismatched.stdout);
    let stderr = String::from_utf8_lossy(&mismatched.stderr);
    assert_eq!(mismatched.status.code(), Some(1), "{stdout}{stderr}");
    assert!(stdout.contains("jobs.example.test/birch/2"), "{stdout}");
    assert!(stdout.contains("submitted"), "{stdout}");
    assert!(stdout.contains("withdrawn"), "{stdout}");
    assert_eq!(fs::read(&ledger).unwrap(), ledger_bytes);
    assert_eq!(fs::read(&log).unwrap(), log_bytes);
    assert_eq!(fs::read(&bad).unwrap(), bad_bytes);
}

fn unknown_time_body() -> &'static str {
    r#"{"company":"Drift Sample","role":"Signal Reader","link":"https://jobs.example.test/drift/4","status":"rejected","submit_time":null,"submit_time_unknown":true,"cv_version":"cv-sample-1"}
{"company":"Fern Example","role":"Quiet Desk","link":"https://jobs.example.test/fern/7","status":"silent-closed","submit_time":null,"submit_time_unknown":true,"cv_version":"cv-sample-1"}
"#
}

fn import_args<'a>(ledger: &'a str, log: &'a str, writer: &'a str) -> Vec<&'a str> {
    vec![
        "record", "apps", "import", "--ledger", ledger, "--log", log, "--writer", writer,
    ]
}

#[test]
fn reimport_without_at_appends_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    fs::create_dir_all(&data).unwrap();
    let ledger = data.join("ledger.jsonl");
    fs::write(&ledger, unknown_time_body()).unwrap();
    let log = data.join("events.jsonl");
    let ledger_s = ledger.to_str().unwrap();
    let log_s = log.to_str().unwrap();
    let first = run(ensembly(dir.path()).args(import_args(ledger_s, log_s, "laptop-1")));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.status.success(), "{text}");
    let once = fs::read(&log).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(2));
    let second = run(ensembly(dir.path()).args(import_args(ledger_s, log_s, "laptop-1")));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(second.status.success(), "{text}");
    assert!(text.contains("\"appended\":0"), "{text}");
    assert_eq!(fs::read(&log).unwrap(), once);
    assert_eq!(fs::read(&ledger).unwrap(), unknown_time_body().as_bytes());
}

#[test]
fn reimport_with_another_writer_appends_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    fs::create_dir_all(&data).unwrap();
    let ledger = data.join("ledger.jsonl");
    fs::write(&ledger, unknown_time_body()).unwrap();
    let log = data.join("events.jsonl");
    let ledger_s = ledger.to_str().unwrap();
    let log_s = log.to_str().unwrap();
    let first = run(ensembly(dir.path()).args(import_args(ledger_s, log_s, "laptop-1")));
    assert!(
        first.status.success(),
        "{}{}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let once = fs::read(&log).unwrap();
    let second = run(ensembly(dir.path()).args(import_args(ledger_s, log_s, "laptop-2")));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(second.status.success(), "{text}");
    assert!(text.contains("\"appended\":0"), "{text}");
    assert_eq!(fs::read(&log).unwrap(), once);
}

#[test]
fn return_to_an_earlier_stage_exits_nonzero_and_names_the_link() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    fs::create_dir_all(&data).unwrap();
    let ledger = data.join("ledger.jsonl");
    let link = "https://jobs.example.test/birch/2";
    let submitted = format!(
        "{{\"company\":\"Birch Fictional\",\"role\":\"Ledger Clerk\",\"link\":\"{link}\",\"status\":\"submitted\",\"submit_time\":\"2026-10-26T18:59:00+01:00\",\"cv_version\":\"cv-sample-1\"}}\n"
    );
    let interview = submitted.replace("submitted", "interview");
    fs::write(&ledger, &submitted).unwrap();
    let log = data.join("events.jsonl");
    let ledger_s = ledger.to_str().unwrap();
    let log_s = log.to_str().unwrap();
    let at = ["--at", "2026-11-01T12:00:00+01:00"];
    let first = run(ensembly(dir.path())
        .args(import_args(ledger_s, log_s, "laptop-1"))
        .args(at));
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    fs::write(&ledger, format!("{submitted}{interview}")).unwrap();
    let second = run(ensembly(dir.path())
        .args(import_args(ledger_s, log_s, "laptop-1"))
        .args(at));
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let log_twice = fs::read(&log).unwrap();
    fs::write(&ledger, format!("{submitted}{interview}{submitted}")).unwrap();
    let ledger_bytes = fs::read(&ledger).unwrap();
    let third = run(ensembly(dir.path())
        .args(import_args(ledger_s, log_s, "laptop-1"))
        .args(at));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&third.stdout),
        String::from_utf8_lossy(&third.stderr)
    );
    assert!(!third.status.success(), "{text}");
    assert!(text.contains(link), "{text}");
    assert!(text.contains("submitted"), "{text}");
    assert!(text.contains("next-stage:1"), "{text}");
    assert_eq!(fs::read(&log).unwrap(), log_twice);
    assert_eq!(fs::read(&ledger).unwrap(), ledger_bytes);
    assert!(ledger_bytes.starts_with(submitted.as_bytes()));
}

#[test]
fn compare_exit_code_splits_mismatch_from_error() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = write_ledger(dir.path());
    let log = dir.path().join("data").join("events.jsonl");
    fs::write(&log, b"not-json\n").unwrap();
    let garbage = run(ensembly(dir.path()).args([
        "record",
        "apps",
        "compare",
        "--ledger",
        ledger.to_str().unwrap(),
        "--log",
        log.to_str().unwrap(),
    ]));
    assert_eq!(
        garbage.status.code(),
        Some(2),
        "{}{}",
        String::from_utf8_lossy(&garbage.stdout),
        String::from_utf8_lossy(&garbage.stderr)
    );
    let missing_ledger = dir.path().join("missing-ledger.jsonl");
    let missing = run(ensembly(dir.path()).args([
        "record",
        "apps",
        "compare",
        "--ledger",
        missing_ledger.to_str().unwrap(),
        "--log",
        log.to_str().unwrap(),
    ]));
    assert_eq!(
        missing.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
}

#[test]
fn compare_missing_log_or_directory_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = write_ledger(dir.path());
    let missing = dir.path().join("data").join("no-such.jsonl");
    assert!(!missing.exists());
    let out = run(ensembly(dir.path()).args([
        "record",
        "apps",
        "compare",
        "--ledger",
        ledger.to_str().unwrap(),
        "--log",
        missing.to_str().unwrap(),
    ]));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(2), "{text}");
    assert!(text.contains("missing"), "{text}");
    assert!(!missing.exists());

    let as_dir = dir.path().join("data").join("log-dir");
    fs::create_dir(&as_dir).unwrap();
    let out = run(ensembly(dir.path()).args([
        "record",
        "apps",
        "compare",
        "--ledger",
        ledger.to_str().unwrap(),
        "--log",
        as_dir.to_str().unwrap(),
    ]));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(2), "{text}");
    assert!(text.contains("directory"), "{text}");
}
