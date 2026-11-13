//! File and process side of evalbench. Core stays pure.
//!
//! E1 STATUS: types are declared; every function body is an `E1:` stub for the builder.

use evalbench_core::runner::{ClientError, ModelClient, ModelReply};
use evalbench_core::taskset::{FixtureFile, TaskSet, TaskSetError};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug)]
pub enum LoadError {
    Io { path: PathBuf, msg: String },
    Symlink(PathBuf),
    NotUtf8(PathBuf),
    TaskSet(TaskSetError),
}

/// Recursive; includes dotfiles so `TaskSet::from_files` rejects them (D15).
pub fn read_fixture_dir(dir: &Path) -> Result<Vec<FixtureFile>, LoadError> {
    let _ = dir;
    unimplemented!("E1: read_fixture_dir")
}

pub fn load_task_set(dir: &Path) -> Result<TaskSet, LoadError> {
    let _ = dir;
    unimplemented!("E1: load_task_set")
}

/// Live calls go through a harness wrapper (D12): prompt on stdin, env
/// `EVALBENCH_TASK_ID`, stdout = one `ModelReply` JSON. evalbench holds no API keys.
#[derive(Debug, Clone)]
pub struct CommandClient {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub timeout: Duration,
}

impl ModelClient for CommandClient {
    fn complete(&mut self, task_id: &str, prompt: &str) -> Result<ModelReply, ClientError> {
        let _ = (task_id, prompt);
        unimplemented!("E1: CommandClient::complete")
    }
}
