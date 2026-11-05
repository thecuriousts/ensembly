//! Domain-agnostic reconciler: detect owner/copy mismatch, resolve conflict states.

use crate::record::{AggregateId, ConflictState, Source, StockholmTime, TextForm, Version};
use crate::Checked;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerSnapshot {
    pub aggregate: AggregateId,
    pub version: Version,
    pub text: TextForm,
    pub at: StockholmTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopySnapshot {
    pub source: Source,
    pub aggregate: AggregateId,
    pub version: Version,
    pub text: TextForm,
    pub at: StockholmTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conflict {
    pub fact: AggregateId,
    pub field: TextForm,
    pub values: Vec<ConflictValue>,
    pub state: ConflictState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConflictValue {
    pub source: Source,
    pub at: StockholmTime,
    pub text: TextForm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictAction {
    Explain,
    ResolveByOwner,
    WaiveByHuman,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IllegalConflictTransition {
    pub from: ConflictState,
    pub action: ConflictAction,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileError {
    SilentMergeRefused,
    AggregateMismatch,
}

impl std::fmt::Display for ReconcileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReconcileError::SilentMergeRefused => write!(f, "silent merge refused"),
            ReconcileError::AggregateMismatch => write!(f, "copy aggregate id mismatch"),
        }
    }
}

const FACT_FIELD: &str = "text";

/// Compare owner to remote copies. Returns [`Conflict`] in `Detected` when values disagree.
/// Never mutates inputs and never picks a winning copy (no last-write-wins).
pub fn reconcile(owner: &OwnerSnapshot, copies: &[CopySnapshot]) -> Option<Conflict> {
    let mut values = vec![ConflictValue {
        source: Source::Owner,
        at: owner.at.clone(),
        text: owner.text.clone(),
    }];
    for copy in copies {
        if copy.aggregate != owner.aggregate {
            continue;
        }
        if copy.text == owner.text && copy.version == owner.version {
            continue;
        }
        values.push(ConflictValue {
            source: copy.source,
            at: copy.at.clone(),
            text: copy.text.clone(),
        });
    }
    if values.len() <= 1 {
        return None;
    }
    Some(Conflict {
        fact: owner.aggregate.clone(),
        field: TextForm(FACT_FIELD.into()),
        values,
        state: ConflictState::Detected,
    })
}

/// Explicit merge entry point — always refused (conflicts stay open until resolve/waive).
pub fn merge_copy_into_owner(
    _owner: &mut OwnerSnapshot,
    _copy: &CopySnapshot,
) -> Result<(), ReconcileError> {
    Err(ReconcileError::SilentMergeRefused)
}

pub fn apply_conflict_action(
    mut conflict: Conflict,
    action: ConflictAction,
) -> Result<Conflict, IllegalConflictTransition> {
    let err = |reason: &'static str| IllegalConflictTransition {
        from: conflict.state,
        action,
        reason,
    };
    conflict.state = match (conflict.state, action) {
        (ConflictState::Detected, ConflictAction::Explain) => ConflictState::Explained,
        (ConflictState::Detected, ConflictAction::ResolveByOwner) => ConflictState::ResolvedByOwner,
        (ConflictState::Detected, ConflictAction::WaiveByHuman) => ConflictState::WaivedByHuman,
        (ConflictState::Explained, ConflictAction::ResolveByOwner) => {
            ConflictState::ResolvedByOwner
        }
        (ConflictState::Explained, ConflictAction::WaiveByHuman) => ConflictState::WaivedByHuman,
        (ConflictState::ResolvedByOwner | ConflictState::WaivedByHuman, _) => {
            return Err(err("terminal state"));
        }
        (ConflictState::Explained, ConflictAction::Explain) => {
            return Err(err("already explained"));
        }
    };
    Ok(conflict)
}

impl Checked for OwnerSnapshot {
    fn check(&self) -> Result<(), String> {
        StockholmTime::new(self.at.as_str())?;
        if self.text.0.is_empty() {
            return Err("text must not be empty".into());
        }
        Ok(())
    }
}

impl Checked for CopySnapshot {
    fn check(&self) -> Result<(), String> {
        StockholmTime::new(self.at.as_str())?;
        if self.text.0.is_empty() {
            return Err("text must not be empty".into());
        }
        Ok(())
    }
}

pub fn is_terminal(state: ConflictState) -> bool {
    matches!(
        state,
        ConflictState::ResolvedByOwner | ConflictState::WaivedByHuman
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Checked;

    fn t(s: &str) -> StockholmTime {
        StockholmTime::new(s).unwrap()
    }

    fn owner_submitted() -> OwnerSnapshot {
        OwnerSnapshot {
            aggregate: AggregateId("agg://example.test/cobalt/1".into()),
            version: Version(2),
            text: TextForm("submitted".into()),
            at: t("2026-10-26T19:00:00+01:00"),
        }
    }

    fn stale_copy() -> CopySnapshot {
        CopySnapshot {
            source: Source::Human,
            aggregate: AggregateId("agg://example.test/cobalt/1".into()),
            version: Version(1),
            text: TextForm("intended".into()),
            at: t("2026-10-25T12:00:00+01:00"),
        }
    }

    #[test]
    fn planted_mismatch_emits_conflict_detected() {
        let owner = owner_submitted();
        let c = reconcile(&owner, &[stale_copy()]).expect("expected conflict");
        assert_eq!(c.state, ConflictState::Detected);
        assert_eq!(c.fact.0, "agg://example.test/cobalt/1");
        assert_eq!(c.values.len(), 2);
    }

    #[test]
    fn resolve_and_waive_reach_terminals() {
        let owner = owner_submitted();
        let mut c = reconcile(&owner, &[stale_copy()]).unwrap();
        c = apply_conflict_action(c, ConflictAction::Explain).unwrap();
        assert_eq!(c.state, ConflictState::Explained);
        c = apply_conflict_action(c, ConflictAction::ResolveByOwner).unwrap();
        assert!(is_terminal(c.state));
        let owner2 = owner_submitted();
        let c2 = reconcile(&owner2, &[stale_copy()]).unwrap();
        let c2 = apply_conflict_action(c2, ConflictAction::WaiveByHuman).unwrap();
        assert_eq!(c2.state, ConflictState::WaivedByHuman);
        assert!(apply_conflict_action(c2, ConflictAction::Explain).is_err());
    }

    #[test]
    fn no_silent_merge_of_disagreeing_copy() {
        let mut owner = owner_submitted();
        let copy = stale_copy();
        assert!(reconcile(&owner, &[copy.clone()]).is_some());
        assert_eq!(
            merge_copy_into_owner(&mut owner, &copy),
            Err(ReconcileError::SilentMergeRefused)
        );
        assert_eq!(owner.text.0, "submitted");
        assert_ne!(owner.text, copy.text);
    }

    #[test]
    fn matching_copy_returns_none() {
        let owner = owner_submitted();
        let agree = CopySnapshot {
            source: Source::CosCopy,
            aggregate: owner.aggregate.clone(),
            version: owner.version,
            text: owner.text.clone(),
            at: owner.at.clone(),
        };
        assert!(reconcile(&owner, &[agree]).is_none());
    }

    #[test]
    fn scenario_c_fixture_mismatch() {
        #[derive(Deserialize)]
        struct ScenarioC {
            owner: OwnerSnapshot,
            copies: Vec<CopySnapshot>,
        }
        let raw = include_str!("../fixtures/scenarios/c-owner-copy-mismatch.json");
        let sc: ScenarioC = serde_json::from_str(raw).expect("scenario c");
        sc.owner.check().unwrap();
        for c in &sc.copies {
            c.check().unwrap();
        }
        let conflict = reconcile(&sc.owner, &sc.copies).expect("scenario c conflict");
        assert_eq!(conflict.state, ConflictState::Detected);
        assert!(sc
            .copies
            .iter()
            .any(|c| c.text.0.contains("intended") || c.version.0 < sc.owner.version.0));
    }
}
