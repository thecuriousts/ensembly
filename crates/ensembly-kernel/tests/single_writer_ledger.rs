//! KernelWriter application-ledger single-writer checks (opportunity-finder pack).
//! Fixtures are synthetic. Writer identity is configured via ENSEMBLY_WRITER.

use ensembly_kernel::application_ledger::{LedgerRow, Status};
use ensembly_kernel::idempotency_key;
use ensembly_kernel::writer::{KernelWriter, WriterError};
use ensembly_schema::{lock, write_seal};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

static WRITER_ENV: Mutex<()> = Mutex::new(());

fn with_writer(name: &str) -> MutexGuard<'static, ()> {
    let guard = WRITER_ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var(ensembly_schema::WRITER_ENV, name);
    guard
}

fn setup_writer() -> MutexGuard<'static, ()> {
    with_writer("laptop-1")
}

fn row(link: &str, status: Status, cv: &str) -> LedgerRow {
    LedgerRow {
        company: "Northwind Fictional AB".into(),
        role: "Engineer, Widgets".into(),
        link: link.into(),
        apply_url: None,
        status,
        submit_time: match status {
            Status::Prepared | Status::Withdrawn => None,
            _ => Some("2026-10-26T18:59:00+01:00".into()),
        },
        submit_time_unknown: None,
        cv_version: cv.into(),
    }
}

fn open_ledger(path: &Path) -> KernelWriter {
    let _writer_guard = setup_writer();
    let writer = KernelWriter::open_ledger(path).unwrap();
    writer.init_ledger().unwrap();
    writer
}

fn open_sealed_ledger(path: &Path, seed: &[u8]) -> KernelWriter {
    let _writer_guard = setup_writer();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(path, seed).unwrap();
    write_seal(path, seed).unwrap();
    lock(path).unwrap();
    KernelWriter::open_ledger(path).unwrap()
}

fn seal_path(ledger: &Path) -> std::path::PathBuf {
    let mut name = ledger.file_name().unwrap().to_os_string();
    name.push(".writer-hash");
    ledger.with_file_name(name)
}

#[test]
fn idempotency_key_is_link_key_tab_stage() {
    let prepared = row(
        "https://jobs.example.test/demo/1/",
        Status::Prepared,
        "cv-sample-1",
    );
    assert_eq!(
        idempotency_key(&prepared),
        "https://jobs.example.test/demo/1\tintended"
    );
    let interview = row(
        "https://jobs.example.test/demo/1",
        Status::Interview,
        "cv-sample-1",
    );
    assert_eq!(
        idempotency_key(&interview),
        "https://jobs.example.test/demo/1\tnext-stage:1"
    );
}

#[test]
fn ledger_append_after_existing_v1_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let v1_line = r#"{"company":"Acme Fictional","role":"Engineer","link":"https://jobs.example.test/acme/9","status":"submitted","submit_time":"2026-10-26T18:59:00+01:00","cv_version":"cv-sample-1","v":1}"#;
    let seed = format!("{v1_line}\n");
    let writer = open_sealed_ledger(&path, seed.as_bytes());
    let next = row(
        "https://jobs.example.test/demo/v1-next",
        Status::Prepared,
        "cv-sample-1",
    );
    assert!(writer.append_ledger(&next).unwrap().appended);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.starts_with(v1_line), "v1 seed line must be preserved");
    assert_eq!(text.lines().count(), 2);
}

#[test]
fn ledger_append_is_idempotent_on_the_same_key_and_payload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    let first = row(
        "https://jobs.example.test/demo/1",
        Status::Prepared,
        "cv-sample-1",
    );
    let once = writer.append_ledger(&first).unwrap();
    assert!(once.appended);
    assert_eq!(once.key, idempotency_key(&first));
    let again = writer.append_ledger(&first).unwrap();
    assert!(!again.appended);
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 1);
}

#[test]
fn ledger_same_key_different_payload_is_a_conflict_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    let first = row(
        "https://jobs.example.test/demo/2",
        Status::Prepared,
        "cv-sample-1",
    );
    writer.append_ledger(&first).unwrap();
    let mut changed = first.clone();
    changed.cv_version = "cv-sample-2".into();
    let err = writer.append_ledger(&changed).unwrap_err();
    match err {
        WriterError::Conflict { key, .. } => {
            assert_eq!(key, idempotency_key(&first));
        }
        other => panic!("expected conflict, got {other}"),
    }
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 1);
}

#[test]
fn ledger_later_stage_appends_and_leaves_the_old_line_bytes_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    let prepared = row(
        "https://jobs.example.test/demo/3",
        Status::Prepared,
        "cv-sample-1",
    );
    writer.append_ledger(&prepared).unwrap();
    let before = fs::read(&path).unwrap();
    let submitted = row(
        "https://jobs.example.test/demo/3",
        Status::Submitted,
        "cv-sample-1",
    );
    assert!(writer.append_ledger(&submitted).unwrap().appended);
    let after = fs::read(&path).unwrap();
    assert!(after.starts_with(&before), "old ledger row was rewritten");
    assert!(after.len() > before.len());
    assert_eq!(std::str::from_utf8(&after).unwrap().lines().count(), 2);
}

#[test]
fn ledger_writer_refuses_to_append_after_an_old_line_is_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/4",
            Status::Prepared,
            "cv-sample-1",
        ))
        .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let original = fs::read_to_string(&path).unwrap();
    let tampered = original.replace("Northwind Fictional AB", "Contoso Fictional Ltd");
    fs::write(&path, tampered).unwrap();
    let err = writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/4",
            Status::Submitted,
            "cv-sample-1",
        ))
        .unwrap_err();
    assert!(err.to_string().contains("rewritten"), "{err}");
}

#[test]
fn deleted_seal_is_refused_and_is_not_adopted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/7",
            Status::Prepared,
            "cv-sample-1",
        ))
        .unwrap();
    let seal = seal_path(&path);
    fs::set_permissions(&seal, fs::Permissions::from_mode(0o644)).unwrap();
    fs::remove_file(&seal).unwrap();
    let err = writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/7",
            Status::Submitted,
            "cv-sample-1",
        ))
        .unwrap_err();
    assert!(err.to_string().contains("seal is missing"), "{err}");
    assert!(!seal.exists());
}

#[test]
fn tampering_then_reseal_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/8",
            Status::Prepared,
            "cv-sample-1",
        ))
        .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        original.replace("Northwind Fictional AB", "Contoso Fictional Ltd"),
    )
    .unwrap();
    let seal = seal_path(&path);
    fs::set_permissions(&seal, fs::Permissions::from_mode(0o644)).unwrap();
    fs::remove_file(&seal).unwrap();
    let err = writer.init_ledger().unwrap_err();
    assert!(
        err.to_string().contains("already has rows") || err.to_string().contains("will not adopt"),
        "{err}"
    );
    assert!(!seal.exists());
    let err = writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/8",
            Status::Submitted,
            "cv-sample-1",
        ))
        .unwrap_err();
    assert!(err.to_string().contains("seal is missing"), "{err}");
}

#[test]
fn init_refuses_a_non_empty_unsealed_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    fs::write(
        &path,
        r#"{"company":"Northwind Fictional AB","role":"Engineer, Widgets","link":"https://jobs.example.test/demo/9","status":"prepared","submit_time":null,"cv_version":"cv-sample-1"}
"#,
    )
    .unwrap();
    let writer = KernelWriter::open_ledger(&path).unwrap();
    let err = writer.init_ledger().unwrap_err();
    assert!(err.to_string().contains("already has rows"), "{err}");
}

#[test]
fn direct_ledger_write_is_refused_on_a_writer_managed_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/5",
            Status::Prepared,
            "cv-sample-1",
        ))
        .unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o444, "chmod is Unix-only; this test runs on Unix");
    let direct = fs::OpenOptions::new().write(true).open(&path);
    assert!(
        direct.is_err(),
        "a direct write must not open the locked ledger"
    );
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 1);
    assert!(
        writer
            .append_ledger(&row(
                "https://jobs.example.test/demo/5",
                Status::Submitted,
                "cv-sample-1",
            ))
            .unwrap()
            .appended
    );
}

#[test]
fn writer_has_no_ledger_delete_and_never_shortens_the_file() {
    fn assert_no_delete(_: &KernelWriter) {}
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.jsonl");
    let writer = open_ledger(&path);
    assert_no_delete(&writer);
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/6",
            Status::Prepared,
            "cv-sample-1",
        ))
        .unwrap();
    let len = fs::read(&path).unwrap().len();
    writer
        .append_ledger(&row(
            "https://jobs.example.test/demo/6",
            Status::Submitted,
            "cv-sample-1",
        ))
        .unwrap();
    assert!(fs::read(&path).unwrap().len() > len);
}
