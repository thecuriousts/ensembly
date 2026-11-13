//! Local fact classification and conflict checks.
//!
//! Replaces the Ollama adapter stub with a llama.cpp (preferred) runtime on
//! the operator machine (mac-mini). MLX is not a production runtime until it
//! can constrain JSON with a schema (`ENSEMBLY_MLX_EXPERIMENTAL=1` overrides).
//!
//! Cloud fallback is off unless both `ENSEMBLY_CLOUD_CHECKS=1` and
//! `ENSEMBLY_CLOUD_CHECK_CMD` are set. A local failure returns that local
//! error (lock, memory, missing model or runtime, invalid JSON) and never
//! escalates. Only a successful local result below the confidence floor may
//! call the cloud, after a fail-closed redaction gate. `grok` on `PATH` is
//! never auto-detected.
//!
//! CI never needs the models. Default tests replay recorded synthetic JSON
//! under `fixtures/local-checks/`. Live model processes are opt-in via
//! `ENSEMBLY_LOCAL_MODELS=1`.

pub mod redact;
pub mod replay;
pub mod runtime;
pub mod schema;
pub mod support;

pub use replay::{fixture_root_from, RecordedSet, FIXTURE_ROOT};
pub use runtime::{
    cloud_checks_enabled, live_models_enabled, CheckFailure, CheckResult, CheckSource,
    CheckSuccess, FallbackReason, LocalCheckConfig, LocalChecker, RuntimeKind,
};
pub use schema::{ConflictClass, ConflictRelation, FactClass, FactLabel, DEFAULT_CONFIDENCE_FLOOR};

use crate::inference::{InferenceBackend, InferenceProvider};
use ensembly_memory::{CrdtDocument, Reflection};
use std::path::Path;

/// Resolve a fact/conflict checker from the process environment.
///
/// This does **not** consult `ENSEMBLY_LOCAL_MODELS`. That flag is the CLI
/// gate for spawning live model processes; library callers that want recorded
/// fixtures should use [`RecordedSet`]. Cloud fallback stays off unless both
/// `ENSEMBLY_CLOUD_CHECKS=1` and `ENSEMBLY_CLOUD_CHECK_CMD` are set in the
/// environment this reads.
pub fn resolve_checker() -> LocalChecker {
    LocalChecker::from_env()
}

/// Replay recorded synthetic outputs from `root` (CI default).
pub fn replay_checker(root: &Path) -> Result<RecordedSet, String> {
    RecordedSet::load(root)
}

/// InferenceProvider for the former Ollama slot: enrich stays deterministic
/// (this lane is classify + conflict, not summary rewrite).
pub struct LocalInferenceProvider {
    pub runtime: RuntimeKind,
}

impl Default for LocalInferenceProvider {
    fn default() -> Self {
        Self {
            runtime: RuntimeKind::LlamaCpp,
        }
    }
}

impl InferenceProvider for LocalInferenceProvider {
    fn name(&self) -> &'static str {
        "local"
    }

    fn enrich_summary(
        &self,
        _doc: &CrdtDocument,
        _reflection: &Reflection,
    ) -> Result<Option<String>, String> {
        // Fact/conflict checks are a separate API (`LocalChecker`). Summary
        // enrichment stays on the deterministic path.
        Ok(None)
    }
}

pub fn provider_for_backend(backend: InferenceBackend) -> Option<LocalInferenceProvider> {
    match backend {
        InferenceBackend::Local => Some(LocalInferenceProvider::default()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_checks::schema::{ConflictRelation, FactLabel};
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        // crate dir is crates/ensembly-agents; fixtures live at the workspace root.
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn recorded_fixtures_parse_and_match_expected_labels() {
        let set = RecordedSet::load(&repo_root().join(FIXTURE_ROOT)).unwrap();
        assert_eq!(set.classify.len(), 5, "five classify fixtures");
        assert_eq!(set.conflict.len(), 3, "three conflict fixtures");

        let stock = set.classify_by_id("classify-stock-fact").unwrap();
        assert_eq!(stock.value.label, FactLabel::Fact);
        assert!(stock.value.confidence >= DEFAULT_CONFIDENCE_FLOOR);
        assert_eq!(stock.source.runtime, RuntimeKind::Replay);

        let opinion = set.classify_by_id("classify-cheerful-opinion").unwrap();
        assert_eq!(opinion.value.label, FactLabel::Opinion);

        let instruction = set.classify_by_id("classify-move-instruction").unwrap();
        assert_eq!(instruction.value.label, FactLabel::Instruction);

        let uncertain = set.classify_by_id("classify-maybe-uncertain").unwrap();
        assert_eq!(uncertain.value.label, FactLabel::Uncertain);

        let other = set.classify_by_id("classify-ping-other").unwrap();
        assert_eq!(other.value.label, FactLabel::Other);

        let clash = set.conflict_by_id("conflict-count-clash").unwrap();
        assert_eq!(clash.value.relation, ConflictRelation::Contradicts);

        let fit = set.conflict_by_id("conflict-location-compatible").unwrap();
        assert_eq!(fit.value.relation, ConflictRelation::Compatible);

        let thin = set.conflict_by_id("conflict-busy-underspecified").unwrap();
        assert_eq!(thin.value.relation, ConflictRelation::Underspecified);
    }

    #[test]
    fn recorded_fixture_digest_matches_manifest() {
        let root = repo_root().join(FIXTURE_ROOT);
        let set = RecordedSet::load(&root).unwrap();
        let digest = set.digest().unwrap();
        let manifest = std::fs::read_to_string(root.join(replay::MANIFEST_NAME)).unwrap();
        let expected = manifest
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().next())
            .expect("manifest hash");
        assert_eq!(
            digest, expected,
            "update {FIXTURE_ROOT}/MANIFEST.sha256 after editing fixtures"
        );
    }

    #[test]
    fn live_flag_is_off_by_default() {
        // CI must not require models. The env may be set in a live shell, so we
        // only assert the helper accepts the documented values.
        let on = matches!(
            std::env::var("ENSEMBLY_LOCAL_MODELS").ok().as_deref(),
            Some("1") | Some("true") | Some("yes") | Some("on")
        );
        assert_eq!(live_models_enabled(), on);
    }

    #[test]
    fn lookup_by_statement_matches_id() {
        let set = RecordedSet::load(&repo_root().join(FIXTURE_ROOT)).unwrap();
        let by_id = set.classify_by_id("classify-stock-fact").unwrap();
        let by_text = set
            .classify("The Northbridge depot of River Co. holds 12 teal crates.")
            .unwrap();
        assert_eq!(by_id.value, by_text.value);
    }
}
