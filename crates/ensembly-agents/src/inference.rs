//! InferenceProvider — pluggable scoring/summary for reflection.
//! Default is deterministic Jaccard. External hosts (Grok MCP/ACP, opencode,
//! Ollama, pi) plug in as adapters; never hard-depend on them.

use ensembly_memory::{CrdtDocument, Reflection};
use serde::{Deserialize, Serialize};

/// Named backends selectable via `ENSEMBLY_INFERENCE` (alias `PERAM_INFERENCE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum InferenceBackend {
    #[default]
    Deterministic,
    GrokMcp,
    GrokAcp,
    OpencodeAcp,
    Ollama,
}

impl InferenceBackend {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "grok-mcp" => Self::GrokMcp,
            "grok-acp" => Self::GrokAcp,
            "opencode-acp" => Self::OpencodeAcp,
            "ollama" => Self::Ollama,
            _ => Self::Deterministic,
        }
    }

    pub fn from_env() -> Self {
        crate::env_alias::env_alias("ENSEMBLY_INFERENCE", "PERAM_INFERENCE")
            .map(|v| Self::parse(&v))
            .unwrap_or_default()
    }
}

/// Optional augmentation of a reflection summary. Returning `Err` means
/// "fall back to deterministic summary" — never fail the control op.
pub trait InferenceProvider: Send + Sync {
    fn name(&self) -> &'static str;

    /// Optionally rewrite/enrich the reflection summary. `Ok(None)` = keep
    /// the deterministic summary unchanged. `Err` = warn + keep deterministic.
    fn enrich_summary(
        &self,
        doc: &CrdtDocument,
        reflection: &Reflection,
    ) -> Result<Option<String>, String>;
}

/// Always-available oracle. Zero network.
#[derive(Debug, Default, Clone)]
pub struct DeterministicProvider;

impl InferenceProvider for DeterministicProvider {
    fn name(&self) -> &'static str {
        "deterministic"
    }

    fn enrich_summary(
        &self,
        _doc: &CrdtDocument,
        reflection: &Reflection,
    ) -> Result<Option<String>, String> {
        // Deterministic path already wrote the summary; leave it.
        let _ = reflection;
        Ok(None)
    }
}

/// Resolve a provider for the selected backend. Unavailable adapters return
/// DeterministicProvider so reflect never blocks on missing hosts.
pub fn resolve_provider(backend: InferenceBackend) -> Box<dyn InferenceProvider> {
    match backend {
        InferenceBackend::Deterministic => Box::new(DeterministicProvider),
        // Stub adapters: log intent, stay deterministic until host wiring lands.
        InferenceBackend::GrokMcp
        | InferenceBackend::GrokAcp
        | InferenceBackend::OpencodeAcp
        | InferenceBackend::Ollama => Box::new(StubProvider { backend }),
    }
}

struct StubProvider {
    backend: InferenceBackend,
}

impl InferenceProvider for StubProvider {
    fn name(&self) -> &'static str {
        match self.backend {
            InferenceBackend::GrokMcp => "grok-mcp",
            InferenceBackend::GrokAcp => "grok-acp",
            InferenceBackend::OpencodeAcp => "opencode-acp",
            InferenceBackend::Ollama => "ollama",
            InferenceBackend::Deterministic => "deterministic",
        }
    }

    fn enrich_summary(
        &self,
        _doc: &CrdtDocument,
        _reflection: &Reflection,
    ) -> Result<Option<String>, String> {
        Err(format!(
            "{} adapter not yet wired — falling back to deterministic (see SN-8 P2b/P2c)",
            self.name()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_backends() {
        assert_eq!(InferenceBackend::parse("grok-mcp"), InferenceBackend::GrokMcp);
        assert_eq!(InferenceBackend::parse("GROK-ACP"), InferenceBackend::GrokAcp);
        assert_eq!(InferenceBackend::parse("nope"), InferenceBackend::Deterministic);
    }

    #[test]
    fn stub_falls_back() {
        let p = resolve_provider(InferenceBackend::GrokMcp);
        assert_eq!(p.name(), "grok-mcp");
        let err = p
            .enrich_summary(
                &CrdtDocument::new("t"),
                &Reflection {
                    timestamp: chrono::Utc::now(),
                    trajectory_length: 0,
                    coherence: 0.5,
                    active_goals: 0,
                    known_skills: 0,
                    new_patterns: 0,
                    new_skills: vec![],
                    goal_proposals: vec![],
                    summary: "x".into(),
                },
            )
            .unwrap_err();
        assert!(err.contains("falling back"));
    }
}
