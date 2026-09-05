//! Official Grok (xAI) CLI integration helpers.
//!
//! Sources of truth:
//! - ACP: <https://docs.x.ai/build/cli/headless-scripting#acp> → `grok agent stdio`
//! - MCP: <https://docs.x.ai/build/features/mcp-servers> → `grok mcp add` / `.grok/config.toml`
//! - Headless prompt: `grok -p "…" --output-format json` (fast path for InferenceProvider)
//!
//! Always pass `--no-auto-update` in scripts/CI. Auth: `grok login` or `XAI_API_KEY`.

use serde_json::{json, Value};

/// ACP agent subprocess (JSON-RPC NDJSON on stdio).
pub const GROK_ACP_ARGS: &[&str] = &["agent", "stdio"];

/// Recommended script flags (suppress background update checks).
pub const GROK_SCRIPT_FLAGS: &[&str] = &["--no-auto-update"];

/// Build argv for headless single-prompt inference (reflect enrichment).
pub fn grok_headless_argv(prompt: &str, model: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--no-auto-update".into(),
        "-p".into(),
        prompt.into(),
        "--output-format".into(),
        "json".into(),
    ];
    if let Some(m) = model {
        args.push("-m".into());
        args.push(m.into());
    }
    args
}

/// Build argv for ACP agent mode.
pub fn grok_acp_argv() -> Vec<String> {
    vec![
        "--no-auto-update".into(),
        "agent".into(),
        "stdio".into(),
    ]
}

/// Project-scoped MCP server entry for `.grok/config.toml` (see xAI MCP docs).
/// Operator runs: `grok mcp add --scope project ensembly -- <command>…`
/// Existing `[mcp_servers.peram]` registrations still work with the `peram-mcp` alias.
pub fn project_mcp_toml_snippet(command: &str, args: &[&str]) -> String {
    let args_lit = args
        .iter()
        .map(|a| format!("\"{a}\""))
        .collect::<Vec<_>>()
        .join(", ");
    // Prefer an absolute ENSEMBLY_MEMORY when registering — relative paths create
    // an empty file if Grok's cwd ≠ repo root (MCP now fails closed instead).
    format!(
        r#"[mcp_servers.ensembly]
command = "{command}"
args = [{args_lit}]
env = {{ ENSEMBLY_MEMORY = "data/local/ensembly-memory.json" }}  # use absolute path in practice
startup_timeout_sec = 30
tool_timeout_sec = 120
"#
    )
}

/// Example `session/new` params fragment (ACP client → `grok agent stdio`).
/// Pass configured MCP servers here; empty array uses only Grok built-ins +
/// servers from `~/.grok/config.toml` / project `.grok/config.toml`.
pub fn acp_session_new_params(cwd: &str) -> Value {
    json!({
        "cwd": cwd,
        "mcpServers": []
    })
}

/// ACP initialize params matching the official headless example (protocolVersion 1).
pub fn acp_initialize_params() -> Value {
    json!({
        "protocolVersion": 1,
        "clientCapabilities": {
            "fs": { "readTextFile": true, "writeTextFile": true },
            "terminal": true
        }
    })
}

/// Shell one-liner to register ensembly-mcp into Grok (project scope).
pub fn register_mcp_shell_hint(mcp_path: &str) -> String {
    format!("grok mcp add --scope project ensembly -- {mcp_path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_argv_includes_json() {
        let a = grok_headless_argv("hello", Some("grok-3"));
        assert!(a.contains(&"-p".into()));
        assert!(a.contains(&"json".into()));
        assert!(a.contains(&"--no-auto-update".into()));
    }

    #[test]
    fn acp_argv_is_official() {
        assert_eq!(grok_acp_argv(), vec!["--no-auto-update", "agent", "stdio"]);
    }

    #[test]
    fn toml_snippet_has_mcp_servers_table() {
        let t = project_mcp_toml_snippet("ensembly-mcp", &[]);
        assert!(t.contains("[mcp_servers.ensembly]"));
        assert!(t.contains("command = \"ensembly-mcp\""));
    }
}
