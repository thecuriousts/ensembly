//! Writer / node identity validation (public host aliases only in fixtures).

pub const HOST_ALIASES: &[&str] = &["laptop-1", "laptop-2", "mac-mini", "box"];

pub fn is_host_alias(s: &str) -> bool {
    HOST_ALIASES.contains(&s.trim())
}

pub const WRITER_ENV: &str = "ENSEMBLY_WRITER";
pub const NODE_ENV: &str = "ENSEMBLY_NODE";

const WRITER_MAX_CHARS: usize = 64;

pub fn validate_writer_identity(writer: &str) -> Result<String, String> {
    let writer = writer.trim();
    if writer.is_empty() || writer.chars().count() > WRITER_MAX_CHARS {
        return Err("writer is required".into());
    }
    let lower = writer.to_ascii_lowercase();
    if writer.contains('@')
        || writer.contains('/')
        || writer.contains('\\')
        || writer.contains('~')
        || lower.contains("/home/")
        || lower.contains("/users/")
        || !writer
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err("writer is not a safe identity".into());
    }
    Ok(writer.to_string())
}

pub fn require_configured_writer(writer: &str) -> Result<String, String> {
    let writer = validate_writer_identity(writer)?;
    let expected = configured_writer()?;
    if writer != expected {
        Err("writer does not match the configured identity".into())
    } else {
        Ok(writer)
    }
}

pub fn configured_writer() -> Result<String, String> {
    let raw = std::env::var(WRITER_ENV).map_err(|_| format!("{WRITER_ENV} is not set"))?;
    validate_writer_identity(&raw)
}

pub fn configured_node() -> Result<String, String> {
    if let Ok(raw) = std::env::var(NODE_ENV) {
        let node = validate_writer_identity(&raw)?;
        if !is_host_alias(&node) {
            return Err("node is not a host alias".into());
        }
        return Ok(node);
    }
    let writer = configured_writer()?;
    if is_host_alias(&writer) {
        Ok(writer)
    } else {
        Err("node is not a host alias".into())
    }
}
