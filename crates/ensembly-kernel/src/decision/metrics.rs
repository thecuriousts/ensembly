//! Precision, accuracy, abstain rate, calibration, and Wilson intervals.
//!
//! Held-out metrics are only computed when every held-out row is labelled.
//! This module never claims accuracy on unlabelled data.

use ensembly_schema::ModelDecision;

#[derive(Debug, Clone, PartialEq)]
pub struct WilsonInterval {
    pub n: usize,
    pub successes: usize,
    pub estimate: f64,
    pub low: f64,
    pub high: f64,
    pub z: f64,
}

/// Wilson score interval for a binomial proportion (default z = 1.96, ~95%).
pub fn wilson(successes: usize, n: usize, z: f64) -> Option<WilsonInterval> {
    if n == 0 {
        return None;
    }
    let nn = n as f64;
    let p = successes as f64 / nn;
    let z2 = z * z;
    let denom = 1.0 + z2 / nn;
    let centre = (p + z2 / (2.0 * nn)) / denom;
    let margin = z * ((p * (1.0 - p) / nn) + z2 / (4.0 * nn * nn)).sqrt() / denom;
    Some(WilsonInterval {
        n,
        successes,
        estimate: p,
        low: (centre - margin).clamp(0.0, 1.0),
        high: (centre + margin).clamp(0.0, 1.0),
        z,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnsweredMetrics {
    pub n: usize,
    pub n_answered: usize,
    pub n_abstain: usize,
    pub n_apply_pred: usize,
    pub apply_precision: Option<f64>,
    pub apply_precision_wilson: Option<WilsonInterval>,
    pub accuracy_answered: Option<f64>,
    pub accuracy_wilson: Option<WilsonInterval>,
    pub abstain_rate: f64,
    pub expected_calibration_error: Option<f64>,
}

/// `y_apply[i]` is true when the label is apply. Predictions may abstain.
pub fn answered_metrics(y_apply: &[bool], preds: &[ModelDecision], p: &[f64]) -> AnsweredMetrics {
    assert_eq!(y_apply.len(), preds.len());
    assert_eq!(y_apply.len(), p.len());
    let n = y_apply.len();
    let answered: Vec<usize> = preds
        .iter()
        .enumerate()
        .filter(|(_, d)| **d != ModelDecision::Abstain)
        .map(|(i, _)| i)
        .collect();
    let apply_pred: Vec<usize> = answered
        .iter()
        .copied()
        .filter(|&i| preds[i] == ModelDecision::Apply)
        .collect();
    let prec = if apply_pred.is_empty() {
        None
    } else {
        let ok = apply_pred.iter().filter(|&&i| y_apply[i]).count();
        Some(ok as f64 / apply_pred.len() as f64)
    };
    let acc = if answered.is_empty() {
        None
    } else {
        let ok = answered
            .iter()
            .filter(|&&i| {
                (preds[i] == ModelDecision::Apply && y_apply[i])
                    || (preds[i] == ModelDecision::Skip && !y_apply[i])
            })
            .count();
        Some(ok as f64 / answered.len() as f64)
    };
    let prec_w = if apply_pred.is_empty() {
        None
    } else {
        let ok = apply_pred.iter().filter(|&&i| y_apply[i]).count();
        wilson(ok, apply_pred.len(), 1.96)
    };
    let acc_w = if answered.is_empty() {
        None
    } else {
        let ok = answered
            .iter()
            .filter(|&&i| {
                (preds[i] == ModelDecision::Apply && y_apply[i])
                    || (preds[i] == ModelDecision::Skip && !y_apply[i])
            })
            .count();
        wilson(ok, answered.len(), 1.96)
    };
    AnsweredMetrics {
        n,
        n_answered: answered.len(),
        n_abstain: n - answered.len(),
        n_apply_pred: apply_pred.len(),
        apply_precision: prec,
        apply_precision_wilson: prec_w,
        accuracy_answered: acc,
        accuracy_wilson: acc_w,
        abstain_rate: if n == 0 {
            0.0
        } else {
            (n - answered.len()) as f64 / n as f64
        },
        expected_calibration_error: ece(y_apply, p, 10),
    }
}

fn ece(y: &[bool], p: &[f64], bins: usize) -> Option<f64> {
    if y.is_empty() || bins == 0 {
        return None;
    }
    let n = y.len() as f64;
    let mut sum = 0.0;
    for b in 0..bins {
        let lo = b as f64 / bins as f64;
        let hi = (b + 1) as f64 / bins as f64;
        let idx: Vec<usize> = p
            .iter()
            .enumerate()
            .filter(|(_, &pi)| {
                if b + 1 == bins {
                    pi >= lo && pi <= hi
                } else {
                    pi >= lo && pi < hi
                }
            })
            .map(|(i, _)| i)
            .collect();
        if idx.is_empty() {
            continue;
        }
        let acc = idx.iter().filter(|&&i| y[i]).count() as f64 / idx.len() as f64;
        let conf = idx.iter().map(|&i| p[i]).sum::<f64>() / idx.len() as f64;
        sum += (idx.len() as f64 / n) * (acc - conf).abs();
    }
    Some(sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wilson_known_values() {
        let w = wilson(8, 10, 1.96).unwrap();
        assert!((w.estimate - 0.8).abs() < 1e-12);
        assert!(w.low < 0.8 && w.high > 0.8);
        assert!(w.low > 0.4 && w.high < 1.0);
    }

    #[test]
    fn empty_set_has_no_precision() {
        let m = answered_metrics(&[], &[], &[]);
        assert_eq!(m.apply_precision, None);
        assert_eq!(m.n, 0);
    }

    #[test]
    fn wilson_n_zero_is_none() {
        assert!(wilson(0, 0, 1.96).is_none());
        assert!(wilson(1, 0, 1.96).is_none());
    }

    #[test]
    fn wilson_p_zero_is_clamped() {
        let w = wilson(0, 10, 1.96).unwrap();
        assert_eq!(w.successes, 0);
        assert_eq!(w.n, 10);
        assert!((w.estimate - 0.0).abs() < 1e-12);
        assert!(w.low >= 0.0);
        assert!(w.high > 0.0 && w.high < 1.0);
    }

    #[test]
    fn wilson_p_one_is_clamped() {
        let w = wilson(10, 10, 1.96).unwrap();
        assert_eq!(w.successes, 10);
        assert_eq!(w.n, 10);
        assert!((w.estimate - 1.0).abs() < 1e-12);
        assert!(w.high <= 1.0);
        assert!(w.low > 0.0 && w.low < 1.0);
    }
}
