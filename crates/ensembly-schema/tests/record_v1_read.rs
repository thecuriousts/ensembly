use ensembly_schema::snapshot_json;
use std::path::PathBuf;

#[test]
fn v1_only_pre_slice2_log_still_reads() {
    let dir = std::env::temp_dir().join(format!(
        "v1-read-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let log: PathBuf = dir.join("events.jsonl");
    std::fs::write(
        &log,
        include_str!("../fixtures/scenarios/pre-slice2-a-happy.jsonl"),
    )
    .unwrap();
    let snap = snapshot_json(&log, None).unwrap();
    assert_eq!(snap.facts.len(), 2);
}