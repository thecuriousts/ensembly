//! v0 and v1 through `apps validate`, `apps audit validate`, and `apps audit append`.
//! v2 is refused. Unknown status, duplicate keys, and wrong case stay refused.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fix(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ensembly-schema/fixtures")
        .join(name)
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .arg("--no-memory")
        .arg("--db")
        .arg(dir.join("ops.sqlite"))
        .args(args)
        .output()
        .expect("run ensembly")
}

fn expect(out: &Output, ok: bool, needle: &str) {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.success(), ok, "{stdout}{stderr}");
    let msg = if ok { stdout } else { stderr };
    assert!(msg.contains(needle), "{msg}");
}

fn write(dir: &Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    fs::write(&path, body).expect("write fixture");
    path.to_str().expect("utf8 path").to_string()
}

fn line_of(name: &str) -> String {
    let mut line = fs::read_to_string(fix(name))
        .expect("fixture")
        .lines()
        .next()
        .expect("line")
        .to_string();
    line.push('\n');
    line
}

#[test]
fn versioned_lines_and_old_refusals() {
    let tmp = tempfile::tempdir().expect("temp");
    let dir = tmp.path();
    accept_v0_and_v1(dir);
    refuse_v2(dir);
    old_refusals(dir);
}

fn accept_v0_and_v1(dir: &Path) {
    for name in ["ledger-v0.jsonl", "ledger-v1.jsonl"] {
        let path = fix(name);
        expect(
            &run(
                dir,
                &["apps", "--ledger", path.to_str().unwrap(), "validate"],
            ),
            true,
            "ok: 6 rows, 6 current applications",
        );
    }
    for name in ["audit-v0.jsonl", "audit-v1.jsonl"] {
        let path = fix(name);
        expect(
            &run(dir, &["apps", "audit", "validate", path.to_str().unwrap()]),
            true,
            "ok: 4 audits, 4 companies",
        );
    }
    let store = dir.join("audits.jsonl");
    let store_s = store.to_str().unwrap();
    let v0 = write(dir, "one-v0.jsonl", &line_of("audit-v0.jsonl"));
    expect(
        &run(dir, &["apps", "audit", "--store", store_s, "append", &v0]),
        true,
        "appended 1, skipped 0 already stored",
    );
    let v1 = write(dir, "one-v1.jsonl", &line_of("audit-v1.jsonl"));
    expect(
        &run(dir, &["apps", "audit", "--store", store_s, "append", &v1]),
        true,
        "appended 0, skipped 1 already stored",
    );
}

fn refuse_v2(dir: &Path) {
    let ledger = write(
        dir,
        "ledger-v2.jsonl",
        &fs::read_to_string(fix("ledger-v1.jsonl"))
            .expect("ledger")
            .replace("\"v\":1", "\"v\":2"),
    );
    expect(
        &run(dir, &["apps", "--ledger", &ledger, "validate"]),
        false,
        "unsupported v 2",
    );
    let audit = write(
        dir,
        "audit-v2.jsonl",
        &fs::read_to_string(fix("audit-v1.jsonl"))
            .expect("audit")
            .replace("\"v\":1", "\"v\":2"),
    );
    expect(
        &run(dir, &["apps", "audit", "validate", &audit]),
        false,
        "unsupported v 2",
    );
    let store = dir.join("v2-store.jsonl");
    expect(
        &run(
            dir,
            &[
                "apps",
                "audit",
                "--store",
                store.to_str().unwrap(),
                "append",
                &audit,
            ],
        ),
        false,
        "unsupported v 2",
    );
    assert!(!store.exists());
}

fn old_refusals(dir: &Path) {
    let row = r#"{"company":"Acme Fictional","role":"Widget Engineer","link":"https://jobs.example.test/acme/1","status":"prepared","submit_time":null,"cv_version":"cv-sample-1"}"#;
    let hired = write(
        dir,
        "hired.jsonl",
        &row.replace("\"status\":\"prepared\"", "\"status\":\"hired\""),
    );
    expect(
        &run(dir, &["apps", "--ledger", &hired, "validate"]),
        false,
        "unknown variant",
    );
    let wrong = write(
        dir,
        "case.jsonl",
        &row.replace("\"status\":\"prepared\"", "\"status\":\"Prepared\""),
    );
    expect(
        &run(dir, &["apps", "--ledger", &wrong, "validate"]),
        false,
        "unknown variant",
    );
    let dup = write(
        dir,
        "dup.jsonl",
        &row.replacen(
            "\"company\":",
            "\"company\":\"Bramble Fictional\",\"company\":",
            1,
        ),
    );
    expect(
        &run(dir, &["apps", "--ledger", &dup, "validate"]),
        false,
        "duplicate key",
    );
    let audit_dup = write(
        dir,
        "audit-dup.jsonl",
        &line_of("audit-v0.jsonl").replacen(
            "\"domain\":",
            "\"domain\":\"other.example.test\",\"domain\":",
            1,
        ),
    );
    expect(
        &run(dir, &["apps", "audit", "validate", &audit_dup]),
        false,
        "duplicate key",
    );
}
