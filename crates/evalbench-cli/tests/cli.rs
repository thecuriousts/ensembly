//! `evalbench` CLI on the shipped path. Exit 0 ok, 1 finding, 2 usage/parse/validation.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn core_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../evalbench-core/fixtures")
}

fn tasks() -> PathBuf {
    core_fixtures().join("tasks")
}

fn script(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn evalbench(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_evalbench"))
        .args(args)
        .output()
        .expect("run evalbench")
}

fn p(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn committed_hash() -> String {
    fs::read_to_string(core_fixtures().join("tasks.sha256")).unwrap()
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let src = e.unwrap().path();
        let dst = to.join(src.file_name().unwrap());
        if src.is_dir() {
            copy_dir(&src, &dst);
        } else {
            fs::copy(&src, &dst).unwrap();
        }
    }
}

#[test]
fn hash_prints_committed_hash() {
    let o = evalbench(&["hash", p(&tasks())]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert_eq!(stdout(&o), committed_hash());
    let v = evalbench(&[
        "hash",
        p(&tasks()),
        "--verify",
        p(&core_fixtures().join("tasks.sha256")),
    ]);
    assert_eq!(v.status.code(), Some(0), "{v:?}");
}

#[test]
fn hash_verify_mismatch_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    let wrong = dir.path().join("wrong.sha256");
    fs::write(&wrong, format!("{}\n", "0".repeat(64))).unwrap();
    let o = evalbench(&["hash", p(&tasks()), "--verify", p(&wrong)]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
}

#[test]
fn hash_rejects_stray_file_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("tasks");
    copy_dir(&tasks(), &copy);
    fs::write(copy.join("t01-full/.DS_Store"), b"x").unwrap();
    let o = evalbench(&["hash", p(&copy)]);
    assert_eq!(o.status.code(), Some(2), "{o:?}");
    assert!(
        String::from_utf8_lossy(&o.stderr).contains(".DS_Store"),
        "{o:?}"
    );
}

#[test]
fn check_exit_codes_and_json() {
    let t01 = tasks().join("t01-full");
    let o = evalbench(&[
        "check",
        p(&t01.join("posting.json")),
        p(&t01.join("pack.json")),
    ]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert_eq!(stdout(&o), "{\"gaps\":[]}\n");

    let t02 = tasks().join("t02-gappy");
    let o = evalbench(&[
        "check",
        p(&t02.join("posting.json")),
        p(&t02.join("pack.json")),
    ]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    assert_eq!(
        stdout(&o),
        "{\"gaps\":[\"LLM evals\",\"RAG\",\"Observability\"]}\n"
    );
}

#[test]
fn check_matches_python_on_every_fixture() {
    let reference = core_fixtures().join("../tests/reference/check.py");
    let mut n = 0;
    for root in ["tasks", "parity"] {
        let mut dirs: Vec<PathBuf> = fs::read_dir(core_fixtures().join(root))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        dirs.sort();
        for d in dirs {
            let (post, pack) = (d.join("posting.json"), d.join("pack.json"));
            let py = Command::new("python3")
                .args(["-I", "-X", "utf8"])
                .arg(&reference)
                .args([&post, &pack])
                .output()
                .expect("python3 not found: differential test cannot run");
            let rs = evalbench(&["check", p(&post), p(&pack)]);
            let a: serde_json::Value = serde_json::from_slice(&py.stdout).unwrap();
            let b: serde_json::Value = serde_json::from_slice(&rs.stdout)
                .unwrap_or_else(|e| panic!("{}: {e} {rs:?}", d.display()));
            assert_eq!(a, b, "{}", d.display());
            assert_eq!(py.status.code(), rs.status.code(), "{}", d.display());
            n += 1;
        }
    }
    assert!(n >= 18);
}

#[test]
fn check_bad_json_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.json");
    fs::write(&bad, "{not json").unwrap();
    let t01 = tasks().join("t01-full");
    let o = evalbench(&["check", p(&t01.join("posting.json")), p(&bad)]);
    assert_eq!(o.status.code(), Some(2), "{o:?}");
}

#[test]
fn score_prints_golden_bytes_twice() {
    let traces = core_fixtures().join("replay/traces.jsonl");
    let golden = fs::read_to_string(core_fixtures().join("replay/score.json")).unwrap();
    let a = evalbench(&["score", "--tasks", p(&tasks()), p(&traces)]);
    let b = evalbench(&["score", "--tasks", p(&tasks()), p(&traces)]);
    assert_eq!(a.status.code(), Some(0), "{a:?}");
    assert_eq!(stdout(&a), format!("{golden}\n"));
    assert_eq!(a.stdout, b.stdout);
}

#[test]
fn score_mixed_runs_exits_2_unless_run_id() {
    let dir = tempfile::tempdir().unwrap();
    let golden = fs::read_to_string(core_fixtures().join("replay/traces.jsonl")).unwrap();
    let mixed = dir.path().join("mixed.jsonl");
    fs::write(
        &mixed,
        format!("{golden}{}", golden.replace("replay-001", "replay-002")),
    )
    .unwrap();
    let o = evalbench(&["score", "--tasks", p(&tasks()), p(&mixed)]);
    assert_eq!(o.status.code(), Some(2), "{o:?}");
    let o = evalbench(&[
        "score",
        "--tasks",
        p(&tasks()),
        "--run-id",
        "replay-002",
        p(&mixed),
    ]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert!(stdout(&o).contains("\"run_id\":\"replay-002\""));
    assert!(stdout(&o).contains("\"score\":58"));
}

#[test]
fn record_appends_checker_judged_rows() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("traces.jsonl");
    let right = dir.path().join("right.txt");
    let wrong = dir.path().join("wrong.txt");
    fs::write(&right, "{\"gaps\":[]}").unwrap();
    fs::write(&wrong, "{\"gaps\":[\"RAG\"]}").unwrap();
    let rec = |task: &str, answer: &Path| {
        evalbench(&[
            "record",
            "--tasks",
            p(&tasks()),
            "--out",
            p(&out),
            "--run-id",
            "r1",
            "--task-id",
            task,
            "--attempt",
            "1",
            "--model-id",
            "synthetic-model",
            "--tokens-in",
            "10",
            "--tokens-out",
            "2",
            "--cost-micro-usd",
            "5",
            "--answer",
            p(answer),
        ])
    };
    assert_eq!(rec("t01-full", &right).status.code(), Some(0));
    assert_eq!(rec("t09-data-full", &wrong).status.code(), Some(0));
    let text = fs::read_to_string(&out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"pass\":true") && lines[1].contains("\"pass\":false"));
    assert!(lines[0].contains(committed_hash().trim()));

    assert_eq!(
        rec("t01-full", &right).status.code(),
        Some(2),
        "duplicate (run, task, attempt)"
    );
    let with_pass = evalbench(&["record", "--pass", "true"]);
    assert_eq!(
        with_pass.status.code(),
        Some(2),
        "no --pass flag: the checker judges (D16)"
    );
}

#[test]
fn run_replay_writes_golden_traces() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("traces.jsonl");
    let replies = core_fixtures().join("replay/replies.jsonl");
    let o = evalbench(&[
        "run",
        "--tasks",
        p(&tasks()),
        "--run-id",
        "replay-001",
        "--out",
        p(&out),
        "--replay",
        p(&replies),
    ]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert_eq!(
        fs::read_to_string(&out).unwrap(),
        fs::read_to_string(core_fixtures().join("replay/traces.jsonl")).unwrap()
    );
}

#[test]
fn run_refuses_existing_out() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("traces.jsonl");
    fs::write(&out, "keep me\n").unwrap();
    let replies = core_fixtures().join("replay/replies.jsonl");
    let o = evalbench(&[
        "run",
        "--tasks",
        p(&tasks()),
        "--run-id",
        "r1",
        "--out",
        p(&out),
        "--replay",
        p(&replies),
    ]);
    assert_eq!(o.status.code(), Some(2), "{o:?}");
    assert_eq!(fs::read_to_string(&out).unwrap(), "keep me\n");
}

#[cfg(unix)]
#[test]
fn run_exec_uses_command_client_offline() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("traces.jsonl");
    let replies_out = dir.path().join("replies.jsonl");
    let o = evalbench(&[
        "run",
        "--tasks",
        p(&tasks()),
        "--run-id",
        "exec-001",
        "--out",
        p(&out),
        "--exec",
        "/bin/sh",
        "--arg",
        p(&script("fake-model.sh")),
        "--replies-out",
        p(&replies_out),
    ]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert_eq!(fs::read_to_string(&out).unwrap().lines().count(), 12);
    assert_eq!(
        fs::read_to_string(&replies_out).unwrap().lines().count(),
        12
    );
    // Always-empty answer passes t01, t09, t11 only: 3/12 = 25, median 101.
    let s = evalbench(&["score", "--tasks", p(&tasks()), p(&out)]);
    assert!(
        stdout(&s).contains("\"first_pass\":3,\"score\":25,\"median_tokens_passing\":101"),
        "{s:?}"
    );
}

#[cfg(unix)]
#[test]
fn run_exec_failure_aborts_naming_task() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("traces.jsonl");
    let o = evalbench(&[
        "run",
        "--tasks",
        p(&tasks()),
        "--run-id",
        "exec-002",
        "--out",
        p(&out),
        "--exec",
        "/bin/sh",
        "--arg",
        p(&script("failing-model.sh")),
    ]);
    assert_ne!(o.status.code(), Some(0), "{o:?}");
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("t03-stripped"),
        "{o:?}"
    );
}
