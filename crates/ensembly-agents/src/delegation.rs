//! DelegationBackend — Human-Out-Of-The-Loop digital hands.
//! Grok Agent Client Protocol is the first planned adapter (x.ai ACP + reverse MCP).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DelegateBackend {
    #[default]
    None,
    GrokAcp,
    OpencodeAcp,
}

impl DelegateBackend {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "grok-acp" => Self::GrokAcp,
            "opencode-acp" => Self::OpencodeAcp,
            _ => Self::None,
        }
    }

    pub fn from_env() -> Self {
        crate::env_alias::env_alias("ENSEMBLY_DELEGATE", "PERAM_DELEGATE")
            .map(|v| Self::parse(&v))
            .unwrap_or_default()
    }
}

/// Fire → work → verify exit. No live chat with the kernel while running
/// (Eagle/satellite offline job pattern).
pub trait DelegationBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn delegate_digital(&self, task_id: &str, brief: &str) -> Result<String, String>;
}

/// No-op: HOOTL stays local AgentWorker claim/complete.
#[derive(Debug, Default)]
pub struct LocalOnlyDelegate;

impl DelegationBackend for LocalOnlyDelegate {
    fn name(&self) -> &'static str {
        "none"
    }

    fn delegate_digital(&self, _task_id: &str, _brief: &str) -> Result<String, String> {
        Err("no external delegate configured".into())
    }
}

pub fn resolve_delegate(backend: DelegateBackend) -> Box<dyn DelegationBackend> {
    match backend {
        DelegateBackend::None => Box::new(LocalOnlyDelegate),
        DelegateBackend::GrokAcp | DelegateBackend::OpencodeAcp => {
            Box::new(StubDelegate { backend })
        }
    }
}

struct StubDelegate {
    backend: DelegateBackend,
}

impl DelegationBackend for StubDelegate {
    fn name(&self) -> &'static str {
        match self.backend {
            DelegateBackend::GrokAcp => "grok-acp",
            DelegateBackend::OpencodeAcp => "opencode-acp",
            DelegateBackend::None => "none",
        }
    }

    fn delegate_digital(&self, task_id: &str, _brief: &str) -> Result<String, String> {
        Err(format!(
            "{} delegate not yet wired for {task_id} — local AgentWorker remains SoT for claim",
            self.name()
        ))
    }
}
