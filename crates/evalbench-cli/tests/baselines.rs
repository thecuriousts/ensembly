//! Committed baselines re-score identically (handoff section 15). Offline replay only.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tasks() -> PathBuf {
    here().join("../evalbench-core/fixtures/tasks")
}

fn evalbench(args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_evalbench"))
        .args(args)
        .output()
        .expect("run evalbench")
}

fn baseline_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(here().join("baselines"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn committed_baselines_replay_and_rescore_identically() {
    let dirs = baseline_dirs();
    assert!(!dirs.is_empty(), "no committed baseline");
    for dir in dirs {
        let run_id = dir.file_name().unwrap().to_owned();
        for f in ["traces.jsonl", "replies.jsonl", "score.json", "run.md"] {
            assert!(dir.join(f).is_file(), "{} lacks {f}", dir.display());
        }
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("traces.jsonl");
        let run = evalbench(&[
            "run".as_ref(),
            "--tasks".as_ref(),
            tasks().as_os_str(),
            "--run-id".as_ref(),
            run_id.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
            "--replay".as_ref(),
            dir.join("replies.jsonl").as_os_str(),
        ]);
        assert_eq!(run.status.code(), Some(0), "{run:?}");
        assert_eq!(
            read(&out),
            read(&dir.join("traces.jsonl")),
            "{}",
            dir.display()
        );

        let traces = dir.join("traces.jsonl");
        let score = |_: u8| {
            let o = evalbench(&[
                "score".as_ref(),
                "--tasks".as_ref(),
                tasks().as_os_str(),
                traces.as_os_str(),
            ]);
            assert_eq!(o.status.code(), Some(0), "{o:?}");
            o.stdout
        };
        let (a, b) = (score(1), score(2));
        assert_eq!(a, b, "two scorings differ: {}", dir.display());
        assert_eq!(
            String::from_utf8(a).unwrap(),
            read(&dir.join("score.json")),
            "{}",
            dir.display()
        );
    }
}

#[test]
fn replay_baseline_uses_the_frozen_replay_fixture() {
    let core = here().join("../evalbench-core/fixtures/replay");
    let base = here().join("baselines/replay-001");
    assert_eq!(
        read(&base.join("replies.jsonl")),
        read(&core.join("replies.jsonl"))
    );
    assert_eq!(
        read(&base.join("score.json")),
        format!("{}\n", read(&core.join("score.json")))
    );
}
