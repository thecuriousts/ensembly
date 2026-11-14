use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

pub static DEFAULT_EXCLUDE_GLOBS: &[&str] = &[
    ".env",
    ".env.*",
    "*.pem",
    "*.key",
    "id_*",
    "*token*",
    "*secret*",
    "credentials*",
    ".git/**",
    "node_modules/**",
    "target/**",
];

static SECRET_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)BEGIN\s+(RSA\s+)?PRIVATE\s+KEY",
        r"AKIA[0-9A-Z]{16}",
        r"(?i)api[_-]?key\s*[=:]\s*[a-z0-9]{16,}",
        r"(?i)secret[_-]?key\s*[:=]",
        r"ghp_[a-zA-Z0-9]{20,}",
        r"xox[baprs]-[a-zA-Z0-9-]{10,}",
    ]
    .iter()
    .map(|p| Regex::new(p).expect("secret pattern"))
    .collect()
});

const SCAN_HEAD_BYTES: usize = 8192;

pub fn file_matches_glob(rel: &str, pattern: &str) -> bool {
    glob::Pattern::new(pattern)
        .ok()
        .is_some_and(|p| p.matches(rel))
}

pub fn default_excluded(rel: &str) -> bool {
    DEFAULT_EXCLUDE_GLOBS
        .iter()
        .any(|g| file_matches_glob(rel, g))
}

/// Sample start of file for obvious secret material.
pub fn scan_secrets(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = vec![0u8; SCAN_HEAD_BYTES];
    let n = std::io::Read::read(&mut file, &mut head).unwrap_or(0);
    head.truncate(n);
    let text = String::from_utf8_lossy(&head);
    for re in SECRET_PATTERNS.iter() {
        if re.is_match(&text) {
            return Some(format!("matched secret pattern {}", re.as_str()));
        }
    }
    None
}