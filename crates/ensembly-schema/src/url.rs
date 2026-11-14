//! Canonical URL helpers shared by record and decision layers (not job-hunt-specific).

/// Canonical HTTPS URL key: trim a trailing `/` and optional `/application` suffix.
pub fn link_key(link: &str) -> &str {
    let l = link.trim_end_matches('/');
    l.strip_suffix("/application").unwrap_or(l)
}

/// RFC 3339 timestamp (`Z`/`z` accepted as UTC).
pub fn check_submit_time(t: &str) -> Result<(), String> {
    chrono::DateTime::parse_from_rfc3339(t)
        .map_err(|e| format!("submit_time {t:?} is not RFC 3339: {e}"))?;
    Ok(())
}
