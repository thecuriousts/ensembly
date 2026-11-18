//! Explicit adoption of a JSONL file that predates the writer seal.
//! Silent adoption stays refused. The operator passes the sha256 of the bytes
//! they reviewed. A seal that already exists is not replaced.
//!
//! The file is locked exclusively before it is read. The adoption log is
//! appended and fsynced before the seal name is published. A symlink at the
//! seal or the log is refused.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use ensembly_schema::SealedIoError;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// What `adopt-seal` recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adoption {
    pub path: String,
    pub sha256: String,
    pub rows: usize,
    pub at: String,
    pub reason: String,
}

/// `adopted path=… sha256=… rows=…`
pub fn adoption_line(adoption: &Adoption) -> String {
    format!(
        "adopted path={} sha256={} rows={}",
        adoption.path, adoption.sha256, adoption.rows
    )
}

/// Seal `path` when no seal exists, the bytes hash to `expected_sha256`, and
/// `row_count` accepts the text. Legacy warnings are the caller's concern:
/// `row_count` returns `Ok` when the only findings are warnings.
pub fn adopt_seal(
    path: &Path,
    expected_sha256: &str,
    reason: &str,
    row_count: impl FnOnce(&str) -> anyhow::Result<usize>,
) -> anyhow::Result<Adoption> {
    let reason = reason.trim();
    if reason.is_empty() || reason.contains(['\n', '\r']) {
        anyhow::bail!("reason is required");
    }
    let expected = expected_sha256.trim().to_ascii_lowercase();
    if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("sha256 must be 64 hex characters");
    }
    let mut file = open_nofollow(path)
        .map_err(|err| anyhow::anyhow!("adopt refused: open {}: {err}", path.display()))?;
    file.lock()
        .map_err(|err| anyhow::anyhow!("adopt refused: lock {}: {err}", path.display()))?;
    refuse_seal_target(&ensembly_schema::seal_path(path))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|err| anyhow::anyhow!("adopt refused: read {}: {err}", path.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|err| anyhow::anyhow!("adopt refused: read {}: {err}", path.display()))?;
    let actual = ensembly_schema::content_sha256(&bytes);
    if actual != expected {
        anyhow::bail!("sha256 does not match the file bytes");
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| anyhow::anyhow!("adopt refused: file is not utf-8"))?;
    let rows = row_count(text).map_err(|err| anyhow::anyhow!("adopt refused: {err}"))?;
    let at = chrono::Local::now().to_rfc3339();
    let record = serde_json::json!({
        "intent": "adopt-seal",
        "path": path.display().to_string(),
        "sha256": actual,
        "rows": rows,
        "at": at,
        "reason": reason,
    });
    // The log is the intent. Publish the seal only after this fsync, so a
    // failed log leaves no seal and the operator can retry.
    append_adoption_log(
        &ensembly_schema::adoption_log_path(path),
        &record.to_string(),
    )?;
    publish_new_seal(path, &actual)?;
    ensembly_schema::lock(path).map_err(sealed)?;
    // Keep the lock until the seal name is durable.
    drop(file);
    Ok(Adoption {
        path: path.display().to_string(),
        sha256: actual,
        rows,
        at,
        reason: reason.to_string(),
    })
}

fn refuse_seal_target(seal: &Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(seal) {
        Ok(meta) if meta.file_type().is_symlink() => {
            anyhow::bail!("seal is a symlink; adopt will not follow it");
        }
        Ok(_) => anyhow::bail!("seal already exists; adopt will not replace it"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => anyhow::bail!("adopt refused: seal {}: {err}", seal.display()),
    }
}

fn open_nofollow(path: &Path) -> std::io::Result<std::fs::File> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "path is a symlink",
            ));
        }
        Ok(meta) if meta.is_file() => {}
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "path is not a file",
            ));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "path is not a file",
            ));
        }
        Err(err) => return Err(err),
    }
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    opts.custom_flags(libc::O_NOFOLLOW);
    opts.open(path)
}

fn append_adoption_log(log: &Path, line: &str) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(log) {
        Ok(meta) if meta.file_type().is_symlink() => {
            anyhow::bail!("adoption log is a symlink; adopt will not follow it");
        }
        Ok(meta) if !meta.is_file() => {
            anyhow::bail!(
                "adopt refused: adoption log {} is not a file",
                log.display()
            );
        }
        Ok(_) | Err(_) => {}
    }
    if let Err(err) = std::fs::symlink_metadata(log) {
        if err.kind() != std::io::ErrorKind::NotFound {
            anyhow::bail!("adopt refused: adoption log {}: {err}", log.display());
        }
    }
    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        opts.mode(0o644);
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = opts
        .open(log)
        .map_err(|err| anyhow::anyhow!("adopt refused: adoption log {}: {err}", log.display()))?;
    #[cfg(unix)]
    {
        std::fs::set_permissions(log, std::fs::Permissions::from_mode(0o644)).map_err(|err| {
            anyhow::anyhow!("adopt refused: adoption log {}: {err}", log.display())
        })?;
    }
    writeln!(file, "{line}")
        .map_err(|err| anyhow::anyhow!("adopt refused: adoption log {}: {err}", log.display()))?;
    file.sync_all()
        .map_err(|err| anyhow::anyhow!("adopt refused: adoption log {}: {err}", log.display()))?;
    Ok(())
}

/// Publish `{hash}\n` as a new seal. Refuses an existing name, including a
/// symlink. Does not truncate or chmod a seal that is already there.
fn publish_new_seal(data: &Path, sha256_hex: &str) -> anyhow::Result<()> {
    let seal = ensembly_schema::seal_path(data);
    refuse_seal_target(&seal)?;
    let tmp = temp_seal_path(&seal)?;
    let body = format!("{sha256_hex}\n");
    let write_tmp = (|| -> anyhow::Result<()> {
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            opts.mode(0o644);
            opts.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = opts
            .open(&tmp)
            .map_err(|err| anyhow::anyhow!("adopt refused: seal temp {}: {err}", tmp.display()))?;
        file.write_all(body.as_bytes())
            .map_err(|err| anyhow::anyhow!("adopt refused: seal temp {}: {err}", tmp.display()))?;
        file.sync_all()
            .map_err(|err| anyhow::anyhow!("adopt refused: seal temp {}: {err}", tmp.display()))?;
        #[cfg(unix)]
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o444))
            .map_err(|err| anyhow::anyhow!("adopt refused: seal temp {}: {err}", tmp.display()))?;
        Ok(())
    })();
    if let Err(err) = write_tmp {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }
    if let Err(err) = std::fs::hard_link(&tmp, &seal) {
        let _ = std::fs::remove_file(&tmp);
        if err.kind() == std::io::ErrorKind::AlreadyExists {
            anyhow::bail!("seal already exists; adopt will not replace it");
        }
        anyhow::bail!("adopt refused: seal {}: {err}", seal.display());
    }
    std::fs::remove_file(&tmp)
        .map_err(|err| anyhow::anyhow!("adopt refused: seal temp {}: {err}", tmp.display()))?;
    if let Some(dir) = seal.parent() {
        if let Ok(dir_file) = std::fs::File::open(dir) {
            let _ = dir_file.sync_all();
        }
    }
    Ok(())
}

fn temp_seal_path(seal: &Path) -> anyhow::Result<PathBuf> {
    let dir = seal.parent().unwrap_or_else(|| Path::new("."));
    let mut name = seal
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("seal"))
        .to_os_string();
    name.push(format!(
        ".{}.tmp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    Ok(dir.join(name))
}

fn sealed(err: SealedIoError) -> anyhow::Error {
    let msg = match err {
        SealedIoError::Io(msg) | SealedIoError::Refused(msg) => msg,
    };
    anyhow::anyhow!("adopt refused: {msg}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_lines(text: &str) -> anyhow::Result<usize> {
        Ok(text.lines().filter(|l| !l.trim().is_empty()).count())
    }

    #[test]
    fn adopt_seals_the_named_bytes_and_a_second_adopt_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        let body = b"{\"ok\":true}\n";
        std::fs::write(&path, body).unwrap();
        let hex = ensembly_schema::content_sha256(body);
        let adopted = adopt_seal(&path, &hex, "reviewed on laptop-1", count_lines).unwrap();
        assert_eq!(adopted.sha256, hex);
        assert_eq!(adopted.rows, 1);
        assert_eq!(
            adoption_line(&adopted),
            format!("adopted path={} sha256={hex} rows=1", path.display())
        );
        let seal = std::fs::read_to_string(ensembly_schema::seal_path(&path)).unwrap();
        assert_eq!(seal, format!("{hex}\n"));
        assert_eq!(std::fs::read(&path).unwrap(), body);
        let log = std::fs::read_to_string(ensembly_schema::adoption_log_path(&path)).unwrap();
        assert!(log.contains("reviewed on laptop-1"), "{log}");
        assert!(log.contains(&hex), "{log}");
        let again = adopt_seal(&path, &hex, "again", count_lines).unwrap_err();
        assert!(again.to_string().contains("seal already exists"), "{again}");
    }

    #[test]
    fn wrong_hash_missing_file_and_invalid_text_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        std::fs::write(&path, b"not-json\n").unwrap();
        let wrong = "0".repeat(64);
        let err = adopt_seal(&path, &wrong, "reviewed on laptop-1", count_lines).unwrap_err();
        assert!(err.to_string().contains("does not match"), "{err}");
        assert!(!ensembly_schema::seal_path(&path).exists());
        let hex = ensembly_schema::content_sha256(b"not-json\n");
        let err = adopt_seal(&path, &hex, "reviewed on laptop-1", |_| {
            anyhow::bail!("line 1: expected value")
        })
        .unwrap_err();
        assert!(err.to_string().contains("adopt refused"), "{err}");
        assert!(!ensembly_schema::seal_path(&path).exists());
        let missing = dir.path().join("nope.jsonl");
        let err = adopt_seal(&missing, &hex, "reviewed on laptop-1", count_lines).unwrap_err();
        assert!(err.to_string().contains("is not a file"), "{err}");
        let err = adopt_seal(&path, &hex, "  ", count_lines).unwrap_err();
        assert!(err.to_string().contains("reason is required"), "{err}");
    }

    #[test]
    fn mismatched_regular_seal_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        let body = b"{\"ok\":true}\n";
        std::fs::write(&path, body).unwrap();
        let seal = ensembly_schema::seal_path(&path);
        let stale = b"not-the-hash\n";
        std::fs::write(&seal, stale).unwrap();
        let hex = ensembly_schema::content_sha256(body);
        let err = adopt_seal(&path, &hex, "reviewed on laptop-1", count_lines).unwrap_err();
        assert!(err.to_string().contains("seal already exists"), "{err}");
        assert_eq!(std::fs::read(&seal).unwrap(), stale);
        assert!(!ensembly_schema::adoption_log_path(&path).exists());
    }

    #[cfg(unix)]
    #[test]
    fn seal_symlink_dangling_or_live_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        let body = b"{\"ok\":true}\n";
        std::fs::write(&path, body).unwrap();
        let hex = ensembly_schema::content_sha256(body);
        let seal = ensembly_schema::seal_path(&path);
        std::os::unix::fs::symlink("missing-seal-target", &seal).unwrap();
        let err = adopt_seal(&path, &hex, "reviewed on laptop-1", count_lines).unwrap_err();
        assert!(err.to_string().contains("symlink"), "{err}");
        assert!(std::fs::symlink_metadata(&seal)
            .unwrap()
            .file_type()
            .is_symlink());
        std::fs::remove_file(&seal).unwrap();
        let live = dir.path().join("live-seal");
        std::fs::write(&live, b"keep-me\n").unwrap();
        std::os::unix::fs::symlink(&live, &seal).unwrap();
        let err = adopt_seal(&path, &hex, "reviewed on laptop-1", count_lines).unwrap_err();
        assert!(err.to_string().contains("symlink"), "{err}");
        assert_eq!(std::fs::read(&live).unwrap(), b"keep-me\n");
        assert!(!ensembly_schema::adoption_log_path(&path).exists());
    }

    #[cfg(unix)]
    #[test]
    fn adoptions_symlink_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        let body = b"{\"ok\":true}\n";
        std::fs::write(&path, body).unwrap();
        let hex = ensembly_schema::content_sha256(body);
        let log = ensembly_schema::adoption_log_path(&path);
        std::os::unix::fs::symlink("missing-log-target", &log).unwrap();
        let err = adopt_seal(&path, &hex, "reviewed on laptop-1", count_lines).unwrap_err();
        assert!(err.to_string().contains("symlink"), "{err}");
        assert!(!ensembly_schema::seal_path(&path).exists());
        assert!(std::fs::symlink_metadata(&log)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn file_changed_between_hash_and_adopt_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        let first = b"{\"ok\":true}\n";
        std::fs::write(&path, first).unwrap();
        let hex = ensembly_schema::content_sha256(first);
        let changed = b"{\"ok\":false}\n";
        std::fs::write(&path, changed).unwrap();
        let err = adopt_seal(&path, &hex, "reviewed on laptop-1", count_lines).unwrap_err();
        assert!(err.to_string().contains("does not match"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), changed);
        assert!(!ensembly_schema::seal_path(&path).exists());
        assert!(!ensembly_schema::adoption_log_path(&path).exists());
    }
}
