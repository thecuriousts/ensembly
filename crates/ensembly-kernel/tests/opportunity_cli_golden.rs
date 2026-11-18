//! CLI bytes for ledger export and audit export (opportunity-finder pack).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LEDGER_LIST: &str = include_str!("golden/ledger-list.md");
const LEDGER_EXPORT: &str = include_str!("golden/ledger-export.csv");
const AUDIT_STATUS: &str = include_str!("golden/audit-status.txt");
const AUDIT_CSV: &str = include_str!("golden/audit-export.csv");
const AUDIT_MD: &str = include_str!("golden/audit-export.md");

fn fix(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ensembly-opportunity-finder/fixtures")
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

#[test]
fn legacy_submit_time_warns_strict_fails_and_append_keeps_the_old_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let body = slurp("ledger-legacy-submit-time.jsonl");
    let ledger_path = dir.join("ledger.jsonl");
    fs::write(&ledger_path, &body).unwrap();
    let ledger = ledger_path.to_str().unwrap();

    let ok = call(dir, &["apps", "--ledger", ledger, "validate"]);
    see(&ok, true, "ok: 2 rows, 2 current applications");
    let warned = String::from_utf8_lossy(&ok.stderr);
    assert!(
        warned.contains("warning: line 1: legacy: submit_time missing"),
        "{warned}"
    );
    assert!(
        warned.contains("warning: line 3: legacy: submit_time missing"),
        "{warned}"
    );

    let strict = call(dir, &["apps", "--ledger", ledger, "validate", "--strict"]);
    see(
        &strict,
        false,
        "legacy: submit_time missing on line(s) 1, 3",
    );
    assert_eq!(strict.status.code(), Some(1));

    let bytes = fs::read(&ledger_path).unwrap();
    ensembly_schema::write_seal(&ledger_path, &bytes).unwrap();
    ensembly_schema::lock(&ledger_path).unwrap();
    see(
        &call(
            dir,
            &[
                "apps",
                "--ledger",
                ledger,
                "append",
                "--company",
                "Pebble Sample",
                "--role",
                "Widget Engineer",
                "--link",
                "https://jobs.example.test/pebble/1",
                "--status",
                "withdrawn",
                "--cv-version",
                "cv-sample-1",
            ],
        ),
        true,
        "appended: https://jobs.example.test/pebble/1 (withdrawn)",
    );
    let after = fs::read(&ledger_path).unwrap();
    assert!(
        after.starts_with(body.as_bytes()),
        "legacy lines were rewritten"
    );

    let conflict = call(
        dir,
        &[
            "apps",
            "--ledger",
            ledger,
            "append",
            "--company",
            "Pebble Sample",
            "--role",
            "Widget Engineer",
            "--link",
            "https://jobs.example.test/pebble/1",
            "--status",
            "rejected",
            "--submit-time",
            "2026-10-20T09:15:00+02:00",
            "--cv-version",
            "cv-sample-1",
        ],
    );
    see(&conflict, false, "idempotency conflict");
    assert_eq!(fs::read(&ledger_path).unwrap(), after);

    let fresh = dir.join("new.jsonl");
    let fresh_s = fresh.to_str().unwrap();
    see(
        &call(
            dir,
            &[
                "apps",
                "--ledger",
                fresh_s,
                "append",
                "--company",
                "Quarry Sample",
                "--role",
                "Widget Engineer",
                "--link",
                "https://jobs.example.test/quarry/2",
                "--status",
                "rejected",
                "--cv-version",
                "cv-sample-1",
            ],
        ),
        false,
        "submit_time is required when status is rejected",
    );
    assert!(!fresh.exists(), "a rejected row without a time was written");
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn legacy_audited_by_warns_strict_fails_and_append_rejects() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let mixed = fix("audit-legacy-audited-by.jsonl");
    let mixed_s = mixed.to_str().unwrap();
    let warn = "warning: line 1: legacy: audited_by missing";

    let ok = call(dir, &["apps", "audit", "validate", mixed_s]);
    assert!(ok.status.success(), "{}", stderr_of(&ok));
    assert!(
        stdout_of(&ok).contains("ok: 2 audits"),
        "{}",
        stdout_of(&ok)
    );
    assert!(stderr_of(&ok).contains(warn), "{}", stderr_of(&ok));
    assert!(
        !stderr_of(&ok).contains("line 3"),
        "the row that has audited_by is not legacy: {}",
        stderr_of(&ok)
    );

    let via_store = call(dir, &["apps", "audit", "--store", mixed_s, "validate"]);
    assert!(via_store.status.success(), "{}", stderr_of(&via_store));
    assert!(
        stderr_of(&via_store).contains(warn),
        "{}",
        stderr_of(&via_store)
    );

    let strict = call(dir, &["apps", "audit", "validate", "--strict", mixed_s]);
    assert_eq!(strict.status.code(), Some(1), "{}", stderr_of(&strict));
    assert!(
        stderr_of(&strict).contains("legacy: audited_by missing"),
        "{}",
        stderr_of(&strict)
    );
    assert!(
        !stdout_of(&strict).contains("ok:"),
        "strict must not report success: {}",
        stdout_of(&strict)
    );

    let now = "2026-10-27T12:00:00+01:00";
    let status = run(cmd(dir)
        .args(["apps", "audit", "--store"])
        .arg(&mixed)
        .args(["status", "northwind.example.test", "--now", now]));
    assert_eq!(status.status.code(), Some(0), "{}", stderr_of(&status));
    assert!(
        stdout_of(&status).contains("\"status\":\"pass\""),
        "{}",
        stdout_of(&status)
    );
    assert!(stderr_of(&status).contains(warn), "{}", stderr_of(&status));

    let other = run(cmd(dir)
        .args(["apps", "audit", "--store"])
        .arg(&mixed)
        .args(["status", "contoso.example.test", "--now", now]));
    assert_eq!(other.status.code(), Some(3), "{}", stderr_of(&other));
    assert!(
        stderr_of(&other).contains(warn),
        "status reports every legacy row, not only the queried one: {}",
        stderr_of(&other)
    );

    let status_strict = run(cmd(dir)
        .args(["apps", "audit", "--store"])
        .arg(&mixed)
        .args(["status", "--strict", "northwind.example.test", "--now", now]));
    assert_eq!(
        status_strict.status.code(),
        Some(1),
        "{}",
        stderr_of(&status_strict)
    );
    assert!(
        stderr_of(&status_strict).contains("legacy: audited_by missing"),
        "{}",
        stderr_of(&status_strict)
    );
    assert!(
        stdout_of(&status_strict).is_empty(),
        "strict status prints no JSON: {}",
        stdout_of(&status_strict)
    );

    let text = fs::read_to_string(&mixed).unwrap();
    let mut file_lines = text.lines();
    let legacy_line = file_lines.next().unwrap().to_string();
    let fresh_line = file_lines.nth(1).unwrap().to_string();
    let legacy_file = put(dir, "legacy-only.jsonl", &legacy_line);
    let fresh_file = put(dir, "fresh-only.jsonl", &fresh_line);
    let empty = dir.join("empty-store.jsonl");
    let empty_s = empty.to_str().unwrap();
    append_to(dir, empty_s, &legacy_file, false, "audited_by");
    assert!(!empty.exists(), "rejected append writes nothing");

    let kept = dir.join("kept.jsonl");
    let body = format!("{legacy_line}\n");
    fs::write(&kept, &body).unwrap();
    let kept_s = kept.to_str().unwrap();
    append_to(dir, kept_s, &fresh_file, true, "appended 1");
    let after = fs::read_to_string(&kept).unwrap();
    assert!(after.starts_with(&body), "old row was rewritten:\n{after}");
    assert!(!after.lines().next().unwrap().contains("audited_by"));

    let clean = dir.join("clean.jsonl");
    fs::write(&clean, format!("{fresh_line}\n")).unwrap();
    let clean_status = run(cmd(dir)
        .args(["apps", "audit", "--store"])
        .arg(&clean)
        .args(["status", "--strict", "contoso.example.test", "--now", now]));
    assert_eq!(
        clean_status.status.code(),
        Some(3),
        "{}",
        stderr_of(&clean_status)
    );
    assert!(
        !stderr_of(&clean_status).contains("legacy:"),
        "{}",
        stderr_of(&clean_status)
    );
    assert!(stdout_of(&clean_status).contains("\"status\":\"flag\""));
}

#[test]
fn adopt_seal_then_append_refuses_the_wrong_hash_an_existing_seal_and_invalid_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let body = slurp("ledger-legacy-submit-time.jsonl");
    let ledger_path = dir.join("ledger.jsonl");
    fs::write(&ledger_path, &body).unwrap();
    let ledger = ledger_path.to_str().unwrap();
    let hex = ensembly_schema::content_sha256(body.as_bytes());
    let wrong = "ab".repeat(32);

    let mismatch = call(
        dir,
        &[
            "apps",
            "adopt-seal",
            "--ledger",
            ledger,
            "--sha256",
            &wrong,
            "--reason",
            "reviewed on laptop-1",
        ],
    );
    see(&mismatch, false, "sha256 does not match");
    assert!(!ensembly_schema::seal_path(&ledger_path).exists());

    let still = call(
        dir,
        &[
            "apps",
            "--ledger",
            ledger,
            "append",
            "--company",
            "Quarry Sample",
            "--role",
            "Widget Engineer",
            "--link",
            "https://jobs.example.test/quarry/9",
            "--status",
            "prepared",
            "--cv-version",
            "cv-sample-1",
        ],
    );
    see(&still, false, "seal is missing");
    assert_eq!(fs::read(&ledger_path).unwrap(), body.as_bytes());

    let bad_path = dir.join("bad.jsonl");
    fs::write(&bad_path, b"{").unwrap();
    let bad = bad_path.to_str().unwrap();
    let bad_hex = ensembly_schema::content_sha256(b"{");
    let invalid = call(
        dir,
        &[
            "apps",
            "adopt-seal",
            "--ledger",
            bad,
            "--sha256",
            &bad_hex,
            "--reason",
            "reviewed on laptop-1",
        ],
    );
    see(&invalid, false, "adopt refused");
    assert!(!ensembly_schema::seal_path(&bad_path).exists());

    let adopted = call(
        dir,
        &[
            "apps",
            "adopt-seal",
            "--ledger",
            ledger,
            "--sha256",
            &hex,
            "--reason",
            "reviewed on laptop-1",
        ],
    );
    see(
        &adopted,
        true,
        &format!("adopted path={ledger} sha256={hex} rows=2"),
    );
    let warned = stderr_of(&adopted);
    assert!(
        warned.contains("warning: line 1: legacy: submit_time missing"),
        "{warned}"
    );
    assert_eq!(fs::read(&ledger_path).unwrap(), body.as_bytes());
    let log = fs::read_to_string(ensembly_schema::adoption_log_path(&ledger_path)).unwrap();
    assert!(
        log.contains(&hex) && log.contains("reviewed on laptop-1"),
        "{log}"
    );

    let again = call(
        dir,
        &[
            "apps",
            "adopt-seal",
            "--ledger",
            ledger,
            "--sha256",
            &hex,
            "--reason",
            "reviewed on laptop-1",
        ],
    );
    see(&again, false, "seal already exists");

    see(
        &call(
            dir,
            &[
                "apps",
                "--ledger",
                ledger,
                "append",
                "--company",
                "Quarry Sample",
                "--role",
                "Widget Engineer",
                "--link",
                "https://jobs.example.test/quarry/9",
                "--status",
                "prepared",
                "--cv-version",
                "cv-sample-1",
            ],
        ),
        true,
        "appended: https://jobs.example.test/quarry/9",
    );

    let audit_body = slurp("audit-legacy-rules.jsonl");
    let audit_path = dir.join("audits.jsonl");
    fs::write(&audit_path, &audit_body).unwrap();
    let audit = audit_path.to_str().unwrap();
    let ok = call(dir, &["apps", "audit", "validate", audit]);
    see(&ok, true, "ok:");
    let audit_warn = stderr_of(&ok);
    assert!(
        audit_warn
            .contains("legacy: check duplicate_ghost: result pass needs at least one https source"),
        "{audit_warn}"
    );
    assert!(audit_warn.contains("warning: line "), "{audit_warn}");
    let strict = call(dir, &["apps", "audit", "validate", "--strict", audit]);
    see(&strict, false, "needs at least one https source");
    assert_eq!(strict.status.code(), Some(1));
    assert!(
        !stdout_of(&strict).contains("ok:"),
        "{}",
        stdout_of(&strict)
    );

    let audit_hex = ensembly_schema::content_sha256(audit_body.as_bytes());
    see(
        &call(
            dir,
            &[
                "apps",
                "adopt-seal",
                "--store",
                audit,
                "--sha256",
                &audit_hex,
                "--reason",
                "reviewed on laptop-1",
            ],
        ),
        true,
        &format!("adopted path={audit} sha256={audit_hex} rows="),
    );
    let second = call(
        dir,
        &[
            "apps",
            "adopt-seal",
            "--store",
            audit,
            "--sha256",
            &audit_hex,
            "--reason",
            "reviewed on laptop-1",
        ],
    );
    see(&second, false, "seal already exists");
}
