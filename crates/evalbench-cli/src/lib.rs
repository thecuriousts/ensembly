//! File and process side of evalbench. Core stays pure.
//!
//! E1 STATUS: types are declared; every function body is an `E1:` stub for the builder.

use evalbench_core::runner::{ClientError, ModelClient, ModelReply};
use evalbench_core::taskset::{FixtureFile, TaskSet, TaskSetError};
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum LoadError {
    Io { path: PathBuf, msg: String },
    Symlink(PathBuf),
    NotUtf8(PathBuf),
    TaskSet(TaskSetError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io { path, msg } => write!(f, "{}: {msg}", path.display()),
            LoadError::Symlink(p) => write!(f, "symlink not allowed: {}", p.display()),
            LoadError::NotUtf8(p) => write!(f, "path is not UTF-8: {}", p.display()),
            LoadError::TaskSet(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LoadError {}

fn io_err(path: &Path, e: std::io::Error) -> LoadError {
    LoadError::Io {
        path: path.to_path_buf(),
        msg: e.to_string(),
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<FixtureFile>) -> Result<(), LoadError> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| io_err(dir, e))?
        .map(|e| e.map(|e| e.path()).map_err(|e| io_err(dir, e)))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        let meta = fs::symlink_metadata(&path).map_err(|e| io_err(&path, e))?;
        if meta.file_type().is_symlink() {
            return Err(LoadError::Symlink(path));
        }
        if meta.is_dir() {
            walk(root, &path, out)?;
            continue;
        }
        let rel = path.strip_prefix(root).expect("walk stays under root");
        let mut parts = Vec::new();
        for c in rel.components() {
            let s = c
                .as_os_str()
                .to_str()
                .ok_or_else(|| LoadError::NotUtf8(path.clone()))?;
            parts.push(s.to_string());
        }
        out.push(FixtureFile {
            rel_path: parts.join("/"),
            bytes: fs::read(&path).map_err(|e| io_err(&path, e))?,
        });
    }
    Ok(())
}

/// Recursive; includes dotfiles so `TaskSet::from_files` rejects them (D15).
pub fn read_fixture_dir(dir: &Path) -> Result<Vec<FixtureFile>, LoadError> {
    let meta = fs::symlink_metadata(dir).map_err(|e| io_err(dir, e))?;
    if meta.file_type().is_symlink() {
        return Err(LoadError::Symlink(dir.to_path_buf()));
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    Ok(out)
}

pub fn load_task_set(dir: &Path) -> Result<TaskSet, LoadError> {
    TaskSet::from_files(read_fixture_dir(dir)?).map_err(LoadError::TaskSet)
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
        let fail = |message: String| ClientError {
            task_id: task_id.to_string(),
            message,
        };
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .env("EVALBENCH_TASK_ID", task_id)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| fail(format!("cannot start {}: {e}", self.program.display())))?;

        // Feed stdin and drain both pipes on threads so a chatty child cannot deadlock.
        let mut stdin = child.stdin.take().expect("piped stdin");
        let prompt_bytes = prompt.as_bytes().to_vec();
        let writer = thread::spawn(move || {
            // A child that exits without reading closes the pipe; that is not our error.
            let _ = stdin.write_all(&prompt_bytes);
        });
        let stdout = drain(child.stdout.take().expect("piped stdout"));
        let stderr = drain(child.stderr.take().expect("piped stderr"));

        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() >= self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(fail(format!("timed out after {:?}", self.timeout)));
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(e) => return Err(fail(format!("wait failed: {e}"))),
            }
        };
        let _ = writer.join();
        let stdout = stdout.join().unwrap_or_default();
        let stderr = stderr.join().unwrap_or_default();

        if !status.success() {
            let tail = String::from_utf8_lossy(&stderr);
            let tail: String = tail.trim().chars().take(400).collect();
            return Err(fail(format!("wrapper exited with {status}: {tail}")));
        }
        let text = String::from_utf8(stdout).map_err(|_| fail("stdout is not UTF-8".into()))?;
        serde_json::from_str::<ModelReply>(text.trim())
            .map_err(|e| fail(format!("stdout is not one ModelReply JSON: {e}")))
    }
}

fn drain<R: Read + Send + 'static>(mut r: R) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = r.read_to_end(&mut buf);
        buf
    })
}
