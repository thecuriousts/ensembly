//! Test helpers. Tests may do I/O; the crate itself does not.
#![allow(dead_code)]

use evalbench_core::check::{parse_pack, parse_posting, Pack, Posting};
use evalbench_core::taskset::{FixtureFile, TaskSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

pub fn tasks_dir() -> PathBuf {
    fixtures().join("tasks")
}

pub fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Every case dir under `tasks/` and `parity/`, sorted.
pub fn case_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in ["tasks", "parity"] {
        let mut dirs: Vec<PathBuf> = fs::read_dir(fixtures().join(root))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        out.extend(dirs);
    }
    out
}

pub fn load_case(dir: &Path) -> (Posting, Pack) {
    let posting = parse_posting(&read(&dir.join("posting.json"))).expect("posting");
    let pack = parse_pack(&read(&dir.join("pack.json"))).expect("pack");
    (posting, pack)
}

pub fn task(id: &str) -> (Posting, Pack) {
    load_case(&tasks_dir().join(id))
}

pub fn parity(id: &str) -> (Posting, Pack) {
    load_case(&fixtures().join("parity").join(id))
}

pub fn expected(dir: &Path) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(&read(&dir.join("expected.json"))).unwrap();
    v["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g.as_str().unwrap().to_string())
        .collect()
}

/// Every file under `dir`, as relative `/` paths, in directory-walk order (unsorted on purpose).
pub fn fixture_files(dir: &Path) -> Vec<FixtureFile> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<FixtureFile>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(FixtureFile {
                    rel_path: rel,
                    bytes: fs::read(&p).unwrap(),
                });
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

pub fn frozen_set() -> TaskSet {
    TaskSet::from_files(fixture_files(&tasks_dir())).expect("frozen task set loads")
}

pub fn committed_hash() -> String {
    read(&fixtures().join("tasks.sha256"))
        .trim_end()
        .to_string()
}

pub fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}
