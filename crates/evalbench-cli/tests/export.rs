//! E4 export pack (`evalbench export`, evalbench-export-v1) on the shipped path.
//! Synthetic fixtures only: the E1 replay baseline and the E2 pulse goldens.
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Commit the E1 replay-001 traces were recorded at (see baselines/replay-001/run.md).
const COMMIT: &str = "a0b5c4d16f377fd5ad05a357d213b1bb7cbe4e81";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn tasks() -> PathBuf {
    root().join("../evalbench-core/fixtures/tasks")
}

fn traces() -> PathBuf {
    root().join("baselines/replay-001/traces.jsonl")
}

fn pulse_vector(name: &str) -> PathBuf {
    root()
        .join("../ensembly-kernel/tests/golden/pulse-pack/vectors")
        .join(name)
}

fn golden() -> PathBuf {
    root().join("exports/e1-replay-001.export.json")
}

fn plugin() -> PathBuf {
    root().join("../../marketplaces/cursor/flowingstack")
}

fn p(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn evalbench(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_evalbench"))
        .args(args)
        .output()
        .expect("run evalbench")
}

fn export(extra: &[&str], out: &Path) -> Output {
    let (t, r) = (tasks(), traces());
    let mut args = vec![
        "export",
        "--tasks",
        p(&t),
        "--traces",
        p(&r),
        "--commit",
        COMMIT,
        "--out",
        p(out),
    ];
    args.extend_from_slice(extra);
    evalbench(&args)
}

fn sha(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

fn json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn export_reproduces_the_committed_golden_byte_for_byte() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("e.json");
    let pack = pulse_vector("02-python-basic/input.json");
    let o = export(&["--pulse-pack", p(&pack)], &out);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert_eq!(fs::read(&out).unwrap(), fs::read(golden()).unwrap());
    // And the plugin's fixture is the same file.
    assert_eq!(
        fs::read(plugin().join("test/fixtures/record-view/e1-replay-001.export.json")).unwrap(),
        fs::read(golden()).unwrap()
    );
}

#[test]
fn export_carries_task_result_tokens_commit_and_the_canonical_pack() {
    let doc = json(&golden());
    assert_eq!(doc["format"], "evalbench-export-v1");
    assert_eq!(doc["commit"], COMMIT);
    let score = json(&root().join("baselines/replay-001/score.json"));
    let run = &doc["runs"][0];
    for k in ["first_pass", "score", "tasks", "task_set_hash"] {
        assert_eq!(run[k], score[k], "{k}");
    }
    let rows = run["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 12);
    for r in rows {
        for k in ["task_id", "pass", "tokens_in", "tokens_out", "model_id"] {
            assert!(!r[k].is_null(), "{k} missing in {r}");
        }
    }
    // The pulse part is the E2 canonical pack, with its committed hash.
    let v = pulse_vector("02-python-basic");
    assert_eq!(
        doc["pulse"]["pack"],
        json(&v.join("canonical.json")),
        "pack is not the canonical golden"
    );
    let want = fs::read_to_string(v.join("sha256.txt")).unwrap();
    assert!(
        want.contains(doc["pulse"]["pack_sha256"].as_str().unwrap()),
        "pack_sha256 does not match the golden"
    );
}

#[test]
fn export_is_deterministic_and_old_and_canonical_packs_give_one_export() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a.json");
    let b = tmp.path().join("b.json");
    let v = pulse_vector("02-python-basic");
    assert!(export(&["--pulse-pack", p(&v.join("input.json"))], &a)
        .status
        .success());
    assert!(export(&["--pulse-pack", p(&v.join("canonical.json"))], &b)
        .status
        .success());
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
}

#[test]
fn export_without_pulse_has_null_pulse() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("e.json");
    assert!(export(&[], &out).status.success());
    assert!(json(&out)["pulse"].is_null());
}

#[test]
fn export_refuses_bad_inputs_and_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("e.json");
    // Short or upper-case commit.
    for c in ["a0b5c4d", &COMMIT.to_uppercase()] {
        let o = evalbench(&[
            "export",
            "--tasks",
            p(&tasks()),
            "--traces",
            p(&traces()),
            "--commit",
            c,
            "--out",
            p(&out),
        ]);
        assert_ne!(o.status.code(), Some(0), "{c}");
        assert!(!out.exists());
    }
    // A pack the canonical checker rejects.
    let bad = tmp.path().join("bad.json");
    fs::write(&bad, b"{\"format\":\"pulse-pack-v1\",\"traces\":[1.5]}").unwrap();
    let o = export(&["--pulse-pack", p(&bad)], &out);
    assert_ne!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stderr).contains("pulse pack is not valid"));
    assert!(!out.exists());
    // Both pulse sources at once.
    let o = export(
        &[
            "--pulse-pack",
            p(&pulse_vector("02-python-basic/input.json")),
            "--pulse-db",
            p(&bad),
        ],
        &out,
    );
    assert_eq!(o.status.code(), Some(2));
    // An existing --out is not overwritten.
    fs::write(&out, b"keep").unwrap();
    assert_ne!(export(&[], &out).status.code(), Some(0));
    assert_eq!(fs::read(&out).unwrap(), b"keep");
}

#[test]
fn export_from_store_matches_the_db_golden_and_leaves_the_store_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("store");
    fs::create_dir(&dir).unwrap();
    let db = dir.join("pulse.sqlite");
    let v = pulse_vector("08-db");
    // Fixture setup only: the export under test never writes.
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch(&fs::read_to_string(v.join("fixture.sql")).unwrap())
        .unwrap();
    let before = sha(&fs::read(&db).unwrap());
    let args = json(&v.join("args.json"));
    let out = tmp.path().join("e.json");
    let o = export(
        &[
            "--pulse-db",
            p(&db),
            "--exported-at",
            args["exported_at"].as_str().unwrap(),
            "--host",
            args["host"].as_str().unwrap(),
        ],
        &out,
    );
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert_eq!(json(&out)["pulse"]["pack"], json(&v.join("canonical.json")));
    assert_eq!(sha(&fs::read(&db).unwrap()), before, "store bytes changed");
    let side: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n != "pulse.sqlite")
        .collect();
    assert!(
        side.iter().all(|n| !n.ends_with("-journal")),
        "journal left: {side:?}"
    );
}

#[test]
fn export_from_a_missing_store_fails_and_creates_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("absent.sqlite");
    let out = tmp.path().join("e.json");
    let o = export(
        &[
            "--pulse-db",
            p(&db),
            "--exported-at",
            "2026-11-13T12:00:00Z",
            "--host",
            "laptop-1",
        ],
        &out,
    );
    assert_ne!(o.status.code(), Some(0));
    assert!(!db.exists(), "export created a store");
    assert!(!out.exists());
}

#[test]
fn record_view_shows_task_result_tokens_and_sha() {
    let o = Command::new("python3")
        .arg("-I")
        .arg(plugin().join("scripts/record-view"))
        .arg(golden())
        .output()
        .expect("python3");
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(s.contains(&format!("commit {COMMIT}")), "{s}");
    assert!(s.lines().filter(|l| l.starts_with("task ")).count() >= 12);
    let doc = json(&golden());
    let r = &doc["runs"][0]["rows"][0];
    let line = format!(
        "task {} {} tokens {}",
        r["task_id"].as_str().unwrap(),
        if r["pass"] == true { "pass" } else { "fail" },
        r["tokens_in"].as_u64().unwrap() + r["tokens_out"].as_u64().unwrap()
    );
    assert!(s.contains(&line), "missing {line:?} in {s}");
}
