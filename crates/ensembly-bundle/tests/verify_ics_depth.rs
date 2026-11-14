use ensembly_bundle::{
    verify_ics_content, verify_ics_file, VERIFY_ICS_MAX_BYTES, VERIFY_ICS_MAX_LOGICAL_LINE_BYTES,
};
use std::fs;
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn verify_rejects_unclosed_vevent() {
    let bad = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nSUMMARY:x\r\nEND:VCALENDAR\r\n";
    let err = verify_ics_content(bad).unwrap_err();
    assert!(err.contains("VEVENT"), "{err}");
}

#[test]
fn verify_accepts_minimal_valid_calendar() {
    let ok = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//t//\r\nBEGIN:VEVENT\r\nUID:1\r\nDTSTAMP:20260101T000000Z\r\nSUMMARY:x\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    verify_ics_content(ok).unwrap();
}

#[test]
fn verify_rejects_deeply_nested_begin_blocks() {
    let mut nested = String::from("BEGIN:VCALENDAR\nVERSION:2.0\n");
    for i in 0..80 {
        nested.push_str(&format!("BEGIN:PART{i}\n"));
    }
    for i in (0..80).rev() {
        nested.push_str(&format!("END:PART{i}\n"));
    }
    nested.push_str("END:VCALENDAR\n");
    let err = verify_ics_content(&nested).unwrap_err();
    assert!(err.contains("nesting") || err.contains("deep"), "{err}");
}

#[test]
fn verify_rejects_multi_mb_logical_line() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("huge-line.ics");
    let mut f = File::create(&path).unwrap();
    f.write_all(b"BEGIN:VCALENDAR\nVERSION:2.0\nSUMMARY:")
        .unwrap();
    let chunk = vec![b'x'; 512 * 1024];
    for _ in 0..((VERIFY_ICS_MAX_LOGICAL_LINE_BYTES / chunk.len()) + 2) {
        f.write_all(&chunk).unwrap();
    }
    f.write_all(b"\nEND:VCALENDAR\n").unwrap();
    let err = verify_ics_file(&path).unwrap_err();
    assert!(
        err.contains("line exceeds") || err.contains("byte limit"),
        "{err}"
    );
}

#[test]
fn verify_rejects_bad_folding() {
    let bad = " BEGIN:VCALENDAR\nVERSION:2.0\nEND:VCALENDAR\n";
    let err = verify_ics_content(bad).unwrap_err();
    assert!(err.contains("fold") || err.contains("preceding"), "{err}");
}

#[test]
fn verify_rejects_huge_file_without_reading_all() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("huge.ics");
    let mut f = File::create(&path).unwrap();
    f.write_all(b"BEGIN:VCALENDAR\nVERSION:2.0\n").unwrap();
    let pad = vec![b' '; VERIFY_ICS_MAX_BYTES as usize + 1024];
    f.write_all(&pad).unwrap();
    f.write_all(b"\nEND:VCALENDAR\n").unwrap();
    let err = verify_ics_file(&path).unwrap_err();
    assert!(
        err.contains("too large") || err.contains("byte limit"),
        "{err}"
    );
}

#[test]
fn verify_ics_file_accepts_on_disk_minimal() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("ok.ics");
    fs::write(&path, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n").unwrap();
    verify_ics_file(&path).unwrap();
}
