//! Ops DB + episodic memory path law (CLI and MCP share this).

mod config;
mod resolve;

pub use config::{config_file_path, load_paths_data_dir, ConfigError};
pub use resolve::{
    env_alias, resolve_data_paths, resolve_memory_path, resolve_memory_path_at, resolve_ops_db,
    resolve_ops_db_at, DataPaths, DEFAULT_AGENT_ID, DEFAULT_MEMORY_PATH, DEFAULT_OPS_PATH,
    DEFAULT_OPS_PRIVATE_PATH, LEGACY_AGENT_ID, LEGACY_MEMORY_PATH, LEGACY_OPS_PATH,
    LEGACY_OPS_PRIVATE_PATH,
};
