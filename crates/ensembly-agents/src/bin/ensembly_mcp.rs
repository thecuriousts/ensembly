//! `ensembly-mcp` — Model Context Protocol server for episodic memory and record Commit/View.
//!
//! Register with Grok/Cursor. Example:
//! ```text
//! cargo run -p ensembly-agents --bin ensembly-mcp
//! # or after install: ensembly-mcp
//! ```
//! Env: `ENSEMBLY_MEMORY` / `PERAM_MEMORY` override; else same path law as the CLI (`paths.data_dir` or cwd).
//! `ENSEMBLY_AGENT_ID` / `PERAM_AGENT_ID` (new stores: ensembly-swarm).

use clap::Parser;
use ensembly_agents::{serve_mcp, McpServeConfig};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    about = "MCP server: memory tools plus record commit and view"
)]
struct Cli {
    #[arg(long)]
    memory: Option<PathBuf>,
    #[arg(long)]
    agent_id: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut config = McpServeConfig::from_env(cli.memory)?;
    if let Some(id) = cli.agent_id {
        config.agent_id = ensembly_agents::env_alias::resolve_agent_id(Some(id));
    }
    eprintln!(
        "ensembly-mcp listening stdio memory={:?} agent={}",
        config.memory_path, config.agent_id
    );
    serve_mcp(config)
}
