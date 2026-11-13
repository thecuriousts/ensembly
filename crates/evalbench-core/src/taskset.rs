//! Frozen synthetic task set and its hash.
//!
//! Hash method: every task dir holds exactly `TASK_FILES`; paths are relative, `/`
//! separated, sorted bytewise; manifest line = `<sha256 hex of raw bytes>  <relpath>\n`
//! (sha256sum format); set hash = sha256 hex of the manifest. Any other entry
//! (dotfile, stray file, missing file, bad id) is an error naming the path.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::check::{gaps, parse_pack, parse_posting, Pack, Posting};
use crate::trace::valid_task_id;

pub const TASK_FILES: [&str; 3] = ["expected.json", "pack.json", "posting.json"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureFile {
    /// Relative to the tasks root, `/` separated, e.g. `t01-full/pack.json`.
    pub rel_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub posting: Posting,
    pub pack: Pack,
    /// From `expected.json` (`{"gaps":[...]}`); must equal `gaps(posting, pack)`.
    pub expected_gaps: Vec<String>,
    pub posting_raw: String,
    pub pack_raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSet {
    pub hash: String,
    /// Sorted by id.
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TaskSetError {
    #[error("bad path: {0}")]
    BadPath(String),
    #[error("stray file: {0}")]
    StrayFile(String),
    #[error("task {task} is missing {file}")]
    MissingFile { task: String, file: String },
    #[error("bad task id: {0}")]
    BadTaskId(String),
    #[error("duplicate path: {0}")]
    DuplicatePath(String),
    #[error("not UTF-8: {0}")]
    NotUtf8(String),
    #[error("bad JSON in {path}: {msg}")]
    Json { path: String, msg: String },
    #[error("task {task}: expected {expected:?}, checker gives {actual:?}")]
    ExpectedMismatch {
        task: String,
        expected: Vec<String>,
        actual: Vec<String>,
    },
}

pub fn manifest(files: &[FixtureFile]) -> String {
    let mut sorted: Vec<&FixtureFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.rel_path.as_bytes().cmp(b.rel_path.as_bytes()));
    let mut out = String::new();
    for f in sorted {
        out.push_str(&hex::encode(Sha256::digest(&f.bytes)));
        out.push_str("  ");
        out.push_str(&f.rel_path);
        out.push('\n');
    }
    out
}

/// Lowercase hex sha256 of `manifest(files)`.
pub fn set_hash(files: &[FixtureFile]) -> String {
    hex::encode(Sha256::digest(manifest(files).as_bytes()))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    gaps: Vec<String>,
}

/// Splits `<id>/<file>` after checking the path shape. Ids are checked by the caller.
fn split_path(rel: &str) -> Result<(&str, &str), TaskSetError> {
    let bad = || TaskSetError::BadPath(rel.to_string());
    if rel.is_empty() || rel.contains('\\') || rel.starts_with('/') || rel.contains('\0') {
        return Err(bad());
    }
    let parts: Vec<&str> = rel.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return Err(bad());
    }
    match parts.as_slice() {
        [id, file] => Ok((id, file)),
        // A file at the root, or anything nested deeper, is not part of the layout.
        _ => Err(TaskSetError::StrayFile(rel.to_string())),
    }
}

fn utf8(f: &FixtureFile) -> Result<String, TaskSetError> {
    String::from_utf8(f.bytes.clone()).map_err(|_| TaskSetError::NotUtf8(f.rel_path.clone()))
}

fn json_err(path: &str, e: serde_json::Error) -> TaskSetError {
    TaskSetError::Json {
        path: path.to_string(),
        msg: e.to_string(),
    }
}

impl TaskSet {
    pub fn from_files(files: Vec<FixtureFile>) -> Result<TaskSet, TaskSetError> {
        let mut files = files;
        files.sort_by(|a, b| a.rel_path.as_bytes().cmp(b.rel_path.as_bytes()));

        // task id -> file name -> file
        let mut by_task: BTreeMap<String, BTreeMap<String, &FixtureFile>> = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for f in &files {
            if !seen.insert(f.rel_path.as_str()) {
                return Err(TaskSetError::DuplicatePath(f.rel_path.clone()));
            }
            let (id, name) = split_path(&f.rel_path)?;
            if !valid_task_id(id) {
                return Err(TaskSetError::BadTaskId(id.to_string()));
            }
            if !TASK_FILES.contains(&name) {
                return Err(TaskSetError::StrayFile(f.rel_path.clone()));
            }
            by_task
                .entry(id.to_string())
                .or_default()
                .insert(name.to_string(), f);
        }

        let mut tasks = Vec::with_capacity(by_task.len());
        for (id, present) in &by_task {
            let get = |file: &str| {
                present
                    .get(file)
                    .copied()
                    .ok_or_else(|| TaskSetError::MissingFile {
                        task: id.clone(),
                        file: file.to_string(),
                    })
            };
            let (expected_f, pack_f, posting_f) = (
                get("expected.json")?,
                get("pack.json")?,
                get("posting.json")?,
            );
            let posting_raw = utf8(posting_f)?;
            let pack_raw = utf8(pack_f)?;
            let expected_raw = utf8(expected_f)?;
            let posting =
                parse_posting(&posting_raw).map_err(|e| json_err(&posting_f.rel_path, e))?;
            let pack = parse_pack(&pack_raw).map_err(|e| json_err(&pack_f.rel_path, e))?;
            let expected: Expected = serde_json::from_str(&expected_raw)
                .map_err(|e| json_err(&expected_f.rel_path, e))?;
            let actual = gaps(&posting, &pack);
            if actual != expected.gaps {
                return Err(TaskSetError::ExpectedMismatch {
                    task: id.clone(),
                    expected: expected.gaps,
                    actual,
                });
            }
            tasks.push(Task {
                id: id.clone(),
                posting,
                pack,
                expected_gaps: expected.gaps,
                posting_raw,
                pack_raw,
            });
        }

        Ok(TaskSet {
            hash: set_hash(&files),
            tasks,
        })
    }

    pub fn ids(&self) -> Vec<String> {
        self.tasks.iter().map(|t| t.id.clone()).collect()
    }
}
