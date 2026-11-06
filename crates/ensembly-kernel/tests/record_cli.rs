//! record commit, recover, and view.

use ensembly_schema::{
    commit_json, parse_event_log, replay, view_append, CommitFault, CommitPhase, JsonPayload,
    RecordView, Snapshot, UnitOfWork, Version, ViewContract, ViewError,
};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn scenario(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ensembly-schema/fixtures/scenarios")
        .join(name);
    fs::read_to_string(path).unwrap()
}

fn kernel_bin(dir: &Path) -> Command {
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

fn stdout_ok(out: &Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "{stdout}{stderr}");
    stdout
}

fn nonempty_lines(path: &Path) -> usize {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

fn dir_has_partial_jsonl_line(dir: &Path) -> bool {
    for ent in fs::read_dir(dir).unwrap() {
        let path = ent.unwrap().path();
        if !path.is_file() {
            continue;
        }
        let Ok(body) = fs::read_to_string(&path) else {
            continue;
        };
        if body.is_empty() {
            continue;
        }
        if body.contains('\n') {
            for line in body.split('\n') {
                if line.trim().is_empty() {
                    continue;
                }
                if serde_json::from_str::<serde_json::Value>(line).is_err() {
                    return true;
                }
            }
        } else if serde_json::from_str::<serde_json::Value>(&body).is_err() {
            return true;
        }
    }
    false
}

#[test]
fn scenario_a_cli_commit_snapshot_equals_replay_and_rejects_same_key() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("events.jsonl");
    let unit = scenario("a-happy-events.json");
    let committed = stdout_ok(&run(kernel_bin(dir.path())
        .args(["record", "commit", "--log"])
        .arg(&log)
        .arg("--unit")
        .arg(&unit)));
    let phase: serde_json::Value = serde_json::from_str(&committed).unwrap();
    assert_eq!(phase["phase"], "done");

    let snap_out = stdout_ok(&run(kernel_bin(dir.path())
        .args(["record", "view", "snapshot", "--log"])
        .arg(&log)));
    let snap: Snapshot = serde_json::from_str(&snap_out).unwrap();
    let text = fs::read_to_string(&log).unwrap();
    let events = parse_event_log::<JsonPayload>(&text).unwrap();
    let rows = replay(&events).unwrap();
    let via = RecordView::from_events(events)
        .snapshot_at(snap.at)
        .unwrap();
    assert_eq!(snap, via);
    assert_eq!(snap.facts.len(), rows.len());
    for (fact, row) in snap.facts.iter().zip(rows.iter()) {
        assert_eq!(fact.stable_id, row.aggregate);
        assert_eq!(fact.version, row.version);
    }

    let again = run(kernel_bin(dir.path())
        .args(["record", "commit", "--log"])
        .arg(&log)
        .arg("--unit")
        .arg(&unit));
    assert!(!again.status.success());
    let err = String::from_utf8_lossy(&again.stderr);
    assert!(err.contains("idempotency"), "{err}");
    assert_eq!(nonempty_lines(&log), 3);

    let before = fs::read(&log).unwrap();
    let at_one = stdout_ok(&run(kernel_bin(dir.path())
        .args(["record", "view", "snapshot", "--log"])
        .arg(&log)
        .args(["--at", "1"])));
    let snap_one: Snapshot = serde_json::from_str(&at_one).unwrap();
    assert_eq!(snap_one.at, Version(1));
    let feed_out = stdout_ok(&run(kernel_bin(dir.path())
        .args(["record", "view", "feed", "--log"])
        .arg(&log)
        .args(["--since", "1"])));
    let feed: serde_json::Value = serde_json::from_str(&feed_out).unwrap();
    assert_eq!(feed["since"], 1);
    let events = feed["events"].as_array().unwrap();
    assert!(events.iter().all(|e| e["version"].as_u64().unwrap() > 1));
    assert_eq!(fs::read(&log).unwrap(), before);
}

#[test]
fn scenario_d_crash_mid_append_and_cli_recover() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("events.jsonl");
    let unit = scenario("d-crash-unit.json");
    let abort = CommitFault {
        append_abort_after_bytes: Some(10),
        ..Default::default()
    };
    assert!(commit_json(&log, &unit, &abort).is_err());
    assert!(!log.exists());
    assert!(!dir_has_partial_jsonl_line(dir.path()));

    let done = stdout_ok(&run(kernel_bin(dir.path())
        .args(["record", "commit", "--log"])
        .arg(&log)
        .arg("--unit")
        .arg(&unit)));
    assert!(done.contains("done"));
    assert_eq!(nonempty_lines(&log), 2);

    let dir_v = tempfile::tempdir().unwrap();
    let log_v = dir_v.path().join("events.jsonl");
    let fault = CommitFault {
        crash_entering: Some(CommitPhase::Verify),
        ..Default::default()
    };
    let mid = commit_json(&log_v, &unit, &fault).unwrap();
    assert_eq!(mid.phase, CommitPhase::Verify);
    let recovered = stdout_ok(&run(kernel_bin(dir_v.path())
        .args(["record", "recover", "--log"])
        .arg(&log_v)));
    let phase: serde_json::Value = serde_json::from_str(&recovered).unwrap();
    assert_eq!(phase["phase"], "done");
    assert_eq!(nonempty_lines(&log_v), 2);

    let text = fs::read_to_string(&log_v).unwrap();
    let events = parse_event_log::<JsonPayload>(&text).unwrap();
    let view = RecordView::from_events(events.clone());
    assert_eq!(
        view_append(
            &view,
            UnitOfWork {
                events: events.clone()
            }
        ),
        Err(ViewError::ReadOnly)
    );
    assert_eq!(fs::read_to_string(&log_v).unwrap(), text);
}
