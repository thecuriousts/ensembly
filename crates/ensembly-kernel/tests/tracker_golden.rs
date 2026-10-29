//! Golden test: `apps export --tracker` output is byte-equal to the file the
//! Python generator (ops gen_tracker.py) wrote for the same synthetic inputs.
use ensembly_kernel::application_ledger as al;
use ensembly_kernel::tracker;

const LEDGER: &str = include_str!("../../../fixtures/tracker/ledger.jsonl");
const HAND: &str = include_str!("../../../fixtures/tracker/hand-tracker.md");
const EXPECTED: &str = include_str!("../../../fixtures/tracker/TRACKER.expected.md");
const STAMP: &str = "2026-10-29 21:30";

#[test]
fn tracker_is_byte_equal_to_the_python_generator() {
    let rows = al::parse_ledger(LEDGER).unwrap();
    let join = tracker::parse_side(HAND, tracker::SideFormat::Markdown).unwrap();
    let t = tracker::build(&rows, &join, &[]);
    assert_eq!(tracker::render(&t, STAMP), EXPECTED);
    assert_eq!(t.ledger.len(), 7, "empty build must fail");
    assert_eq!(t.prepared.len(), 4);
}

#[test]
fn cli_writes_the_same_bytes_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = dir.path().join("ledger.jsonl");
    let hand = dir.path().join("hand.md");
    std::fs::write(&ledger, LEDGER).unwrap();
    std::fs::write(&hand, HAND).unwrap();
    let out = dir.path().join("out");
    let st = std::process::Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .arg("apps")
        .arg("--ledger")
        .arg(&ledger)
        .args(["export", "--tracker", "--stamp", STAMP, "--join"])
        .arg(&hand)
        .arg("--out-dir")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "{}",
        String::from_utf8_lossy(&st.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(out.join("TRACKER.md")).unwrap(),
        EXPECTED
    );
    let names: Vec<_> = std::fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(
        names,
        ["TRACKER.md"],
        "no temp files left, no other exports"
    );
    // inputs untouched
    assert_eq!(std::fs::read_to_string(&ledger).unwrap(), LEDGER);
    assert_eq!(std::fs::read_to_string(&hand).unwrap(), HAND);
}
