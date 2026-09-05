//! UncertaintyDive — process architecture for epistemic emptiness.
//!
//! Prior → cheap Probe → Simulate (CP+P) → Score → ActOrAsk (HITL only at Auth/Physical).
//! Pure: never mutates graph/runtime. Trauma guards are explicit on the report.
//!
//! Law: Claim-via-CP for HOOTL digital; Auth/Physical black holes never auto-act.
//! See docs/thinking/uncertainty-ocean-quest.md.

use serde::{Deserialize, Serialize};

use crate::critical_path::CriticalPathReport;
use crate::graph::{DepGraph, GateKind, TaskRealm, TaskStatus};

pub const DIVE_IR_VERSION: u32 = 1;
pub const DEFAULT_PROBE_BUDGET: u32 = 3;

/// Named process steps (the “fastest way to think” rope into the ocean).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiveStep {
    Prior,
    Probe,
    Simulate,
    Score,
    ActOrAsk,
}

/// How empty is this node’s knowledge?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicClass {
    /// Low relative duration span; safe to treat as known work.
    Known,
    /// Wide PERT span — sparse knowledge, worth a cheap probe.
    Sparse,
    /// Degenerate / missing structure (zero expected, or extreme span).
    Empty,
    /// Authorization gate — HITL only (black hole).
    AuthBlackHole,
    /// Body-world beacon — HITL only.
    PhysicalBeacon,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiveCandidate {
    pub id: String,
    pub title: String,
    pub class: EpistemicClass,
    /// Higher = more epistemic emptiness / more value to probe (or escalate).
    pub uncertainty_score: f64,
    pub on_critical_path: bool,
    pub recommended: DiveStep,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraumaGuards {
    pub probe_budget: u32,
    pub refuse_auto_auth: bool,
    pub refuse_auto_physical: bool,
    pub claim_via_cp_only: bool,
    pub one_hootl_step_per_tick: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiveSimulate {
    pub cp_path: Vec<String>,
    pub length_expected: f64,
    pub pert_sigma: f64,
    pub monte_carlo_p90: Option<f64>,
    pub explain: String,
}

/// Inspectable dive plan — the control-plane answer to “jump without trauma.”
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiveReport {
    pub version: u32,
    pub process: Vec<DiveStep>,
    pub trauma_guards: TraumaGuards,
    pub simulate: DiveSimulate,
    pub candidates: Vec<DiveCandidate>,
    /// Cheapest high-uncertainty digital HOOTL on CP (probe target).
    pub next_probe: Option<DiveCandidate>,
    /// Auth black hole (ask HITL).
    pub next_auth: Option<DiveCandidate>,
    /// Physical beacon (ask HITL).
    pub next_physical: Option<DiveCandidate>,
    pub coach_line: String,
}

fn relative_span(optimistic: f64, likely: f64, pessimistic: f64) -> f64 {
    let denom = likely.max(1.0);
    ((pessimistic - optimistic) / denom).max(0.0)
}

fn classify_node(
    graph: &DepGraph,
    report: &CriticalPathReport,
    id: &str,
) -> Option<DiveCandidate> {
    let n = graph.nodes.get(id)?;
    if n.status == TaskStatus::Done {
        return None;
    }
    let on_cp = report.path.iter().any(|p| p == id);
    let span = relative_span(n.duration.optimistic, n.duration.likely, n.duration.pessimistic);
    let expected = n.duration.expected();

    let (class, score, recommended, reason) = match n.gate {
        GateKind::Auth => (
            EpistemicClass::AuthBlackHole,
            1000.0,
            DiveStep::ActOrAsk,
            "Auth black hole — wait only for permission (HITL)".to_string(),
        ),
        GateKind::Physical => (
            EpistemicClass::PhysicalBeacon,
            900.0,
            DiveStep::ActOrAsk,
            "Physical beacon — body-world pickup (HITL)".to_string(),
        ),
        GateKind::None => {
            if expected <= 0.0 || span >= 3.0 {
                (
                    EpistemicClass::Empty,
                    80.0 + span * 10.0 + if on_cp { 15.0 } else { 0.0 },
                    if on_cp && n.realm == TaskRealm::Digital {
                        DiveStep::Probe
                    } else {
                        DiveStep::Score
                    },
                    format!("Epistemic empty (span={span:.2}, E≈{expected:.1}) — probe or rescore"),
                )
            } else if span >= 1.0 {
                (
                    EpistemicClass::Sparse,
                    40.0 + span * 10.0 + if on_cp { 20.0 } else { 0.0 },
                    if on_cp && n.realm == TaskRealm::Digital {
                        DiveStep::Probe
                    } else {
                        DiveStep::Score
                    },
                    format!("Sparse knowledge (PERT span={span:.2}) — cheap probe if on CP"),
                )
            } else {
                (
                    EpistemicClass::Known,
                    5.0 + if on_cp { 5.0 } else { 0.0 },
                    DiveStep::Score,
                    format!("Known-enough (span={span:.2}) — schedule via CP score"),
                )
            }
        }
    };

    Some(DiveCandidate {
        id: n.id.clone(),
        title: n.title.clone(),
        class,
        uncertainty_score: score,
        on_critical_path: on_cp,
        recommended,
        reason,
    })
}

/// Plan a trauma-light dive from current G + CP+P.
///
/// Does not mutate state. `probe_budget` is advisory (surfaced on guards).
pub fn plan_dive(
    graph: &DepGraph,
    report: &CriticalPathReport,
    probe_budget: u32,
) -> DiveReport {
    let mut candidates: Vec<DiveCandidate> = graph
        .nodes
        .keys()
        .filter_map(|id| classify_node(graph, report, id))
        .collect();
    candidates.sort_by(|a, b| {
        b.uncertainty_score
            .partial_cmp(&a.uncertainty_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    let next_auth = candidates
        .iter()
        .find(|c| c.class == EpistemicClass::AuthBlackHole)
        .cloned();
    let next_physical = candidates
        .iter()
        .find(|c| c.class == EpistemicClass::PhysicalBeacon)
        .cloned();
    let next_probe = candidates
        .iter()
        .find(|c| {
            c.on_critical_path
                && c.recommended == DiveStep::Probe
                && matches!(
                    c.class,
                    EpistemicClass::Sparse | EpistemicClass::Empty
                )
                && graph
                    .nodes
                    .get(&c.id)
                    .map(|n| n.realm == TaskRealm::Digital && n.gate == GateKind::None)
                    .unwrap_or(false)
        })
        .cloned();

    let coach_line = if next_auth.is_some() {
        "Black hole ahead: clear the auth gate before more digital thrash.".into()
    } else if let Some(ref p) = next_probe {
        format!(
            "Dive: probe `{}` (budget {}) — one tick, then rescore.",
            p.id, probe_budget
        )
    } else if next_physical.is_some() {
        "Ocean is calm digitally — pick up the physical beacon.".into()
    } else {
        "No sparse CP digital — run status/tick or load richer priors.".into()
    };

    DiveReport {
        version: DIVE_IR_VERSION,
        process: vec![
            DiveStep::Prior,
            DiveStep::Probe,
            DiveStep::Simulate,
            DiveStep::Score,
            DiveStep::ActOrAsk,
        ],
        trauma_guards: TraumaGuards {
            probe_budget,
            refuse_auto_auth: true,
            refuse_auto_physical: true,
            claim_via_cp_only: true,
            one_hootl_step_per_tick: true,
        },
        simulate: DiveSimulate {
            cp_path: report.path.clone(),
            length_expected: report.length_expected,
            pert_sigma: report.pert_sigma,
            monte_carlo_p90: report
                .monte_carlo
                .as_ref()
                .map(|m| m.p90),
            explain: report.explain.clone(),
        },
        candidates,
        next_probe,
        next_auth,
        next_physical,
        coach_line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::critical_path::compute_critical_path;
    use crate::graph::{DepGraph, DurationEstimate, GateKind, TaskNode, TaskRealm, TaskStatus};
    // Utc not needed for node helpers

    fn node(
        id: &str,
        realm: TaskRealm,
        gate: GateKind,
        o: f64,
        l: f64,
        p: f64,
    ) -> TaskNode {
        TaskNode {
            id: id.into(),
            title: id.into(),
            realm,
            status: TaskStatus::Open,
            gate,
            duration: DurationEstimate {
                optimistic: o,
                likely: l,
                pessimistic: p,
            },
            urgency: 1,
            importance: 1,
            area: None,
            kind: None,
            depends_on: vec![],
            claimed_by: None,
            deadline_at: None,
        }
    }

    #[test]
    fn dive_flags_auth_as_black_hole_and_sparse_cp_as_probe() {
        let mut g = DepGraph::new();
        // Wide span digital on CP
        g.upsert_node(node(
            "triage",
            TaskRealm::Digital,
            GateKind::None,
            5.0,
            30.0,
            120.0,
        ));
        g.upsert_node(node(
            "pay",
            TaskRealm::Digital,
            GateKind::Auth,
            10.0,
            15.0,
            20.0,
        ));
        g.add_edge("triage", "pay").unwrap();
        let cp = compute_critical_path(&g, 0).unwrap();
        let dive = plan_dive(&g, &cp, DEFAULT_PROBE_BUDGET);

        assert_eq!(dive.version, DIVE_IR_VERSION);
        assert!(dive.trauma_guards.refuse_auto_auth);
        assert!(dive.next_auth.as_ref().unwrap().id == "pay");
        assert_eq!(
            dive.next_auth.as_ref().unwrap().class,
            EpistemicClass::AuthBlackHole
        );
        let probe = dive.next_probe.expect("sparse CP digital probe");
        assert_eq!(probe.id, "triage");
        assert!(matches!(
            probe.class,
            EpistemicClass::Sparse | EpistemicClass::Empty
        ));
        assert_eq!(probe.recommended, DiveStep::Probe);
    }

    #[test]
    fn known_narrow_span_is_not_probe_target() {
        let mut g = DepGraph::new();
        g.upsert_node(node(
            "easy",
            TaskRealm::Digital,
            GateKind::None,
            9.0,
            10.0,
            11.0,
        ));
        let cp = compute_critical_path(&g, 0).unwrap();
        let dive = plan_dive(&g, &cp, 3);
        assert!(dive.next_probe.is_none());
        let c = dive.candidates.iter().find(|c| c.id == "easy").unwrap();
        assert_eq!(c.class, EpistemicClass::Known);
    }
}
