//! Agent id from env or CLI override.

use ensembly_paths::{env_alias, DEFAULT_AGENT_ID};

pub fn resolve_agent_id(explicit: Option<String>) -> String {
    if let Some(id) = explicit.filter(|s| !s.is_empty()) {
        return id;
    }
    env_alias("ENSEMBLY_AGENT_ID", "PERAM_AGENT_ID").unwrap_or_else(|| DEFAULT_AGENT_ID.into())
}
