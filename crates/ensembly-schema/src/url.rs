//! Canonical URL helpers shared by record and decision layers (not job-hunt-specific).

/// Posting URL without a trailing `/` or `/application`.
pub fn link_key(link: &str) -> &str {
    let l = link.trim_end_matches('/');
    l.strip_suffix("/application").unwrap_or(l)
}

/// RFC 3339 timestamp (`Z`/`z` accepted as UTC).
pub fn check_submit_time(t: &str) -> Result<(), String> {
    let normalized = if t.ends_with('Z') || t.ends_with('z') {
        format!("{}+00:00", &t[..t.len() - 1])
    } else {
        t.to_string()
    };
    chrono::DateTime::parse_from_rfc3339(&normalized)
        .map_err(|e| format!("submit_time {t:?} is not RFC 3339: {e}"))?;
    Ok(())
}
