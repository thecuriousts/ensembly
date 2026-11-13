//! Frozen synthetic task set and its hash.
//!
//! Hash method: every task dir holds exactly `TASK_FILES`; paths are relative, `/`
//! separated, sorted bytewise; manifest line = `<sha256 hex of raw bytes>  <relpath>\n`
//! (sha256sum format); set hash = sha256 hex of the manifest. Any other entry
//! (dotfile, stray file, missing file, bad id) is an error naming the path.

use crate::check::{Pack, Posting};

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
    let _ = files;
    unimplemented!("E1: manifest")
}

/// Lowercase hex sha256 of `manifest(files)`.
pub fn set_hash(files: &[FixtureFile]) -> String {
    let _ = files;
    unimplemented!("E1: set_hash")
}

impl TaskSet {
    pub fn from_files(files: Vec<FixtureFile>) -> Result<TaskSet, TaskSetError> {
        let _ = files;
        unimplemented!("E1: TaskSet::from_files")
    }

    pub fn ids(&self) -> Vec<String> {
        unimplemented!("E1: TaskSet::ids")
    }
}
