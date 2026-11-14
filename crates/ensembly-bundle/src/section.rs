use regex::Regex;
use std::sync::LazyLock;

pub const RESERVED_SECTION_ENSEMBLY: &str = "ensembly";
pub const DEFAULT_SECTION: &str = "shared";

pub static SECTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_-]+$").expect("section re"));

/// Safe folder segment for bundle layout. Default `shared` when empty.
pub fn normalize_section(raw: Option<&str>) -> Result<String, String> {
    let s = raw.unwrap_or(DEFAULT_SECTION).trim();
    let s = if s.is_empty() { DEFAULT_SECTION } else { s };
    if s == RESERVED_SECTION_ENSEMBLY {
        return Err(format!(
            "section name '{RESERVED_SECTION_ENSEMBLY}' is reserved for kernel stores"
        ));
    }
    if !SECTION_RE.is_match(s) {
        return Err(format!(
            "invalid section name {s:?}: use letters, digits, '-' or '_' only"
        ));
    }
    Ok(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_reserved_and_dots() {
        assert!(normalize_section(Some("ensembly")).is_err());
        assert!(normalize_section(Some("work.folder")).is_err());
        assert_eq!(normalize_section(None).unwrap(), "shared");
        assert_eq!(normalize_section(Some("work")).unwrap(), "work");
    }
}
