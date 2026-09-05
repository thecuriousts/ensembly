//! `ensembly-mcp` — read-only Model Context Protocol server for episodic memory.
//!
//! Register with Grok/Cursor. Example:
//! ```text
//! cargo run -p ensembly-agents --bin ensembly-mcp
//! # or after install: ensembly-mcp
//! ```
//! Env: `ENSEMBLY_MEMORY` / `PERAM_MEMORY` (fresh default data/local/ensembly-memory.json),
//! `ENSEMBLY_AGENT_ID` / `PERAM_AGENT_ID` (new stores: ensembly-swarm).

use clap::Parser;
use ensembly_agents::{serve_mcp, McpServeConfig};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = env!("CARGO_BIN_NAME"), about = "ensembly read-only MCP server (memory tools for Grok/Cursor)")]
struct Cli {
    #[arg(long)]
    memory: Option<PathBuf>,
    #[arg(long)]
    agent_id: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut config = McpServeConfig::default();
    if let Some(p) = cli.memory.or_else(|| {
        ensembly_agents::env_alias::env_alias("ENSEMBLY_MEMORY", "PERAM_MEMORY").map(PathBuf::from)
    }) {
        config.memory_path = p;
    }
    config.agent_id = ensembly_agents::env_alias::resolve_agent_id(cli.agent_id);
    eprintln!(
        "ensembly-mcp listening stdio memory={:?} agent={}",
        config.memory_path, config.agent_id
    );
    serve_mcp(config)
}
