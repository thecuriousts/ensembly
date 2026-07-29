//! `peram-mcp` — read-only Model Context Protocol server for episodic memory.
//!
//! Register with Grok/Cursor. Example:
//! ```text
//! cargo run -p peram-agents --bin peram-mcp
//! # or after install: peram-mcp
//! ```
//! Env: `PERAM_MEMORY` (default data/local/peram-memory.json), `PERAM_AGENT_ID`.

use clap::Parser;
use peram_agents::{serve_mcp, McpServeConfig};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "peram-mcp", about = "peram read-only MCP server (memory tools for Grok/Cursor)")]
struct Cli {
    #[arg(long, env = "PERAM_MEMORY")]
    memory: Option<PathBuf>,
    #[arg(long, env = "PERAM_AGENT_ID", default_value = "peram-swarm")]
    agent_id: String,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut config = McpServeConfig::default();
    if let Some(p) = cli.memory {
        config.memory_path = p;
    }
    config.agent_id = cli.agent_id;
    eprintln!(
        "PERAM_MCP listening stdio memory={:?} agent={}",
        config.memory_path, config.agent_id
    );
    serve_mcp(config)
}
