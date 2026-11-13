//! Memory probe, one-model lock, and cloud-redaction helpers.
//!
//! Public git and cloud fallbacks must never see personal data, home paths,
//! or real host names. Tests use synthetic fixtures only.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Extra RAM kept free beyond the model file size before a load is allowed.
pub const MEMORY_HEADROOM_BYTES: u64 = 768 * 1024 * 1024;

/// How available memory is measured. Tests inject a fake view.
pub trait MemoryView: Send + Sync {
    fn available_bytes(&self) -> Option<u64>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct HostMemory;

impl MemoryView for HostMemory {
    fn available_bytes(&self) -> Option<u64> {
        available_bytes_now()
    }
}

pub fn available_bytes_now() -> Option<u64> {
    if cfg!(target_os = "linux") {
        linux_mem_available()
    } else if cfg!(target_os = "macos") {
        darwin_available()
    } else {
        None
    }
}

fn linux_mem_available() -> Option<u64> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb.saturating_mul(1024));
        }
    }
    None
}

fn darwin_available() -> Option<u64> {
    // vm_stat page size can differ from getconf; parse both.
    let page_size = Command::new("pagesize")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(16384);
    let output = Command::new("vm_stat").output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    let mut free = 0u64;
    let mut speculative = 0u64;
    let mut purgeable = 0u64;
    let mut inactive = 0u64;
    for line in text.lines() {
        let value = parse_vm_stat_pages(line).unwrap_or(0);
        if line.starts_with("Pages free:") {
            free = value;
        } else if line.starts_with("Pages speculative:") {
            speculative = value;
        } else if line.starts_with("Pages purgeable:") {
            purgeable = value;
        } else if line.starts_with("Pages inactive:") {
            inactive = value;
        }
    }
    // Reclaimable pages: free + speculative + purgeable + inactive (file-backed
    // cache can also be evicted, but we stay conservative).
    Some(
        free.saturating_add(speculative)
            .saturating_add(purgeable)
            .saturating_add(inactive)
            .saturating_mul(page_size),
    )
}

fn parse_vm_stat_pages(line: &str) -> Option<u64> {
    let (_, rhs) = line.split_once(':')?;
    rhs.trim().trim_end_matches('.').trim().parse().ok()
}

pub fn required_bytes_for_model(model_file_bytes: u64) -> u64 {
    model_file_bytes.saturating_add(MEMORY_HEADROOM_BYTES)
}

pub fn can_load_model(available: u64, model_file_bytes: u64) -> bool {
    available >= required_bytes_for_model(model_file_bytes)
}

/// Cross-process exclusive lock so only one model is loaded at a time.
#[derive(Debug)]
pub struct ModelLock {
    path: PathBuf,
    _file: File,
}

impl ModelLock {
    pub fn acquire(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("lock dir: {e}"))?;
        }
        for _ in 0..40 {
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(mut file) => {
                    let pid = std::process::id();
                    let _ = write!(file, "{pid}");
                    return Ok(Self {
                        path: path.to_path_buf(),
                        _file: file,
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    if stale_lock(path) {
                        let _ = fs::remove_file(path);
                        continue;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(err) => return Err(format!("model lock: {err}")),
            }
        }
        Err("model lock is held; only one local model may be loaded at a time".into())
    }
}

impl Drop for ModelLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn stale_lock(path: &Path) -> bool {
    let mut buf = String::new();
    if File::open(path)
        .and_then(|mut f| f.read_to_string(&mut buf))
        .is_err()
    {
        return true;
    }
    let Ok(pid) = buf.trim().parse::<i32>() else {
        return true;
    };
    if pid <= 0 {
        return true;
    }
    // Safety: `kill(pid, 0)` only probes existence.
    #[cfg(unix)]
    {
        let alive = libc_kill(pid, 0) == 0;
        !alive
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        // Fall back to age: locks older than 30 minutes are stale.
        match fs::metadata(path).and_then(|m| m.modified()) {
            Ok(modified) => match SystemTime::now().duration_since(modified) {
                Ok(age) => age.as_secs() > 30 * 60,
                Err(_) => true,
            },
            Err(_) => true,
        }
    }
}

#[cfg(unix)]
fn libc_kill(pid: i32, sig: i32) -> i32 {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    unsafe { kill(pid, sig) }
}

/// Patterns that must never ride a cloud fallback. Synthetic fixtures avoid them.
pub fn cloud_redaction_hits(text: &str) -> Vec<&'static str> {
    let mut hits = Vec::new();
    let lower = text.to_ascii_lowercase();
    if text.contains("/Users/") || text.contains("/home/") || lower.contains("\\users\\") {
        hits.push("home-path");
    }
    if text.contains("@") && text.contains('.') {
        // cheap email shape: token@token.tld
        if text.split_whitespace().any(is_email_shape) {
            hits.push("email");
        }
    }
    if looks_like_phone(&lower) {
        hits.push("phone");
    }
    if looks_like_cgnat(text) {
        hits.push("cgnat-ip");
    }
    // Real host names are denied; aliases are fine.
    if lower.contains(".local") && !lower.contains("example.local") {
        hits.push("local-hostname");
    }
    hits
}

pub fn assert_safe_for_cloud(parts: &[&str]) -> Result<(), String> {
    let mut joined = String::new();
    for part in parts {
        joined.push_str(part);
        joined.push('\n');
    }
    let hits = cloud_redaction_hits(&joined);
    if hits.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "refusing to send text to the cloud fallback ({})",
            hits.join(", ")
        ))
    }
}

fn is_email_shape(tok: &str) -> bool {
    let tok = tok.trim_matches(|c: char| {
        !c.is_ascii_alphanumeric() && c != '@' && c != '.' && c != '_' && c != '-' && c != '+'
    });
    let Some((user, rest)) = tok.split_once('@') else {
        return false;
    };
    let Some((host, tld)) = rest.rsplit_once('.') else {
        return false;
    };
    !user.is_empty() && !host.is_empty() && (2..=24).contains(&tld.len())
}

fn looks_like_phone(lower: &str) -> bool {
    let digits: String = lower.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.len() >= 10
        && (lower.contains('+') || lower.contains("tel:") || has_phone_separators(lower))
}

fn has_phone_separators(text: &str) -> bool {
    text.contains("(+") || text.chars().filter(|c| *c == '-').count() >= 2
}

fn looks_like_cgnat(text: &str) -> bool {
    text.split(|c: char| !c.is_ascii_digit() && c != '.')
        .filter(|tok| !tok.is_empty())
        .any(|tok| tok.starts_with("100.") && tok.chars().filter(|c| *c == '.').count() == 3)
}

pub fn default_lock_path() -> PathBuf {
    std::env::temp_dir().join("ensembly-local-check.lock")
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn memory_guard_requires_headroom() {
        let model = 3 * 1024 * 1024 * 1024;
        assert!(!can_load_model(model, model));
        assert!(can_load_model(model + MEMORY_HEADROOM_BYTES, model));
    }

    #[test]
    fn lock_is_exclusive_then_released() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.lock");
        let first = ModelLock::acquire(&path).unwrap();
        let start = std::time::Instant::now();
        // A second acquirer in this thread would wait; use a short timeout path
        // by leaving the lock held and checking the error after a stale-free file.
        let path2 = dir.path().join("held.lock");
        let held = ModelLock::acquire(&path2).unwrap();
        // Simulate a waiter seeing a live lock: write the current pid and expect timeout.
        let err = ModelLock::acquire(&path2).unwrap_err();
        assert!(err.contains("only one local model"), "{err}");
        drop(held);
        ModelLock::acquire(&path2).unwrap();
        drop(first);
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn redaction_blocks_email_and_home() {
        let home = format!("/{}/someone/notes", "Users");
        let mail = format!("{}@{}", "ada", "example.test");
        let hits = cloud_redaction_hits(&format!("write {home} and {mail}"));
        assert!(hits.contains(&"home-path"));
        assert!(hits.contains(&"email"));
        assert!(assert_safe_for_cloud(&["River Co. holds 12 teal crates."]).is_ok());
    }

    #[test]
    fn aliases_are_allowed() {
        let hits = cloud_redaction_hits("checked on mac-mini and laptop-1");
        assert!(hits.is_empty(), "{hits:?}");
    }
}
