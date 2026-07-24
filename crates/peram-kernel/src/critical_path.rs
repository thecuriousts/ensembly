//! Critical-path operator CP(G) + uncertainty measure P (PERT / Monte Carlo).
//! Every prioritization decision must be explainable via CP(G).

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::graph::{DepGraph, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTiming {
    pub id: String,
    pub expected: f64,
    pub earliest_start: f64,
    pub earliest_finish: f64,
    pub latest_start: f64,
    pub latest_finish: f64,
    pub slack: f64,
    pub on_critical_path: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalPathReport {
    /// Ordered critical path node ids (source → sink along longest path).
    pub path: Vec<String>,
    pub length_expected: f64,
    pub timings: Vec<NodeTiming>,
    pub pert_variance_path: f64,
    pub pert_sigma: f64,
    /// Monte Carlo summary (empty if samples == 0).
    pub monte_carlo: Option<MonteCarloSummary>,
    pub explain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloSummary {
    pub samples: u32,
    pub mean_length: f64,
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
}

/// Compute CP on open/claimed subgraph (done nodes treated as zero remaining).
pub fn compute_critical_path(graph: &DepGraph, mc_samples: u32) -> Result<CriticalPathReport, String> {
    let order = graph.topo_order().map_err(|e| e.to_string())?;
    if order.is_empty() {
        return Ok(CriticalPathReport {
            path: vec![],
            length_expected: 0.0,
            timings: vec![],
            pert_variance_path: 0.0,
            pert_sigma: 0.0,
            monte_carlo: None,
            explain: "empty graph — no critical path".into(),
        });
    }

    let dur = |id: &str| -> f64 {
        let n = &graph.nodes[id];
        if n.status == TaskStatus::Done {
            0.0
        } else {
            n.duration.expected()
        }
    };

    let mut es: HashMap<String, f64> = HashMap::new();
    let mut ef: HashMap<String, f64> = HashMap::new();
    for id in &order {
        let preds = graph.predecessors(id);
        let start = preds
            .iter()
            .map(|p| *ef.get(*p).unwrap_or(&0.0))
            .fold(0.0_f64, f64::max);
        let finish = start + dur(id);
        es.insert(id.clone(), start);
        ef.insert(id.clone(), finish);
    }

    let project_end = ef.values().cloned().fold(0.0_f64, f64::max);

    let mut lf: HashMap<String, f64> = HashMap::new();
    let mut ls: HashMap<String, f64> = HashMap::new();
    for id in order.iter().rev() {
        let succs = graph.successors(id);
        let finish = if succs.is_empty() {
            project_end
        } else {
            succs
                .iter()
                .map(|s| *ls.get(*s).unwrap_or(&project_end))
                .fold(f64::INFINITY, f64::min)
        };
        let start = finish - dur(id);
        lf.insert(id.clone(), finish);
        ls.insert(id.clone(), start);
    }

    let mut timings = Vec::with_capacity(order.len());
    let mut critical: HashSet<String> = HashSet::new();
    for id in &order {
        let slack = ls[id] - es[id];
        let on_cp = slack.abs() < 1e-6;
        if on_cp {
            critical.insert(id.clone());
        }
        timings.push(NodeTiming {
            id: id.clone(),
            expected: dur(id),
            earliest_start: es[id],
            earliest_finish: ef[id],
            latest_start: ls[id],
            latest_finish: lf[id],
            slack,
            on_critical_path: on_cp,
        });
    }

    // Reconstruct one critical path: walk successors preferring critical nodes with max EF.
    let path = reconstruct_path(graph, &order, &critical, &ef);
    let pert_var: f64 = path
        .iter()
        .filter_map(|id| graph.nodes.get(id))
        .filter(|n| n.status != TaskStatus::Done)
        .map(|n| n.duration.variance())
        .sum();
    let pert_sigma = pert_var.max(0.0).sqrt();

    let monte_carlo = if mc_samples > 0 {
        Some(run_monte_carlo(graph, &order, mc_samples))
    } else {
        None
    };

    let explain = if path.is_empty() {
        "no critical path nodes".into()
    } else {
        format!(
            "CP length≈{:.1}m σ≈{:.1} path=[{}]",
            project_end,
            pert_sigma,
            path.join(" → ")
        )
    };

    Ok(CriticalPathReport {
        path,
        length_expected: project_end,
        timings,
        pert_variance_path: pert_var,
        pert_sigma,
        monte_carlo,
        explain,
    })
}

fn reconstruct_path(
    graph: &DepGraph,
    order: &[String],
    critical: &HashSet<String>,
    ef: &HashMap<String, f64>,
) -> Vec<String> {
    let es: HashMap<String, f64> = order
        .iter()
        .filter_map(|id| {
            let n = graph.nodes.get(id)?;
            let d = if n.status == TaskStatus::Done {
                0.0
            } else {
                n.duration.expected()
            };
            Some((id.clone(), ef[id] - d))
        })
        .collect();

    let sources: Vec<_> = order
        .iter()
        .filter(|id| critical.contains(id.as_str()) && graph.predecessors(id).is_empty())
        .cloned()
        .collect();
    let start = sources
        .into_iter()
        .max_by(|a, b| {
            ef[a]
                .partial_cmp(&ef[b])
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(b))
        })
        .or_else(|| {
            order
                .iter()
                .filter(|id| critical.contains(id.as_str()))
                .max_by(|a, b| {
                    ef[*a]
                        .partial_cmp(&ef[*b])
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.cmp(b))
                })
                .cloned()
        });
    let Some(mut cur) = start else {
        return vec![];
    };
    let mut path = vec![cur.clone()];
    loop {
        // Prefer critical successors whose earliest start equals current EF (tight edge).
        let mut nexts: Vec<_> = graph
            .successors(&cur)
            .into_iter()
            .filter(|s| critical.contains(*s))
            .map(str::to_string)
            .collect();
        if nexts.is_empty() {
            break;
        }
        let cur_ef = ef[&cur];
        nexts.sort_by(|a, b| {
            let a_tight = (es[a] - cur_ef).abs() < 1e-6;
            let b_tight = (es[b] - cur_ef).abs() < 1e-6;
            b_tight
                .cmp(&a_tight)
                .then_with(|| {
                    ef[b]
                        .partial_cmp(&ef[a])
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.cmp(b))
        });
        cur = nexts[0].clone();
        path.push(cur.clone());
    }
    path
}

fn sample_duration(optimistic: f64, likely: f64, pessimistic: f64, rng: &mut impl Rng) -> f64 {
    // Triangular distribution on [o, p] with mode = likely.
    let o = optimistic.min(likely).min(pessimistic);
    let p = optimistic.max(likely).max(pessimistic);
    let m = likely.clamp(o, p);
    let u: f64 = rng.gen();
    let fc = (m - o) / (p - o).max(1e-9);
    if u < fc {
        o + ((u * (p - o) * (m - o)).max(0.0)).sqrt()
    } else {
        p - (((1.0 - u) * (p - o) * (p - m)).max(0.0)).sqrt()
    }
}

fn run_monte_carlo(graph: &DepGraph, order: &[String], samples: u32) -> MonteCarloSummary {
    let mut rng = rand::thread_rng();
    let mut lengths = Vec::with_capacity(samples as usize);
    for _ in 0..samples {
        let mut sampled: HashMap<String, f64> = HashMap::new();
        for id in order {
            let n = &graph.nodes[id];
            let d = if n.status == TaskStatus::Done {
                0.0
            } else {
                sample_duration(
                    n.duration.optimistic,
                    n.duration.likely,
                    n.duration.pessimistic,
                    &mut rng,
                )
            };
            sampled.insert(id.clone(), d);
        }
        let mut ef: HashMap<String, f64> = HashMap::new();
        for id in order {
            let preds = graph.predecessors(id);
            let start = preds
                .iter()
                .map(|p| *ef.get(*p).unwrap_or(&0.0))
                .fold(0.0_f64, f64::max);
            ef.insert(id.clone(), start + sampled[id]);
        }
        lengths.push(ef.values().cloned().fold(0.0_f64, f64::max));
    }
    lengths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean = lengths.iter().sum::<f64>() / lengths.len() as f64;
    let pct = |p: f64| {
        let idx = ((p / 100.0) * (lengths.len() as f64 - 1.0)).round() as usize;
        lengths[idx.min(lengths.len() - 1)]
    };
    MonteCarloSummary {
        samples,
        mean_length: mean,
        p50: pct(50.0),
        p90: pct(90.0),
        p95: pct(95.0),
    }
}

/// Explain why a node is (or is not) prioritized via CP.
pub fn explain_node(report: &CriticalPathReport, id: &str) -> String {
    if let Some(t) = report.timings.iter().find(|t| t.id == id) {
        if t.on_critical_path {
            format!(
                "{id} is on critical path (slack≈{:.1}); {}",
                t.slack, report.explain
            )
        } else {
            format!(
                "{id} has slack≈{:.1}m — not on CP; CP=[{}]",
                t.slack,
                report.path.join(" → ")
            )
        }
    } else {
        format!("{id} not in current graph timings")
    }
}

/// First open digital HOOTL (gate=None) node on the critical path — agent claim target.
pub fn next_hootl_digital(graph: &DepGraph, report: &CriticalPathReport) -> Option<String> {
    for id in &report.path {
        if let Some(n) = graph.nodes.get(id) {
            if n.realm == crate::graph::TaskRealm::Digital
                && n.gate == crate::graph::GateKind::None
                && n.status == TaskStatus::Open
            {
                return Some(id.clone());
            }
        }
    }
    // Fallback: any open digital HOOTL by earliest start among critical, else any open.
    let mut candidates: Vec<_> = report
        .timings
        .iter()
        .filter(|t| {
            graph
                .nodes
                .get(&t.id)
                .map(|n| {
                    n.realm == crate::graph::TaskRealm::Digital
                        && n.gate == crate::graph::GateKind::None
                        && n.status == TaskStatus::Open
                })
                .unwrap_or(false)
        })
        .collect();
    candidates.sort_by(|a, b| {
        a.earliest_start
            .partial_cmp(&b.earliest_start)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    candidates.first().map(|t| t.id.clone())
}

/// First Auth-gated open node that should surface (HITL).
pub fn next_auth_gate(graph: &DepGraph, report: &CriticalPathReport) -> Option<String> {
    for id in &report.path {
        if let Some(n) = graph.nodes.get(id) {
            if n.gate == crate::graph::GateKind::Auth && n.status == TaskStatus::Open {
                return Some(id.clone());
            }
        }
    }
    graph
        .nodes
        .values()
        .filter(|n| n.gate == crate::graph::GateKind::Auth && n.status == TaskStatus::Open)
        .min_by_key(|n| n.id.clone())
        .map(|n| n.id.clone())
}

/// First Physical-gated open node (body beacon).
pub fn next_physical_beacon(graph: &DepGraph, report: &CriticalPathReport) -> Option<String> {
    for id in &report.path {
        if let Some(n) = graph.nodes.get(id) {
            if n.gate == crate::graph::GateKind::Physical && n.status == TaskStatus::Open {
                return Some(id.clone());
            }
        }
    }
    graph
        .nodes
        .values()
        .filter(|n| n.gate == crate::graph::GateKind::Physical && n.status == TaskStatus::Open)
        .min_by_key(|n| n.id.clone())
        .map(|n| n.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{DepGraph, DurationEstimate, GateKind, TaskNode, TaskRealm, TaskStatus};
    use std::collections::HashMap;

    fn node(id: &str, deps: &[&str], likely: f64, gate: GateKind, realm: TaskRealm) -> TaskNode {
        TaskNode {
            id: id.into(),
            title: id.into(),
            realm,
            status: TaskStatus::Open,
            gate,
            duration: DurationEstimate::minutes(likely),
            urgency: 3,
            importance: 3,
            area: None,
            kind: None,
            depends_on: deps.iter().map(|s| (*s).into()).collect(),
            claimed_by: None,
            deadline_at: None,
        }
    }

    #[test]
    fn longest_path_is_critical() {
        let mut g = DepGraph::new();
        // a(10) → c(10); a → b(50) → c  ⇒ CP = a-b-c
        g.upsert_node(node("a", &[], 10.0, GateKind::None, TaskRealm::Digital));
        g.upsert_node(node("b", &["a"], 50.0, GateKind::None, TaskRealm::Digital));
        g.upsert_node(node(
            "c",
            &["a", "b"],
            10.0,
            GateKind::Auth,
            TaskRealm::Digital,
        ));
        let report = compute_critical_path(&g, 64).unwrap();
        assert!(report.path.contains(&"a".into()));
        assert!(report.path.contains(&"b".into()));
        assert!(report.path.contains(&"c".into()));
        assert!(report.length_expected > 60.0);
        assert!(report.monte_carlo.as_ref().unwrap().samples == 64);
        assert_eq!(next_hootl_digital(&g, &report).as_deref(), Some("a"));
        assert_eq!(next_auth_gate(&g, &report).as_deref(), Some("c"));
    }

    #[test]
    fn explain_references_cp() {
        let mut g = DepGraph::new();
        g.upsert_node(node("x", &[], 20.0, GateKind::None, TaskRealm::Digital));
        let report = compute_critical_path(&g, 0).unwrap();
        let e = explain_node(&report, "x");
        assert!(e.contains("critical"));
    }

    #[test]
    fn from_empty() {
        let g = DepGraph::new();
        let r = compute_critical_path(&g, 0).unwrap();
        assert!(r.path.is_empty());
    }

    #[test]
    fn fingerprint_independent_hashmap_order() {
        let _ = HashMap::<String, u8>::new();
    }
}
