use ensembly_bundle::restore_file_with_merge;
use std::fs;
use tempfile::tempdir;

#[test]
fn jsonl_merge_keeps_both_ids_and_sidecars_conflict() {
    let dir = tempdir().unwrap();
    let dest = dir.path().join("ledger.jsonl");
    fs::write(
        &dest,
        r#"{"id":"a","v":1}
{"id":"b","v":1}
"#,
    )
    .unwrap();
    let src = dir.path().join("incoming.jsonl");
    fs::write(
        &src,
        r#"{"id":"b","v":2}
{"id":"c","v":1}
"#,
    )
    .unwrap();
    let sidecars = restore_file_with_merge(&src, &dest, true).unwrap();
    let body = fs::read_to_string(&dest).unwrap();
    assert!(body.contains("\"id\":\"a\""));
    assert!(body.contains("\"id\":\"b\",\"v\":1"));
    assert!(body.contains("\"id\":\"c\""));
    assert!(!body.contains("\"v\":2"));
    assert_eq!(sidecars.len(), 1);
    let conflict = fs::read_to_string(&sidecars[0]).unwrap();
    assert!(conflict.contains("\"v\":2"));
}

#[test]
fn merge_keeps_existing_file_when_content_differs() {
    let dir = tempdir().unwrap();
    let dest = dir.path().join("note.txt");
    fs::write(&dest, b"original").unwrap();
    let src = dir.path().join("new.txt");
    fs::write(&src, b"incoming").unwrap();
    let sidecars = restore_file_with_merge(&src, &dest, true).unwrap();
    assert_eq!(fs::read(&dest).unwrap(), b"original");
    assert_eq!(sidecars.len(), 1);
    assert_eq!(fs::read(&sidecars[0]).unwrap(), b"incoming");
}
