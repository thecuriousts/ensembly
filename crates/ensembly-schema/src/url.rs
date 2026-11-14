//! Canonical URL helpers shared by record and decision layers (not job-hunt-specific).

/// Posting URL without a trailing `/` or `/application`.
pub fn link_key(link: &str) -> &str {
    let l = link.trim_end_matches('/');
    l.strip_suffix("/application").unwrap_or(l)
}

/// RFC 3339 with Europe/Stockholm offset (+01:00 or +02:00).
pub fn check_submit_time(t: &str) -> Result<(), String> {
    chrono::DateTime::parse_from_rfc3339(t)
        .map_err(|e| format!("submit_time {t:?} is not RFC 3339 with offset: {e}"))?;
    if t.ends_with("+01:00") || t.ends_with("+02:00") {
        Ok(())
    } else {
        Err(format!(
            "submit_time {t:?} must carry the Europe/Stockholm offset (+01:00 or +02:00)"
        ))
    }
}
