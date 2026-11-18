//! Explicit adoption of a JSONL file that predates the writer seal.
//! Silent adoption stays refused. The operator passes the sha256 of the bytes
//! they reviewed. A seal that already exists is not replaced.

use std::io::Write;
use std::path::Path;

use ensembly_schema::SealedIoError;

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
    if !path.is_file() {
        anyhow::bail!("adopt refused: {} is not a file", path.display());
    }
    if ensembly_schema::seal_path(path).exists() {
        anyhow::bail!("seal already exists; adopt will not replace it");
    }
    let bytes = std::fs::read(path)
        .map_err(|err| anyhow::anyhow!("adopt refused: read {}: {err}", path.display()))?;
    let actual = ensembly_schema::content_sha256(&bytes);
    if actual != expected {
        anyhow::bail!("sha256 does not match the file bytes");
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| anyhow::anyhow!("adopt refused: file is not utf-8"))?;
    let rows = row_count(text).map_err(|err| anyhow::anyhow!("adopt refused: {err}"))?;
    ensembly_schema::write_seal(path, &bytes).map_err(sealed)?;
    ensembly_schema::lock(path).map_err(sealed)?;
    let at = chrono::Local::now().to_rfc3339();
    let record = serde_json::json!({
        "path": path.display().to_string(),
        "sha256": actual,
        "rows": rows,
        "at": at,
        "reason": reason,
    });
    let log = ensembly_schema::adoption_log_path(path);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .map_err(|err| anyhow::anyhow!("seal written; adoption log {}: {err}", log.display()))?;
    writeln!(file, "{record}")
        .map_err(|err| anyhow::anyhow!("seal written; adoption log {}: {err}", log.display()))?;
    Ok(Adoption {
        path: path.display().to_string(),
        sha256: actual,
        rows,
        at,
        reason: reason.to_string(),
    })
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
}
