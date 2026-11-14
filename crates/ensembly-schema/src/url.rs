//! Canonical URL helpers shared by record and decision layers (not job-hunt-specific).

/// Posting URL without a trailing `/` or `/application`.
pub fn link_key(link: &str) -> &str {
    let l = link.trim_end_matches('/');
    l.strip_suffix("/application").unwrap_or(l)
}

/// RFC 3339 with a numeric timezone offset (not `Z` alone).
pub fn check_submit_time(t: &str) -> Result<(), String> {
    if t.ends_with('Z') {
        return Err(format!(
            "submit_time {t:?} must include a numeric offset, not Z"
        ));
    }
    chrono::DateTime::parse_from_rfc3339(t)
        .map_err(|e| format!("submit_time {t:?} is not RFC 3339 with offset: {e}"))?;
    Ok(())
}
