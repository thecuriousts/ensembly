//! ensembly-agents — Eagle satellites for inference, MCP export, Grok ACP helpers.
//!
//! Kernel (`ensembly-kernel`) remains control Source of Truth. This crate is the
//! protocol / learning-augmentation satellite: optional InferenceProvider
//! adapters, sync Model Context Protocol server for Grok/Cursor, and
//! DelegationBackend stubs for Human-Out-Of-The-Loop digital hands.
//!
//! Research bet (2026-07): official xAI docs — register MCP via `grok mcp add`
//! / `.grok/config.toml`; ACP via `grok agent stdio`; fast inference via
//! `grok -p --output-format json`. Export `ensembly-mcp`; never hard-dep Ollama.

pub mod delegation;
pub mod grok;
pub mod inference;
pub mod mcp_server;
pub mod rpc;

pub use delegation::{
    resolve_delegate, DelegateBackend, DelegationBackend, LocalOnlyDelegate,
};
pub use grok::{
    acp_initialize_params, acp_session_new_params, grok_acp_argv, grok_headless_argv,
    project_mcp_toml_snippet, register_mcp_shell_hint, GROK_ACP_ARGS, GROK_SCRIPT_FLAGS,
};
pub use inference::{
    resolve_provider, DeterministicProvider, InferenceBackend, InferenceProvider,
};
pub use mcp_server::{serve as serve_mcp, McpServeConfig, PROTOCOL_VERSION};

pub fn agents_version() -> &'static str {
    "ensembly-agents 0.1.0 inference-provider mcp-export grok-cli-official"
}
