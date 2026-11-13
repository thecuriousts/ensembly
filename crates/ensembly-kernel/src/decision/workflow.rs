//! Propose-and-wait statechart, written for this crate.
//!
//! Silence (timeout with no operator signal) records `expired` and takes
//! no action. A late confirm after expiry is refused; a new proposal needs
//! a new rid. The model never writes a posting status.

use ensembly_schema::{DecisionEvent, DecisionEventType, ModelDecision};

/// Live phase of one proposal, derived only from its event log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Waiting,
    Confirmed,
    Overridden,
    Expired,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    Confirm,
    Override { label: ModelDecision },
    Expire,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    AlreadyClosed { phase: Phase },
    LateSignal { phase: Phase },
    SilenceCannotAct,
    InvalidOverride,
    EmptyLog,
}

impl std::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransitionError::AlreadyClosed { phase } => {
                write!(f, "proposal already closed ({phase:?})")
            }
            TransitionError::LateSignal { phase } => {
                write!(f, "late signal after {phase:?}; open a new proposal")
            }
            TransitionError::SilenceCannotAct => {
                write!(f, "silence never becomes an action")
            }
            TransitionError::InvalidOverride => {
                write!(f, "override must be apply or skip")
            }
            TransitionError::EmptyLog => write!(f, "no events for this rid"),
        }
    }
}

/// Phase of a rid from its events, in log order.
pub fn phase(events: &[DecisionEvent]) -> Result<Phase, TransitionError> {
    if events.is_empty() {
        return Err(TransitionError::EmptyLog);
    }
    let mut phase = match events[0].event_type {
        DecisionEventType::Proposed => Phase::Waiting,
        DecisionEventType::Skipped => Phase::Skipped,
        DecisionEventType::Confirmed
        | DecisionEventType::Overridden
        | DecisionEventType::Expired => {
            return Err(TransitionError::EmptyLog);
        }
    };
    for ev in events.iter().skip(1) {
        phase = apply(phase, event_signal(ev)?)?;
    }
    Ok(phase)
}

fn event_signal(ev: &DecisionEvent) -> Result<Signal, TransitionError> {
    match ev.event_type {
        DecisionEventType::Confirmed => Ok(Signal::Confirm),
        DecisionEventType::Overridden => Ok(Signal::Override {
            label: ev.decision.ok_or(TransitionError::InvalidOverride)?,
        }),
        DecisionEventType::Expired => Ok(Signal::Expire),
        DecisionEventType::Proposed | DecisionEventType::Skipped => {
            Err(TransitionError::AlreadyClosed {
                phase: Phase::Waiting,
            })
        }
    }
}

/// Apply one operator or timer signal to the current phase.
///
/// `Expire` from `Waiting` is the only silence path: the next phase is
/// `Expired`, never apply.
pub fn apply(phase: Phase, signal: Signal) -> Result<Phase, TransitionError> {
    match (phase, signal) {
        (Phase::Waiting, Signal::Confirm) => Ok(Phase::Confirmed),
        (Phase::Waiting, Signal::Override { label }) => {
            if label == ModelDecision::Abstain {
                Err(TransitionError::InvalidOverride)
            } else {
                Ok(Phase::Overridden)
            }
        }
        (Phase::Waiting, Signal::Expire) => Ok(Phase::Expired),
        (Phase::Expired, Signal::Confirm | Signal::Override { .. }) => {
            Err(TransitionError::LateSignal {
                phase: Phase::Expired,
            })
        }
        (Phase::Expired, Signal::Expire) => Err(TransitionError::AlreadyClosed {
            phase: Phase::Expired,
        }),
        (phase, Signal::Expire) if phase != Phase::Waiting => {
            Err(TransitionError::AlreadyClosed { phase })
        }
        (phase, _) => Err(TransitionError::AlreadyClosed { phase }),
    }
}

/// An expired proposal produced no apply action.
pub fn silence_took_action(events: &[DecisionEvent]) -> bool {
    events.iter().any(|e| {
        e.event_type == DecisionEventType::Expired && e.decision == Some(ModelDecision::Apply)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ensembly_schema::{DecisionEvent, MODEL_VERSION, TRIAGE_QUESTION};

    fn ev(kind: DecisionEventType, decision: Option<ModelDecision>) -> DecisionEvent {
        DecisionEvent {
            event_type: kind,
            rid: "abc".into(),
            link_key: "https://jobs.example.test/posting/1".into(),
            event_time: "2026-11-13T12:00:00+01:00".into(),
            writer: "laptop-1".into(),
            model_version: Some(MODEL_VERSION.into()),
            snapshot_hash: "00".into(),
            decision,
            p_apply: None,
            reason: None,
            deadline: None,
            provenance: "synthetic-fixture".into(),
            triage_question: TRIAGE_QUESTION.into(),
        }
    }

    #[test]
    fn confirm_from_waiting() {
        let log = [ev(DecisionEventType::Proposed, Some(ModelDecision::Apply))];
        assert_eq!(phase(&log).unwrap(), Phase::Waiting);
        assert_eq!(
            apply(Phase::Waiting, Signal::Confirm).unwrap(),
            Phase::Confirmed
        );
    }

    #[test]
    fn expire_does_not_apply() {
        let next = apply(Phase::Waiting, Signal::Expire).unwrap();
        assert_eq!(next, Phase::Expired);
        let log = [
            ev(DecisionEventType::Proposed, Some(ModelDecision::Apply)),
            ev(DecisionEventType::Expired, None),
        ];
        assert_eq!(phase(&log).unwrap(), Phase::Expired);
        assert!(!silence_took_action(&log));
    }

    #[test]
    fn late_confirm_is_refused() {
        let err = apply(Phase::Expired, Signal::Confirm).unwrap_err();
        assert!(matches!(err, TransitionError::LateSignal { .. }));
    }

    #[test]
    fn double_confirm_is_closed() {
        let err = apply(Phase::Confirmed, Signal::Confirm).unwrap_err();
        assert!(matches!(err, TransitionError::AlreadyClosed { .. }));
    }
}
