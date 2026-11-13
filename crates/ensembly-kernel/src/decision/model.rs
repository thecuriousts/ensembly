//! Transparent features, a small L2 logistic model, and an untuned keyword baseline.
//!
//! Thresholds and weights are fit on train (and checked on val) only.
//! Held-out rows must not be passed to [`fit`] or [`pick_band`].

use ensembly_schema::{GateVerdict, LabelRow, ModelDecision};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use super::gate::{audit, FlagList, GateResult, GATE_REASON};

pub const FEATURE_NAMES: &[&str] = &[
    "loc_sweden",
    "loc_eu",
    "loc_us",
    "role_intern",
    "lang_blocker",
    "role_senior",
    "stack_match",
    "role_agent",
    "agency",
    "jd_len_log",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Band {
    pub t_skip: f64,
    pub t_apply: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FittedModel {
    pub version: String,
    pub mean: Vec<f64>,
    pub std: Vec<f64>,
    pub weights: Vec<f64>,
    pub bias: f64,
    pub band: Band,
    pub trained_on: usize,
}

#[derive(Debug, Clone)]
pub struct Score {
    pub p_apply: f64,
    pub decision: ModelDecision,
    pub reason: String,
    pub gate: GateResult,
}

fn rx(slot: &'static OnceLock<Regex>, pat: &'static str) -> &'static Regex {
    slot.get_or_init(|| Regex::new(pat).expect("feature regex"))
}

fn blob(row: &LabelRow) -> String {
    format!(
        "{} {} {} {} {}",
        row.company,
        row.role,
        row.location.as_deref().unwrap_or(""),
        row.posting_text,
        row.url
    )
}

fn bit(rx: &Regex, text: &str) -> f64 {
    if rx.is_match(text) {
        1.0
    } else {
        0.0
    }
}

/// Extract the frozen feature vector for one posting.
pub fn features(row: &LabelRow) -> Vec<f64> {
    static SWE: OnceLock<Regex> = OnceLock::new();
    static EU: OnceLock<Regex> = OnceLock::new();
    static US: OnceLock<Regex> = OnceLock::new();
    static INTERN: OnceLock<Regex> = OnceLock::new();
    static LANG: OnceLock<Regex> = OnceLock::new();
    static SENIOR: OnceLock<Regex> = OnceLock::new();
    static STACK: OnceLock<Regex> = OnceLock::new();
    static AGENT: OnceLock<Regex> = OnceLock::new();
    static AGENCY: OnceLock<Regex> = OnceLock::new();
    let text = blob(row);
    let jd = row.posting_text.len() as f64;
    vec![
        bit(
            rx(
                &SWE,
                r"(?i)\b(?:stockholm|sweden|gothenburg|malmo|göteborg|malmö)\b",
            ),
            &text,
        ),
        bit(
            rx(&EU, r"(?i)\b(?:\beu\b|emea|europe|european|eu-remote)\b"),
            &text,
        ),
        bit(
            rx(
                &US,
                r"(?i)\b(?:united states|\busa\b|san francisco|new york|seattle)\b",
            ),
            &text,
        ),
        bit(
            rx(&INTERN, r"(?i)\b(?:intern|internship|student|new grad)\b"),
            &text,
        ),
        bit(
            rx(
                &LANG,
                r"(?i)(?:fluent|native|professional)\s+(?:german|swedish|french|japanese|mandarin)|(?:german|swedish|japanese)\s+(?:c1|c2|required)",
            ),
            &text,
        ),
        bit(rx(&SENIOR, r"(?i)\b(?:senior|staff|principal)\b"), &text),
        bit(
            rx(&STACK, r"(?i)\b(?:typescript|rust|react|node\.?js)\b"),
            &text,
        ),
        bit(rx(&AGENT, r"(?i)\b(?:agentic|llm|mcp)\b"), &text),
        bit(
            rx(&AGENCY, r"(?i)\b(?:recruitment agency|staffing firm)\b"),
            &text,
        ),
        if jd <= 0.0 { 0.0 } else { jd.ln() },
    ]
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z).exp())
}

fn predict_raw(model: &FittedModel, x: &[f64]) -> f64 {
    let mut z = model.bias;
    for (i, &xi) in x.iter().enumerate() {
        let s = if model.std[i] < 1e-9 {
            1.0
        } else {
            model.std[i]
        };
        z += model.weights[i] * ((xi - model.mean[i]) / s);
    }
    sigmoid(z)
}

pub fn decide_from_p(p: f64, gate: &GateResult, band: &Band) -> ModelDecision {
    match gate.verdict {
        GateVerdict::Fail => ModelDecision::Skip,
        GateVerdict::Uncertain => ModelDecision::Abstain,
        GateVerdict::Pass if p >= band.t_apply => ModelDecision::Apply,
        GateVerdict::Pass if p <= band.t_skip => ModelDecision::Skip,
        GateVerdict::Pass => ModelDecision::Abstain,
    }
}

/// Fit mean/std and L2 logistic weights on labelled train rows only.
///
/// This function does not check the split. Passing held-out or validation
/// rows here leaks the evaluation set into the weights and the band.
/// Callers must pass labelled train rows only; [`super::run_eval`] already
/// does that.
pub fn fit(rows: &[&LabelRow], flags: &FlagList, target_prec: f64) -> Result<FittedModel, String> {
    if rows.is_empty() {
        return Err("train set is empty".into());
    }
    if rows.iter().any(|r| r.decision.is_none()) {
        return Err("train rows must be labelled".into());
    }
    let xs: Vec<Vec<f64>> = rows.iter().map(|r| features(r)).collect();
    let k = FEATURE_NAMES.len();
    let n = xs.len() as f64;
    let mut mean = vec![0.0; k];
    for x in &xs {
        for i in 0..k {
            mean[i] += x[i];
        }
    }
    for m in &mut mean {
        *m /= n;
    }
    let mut std = vec![0.0; k];
    for x in &xs {
        for i in 0..k {
            let d = x[i] - mean[i];
            std[i] += d * d;
        }
    }
    for s in &mut std {
        *s = (*s / n).sqrt();
        if *s < 1e-9 {
            *s = 1.0;
        }
    }
    let y: Vec<f64> = rows
        .iter()
        .map(|r| {
            if r.decision.unwrap() == ensembly_schema::FitLabel::Apply {
                1.0
            } else {
                0.0
            }
        })
        .collect();
    let mut w = vec![0.0; k];
    let mut bias = 0.0;
    let l2 = 1.0;
    let lr = 0.25;
    let steps = 1500usize;
    for _ in 0..steps {
        let mut gw = vec![0.0; k];
        let mut gb = 0.0;
        for (x, &yi) in xs.iter().zip(y.iter()) {
            let mut z = bias;
            for i in 0..k {
                z += w[i] * ((x[i] - mean[i]) / std[i]);
            }
            let p = sigmoid(z);
            let e = p - yi;
            gb += e;
            for i in 0..k {
                gw[i] += e * ((x[i] - mean[i]) / std[i]);
            }
        }
        for i in 0..k {
            gw[i] = gw[i] / n + l2 * w[i];
            w[i] -= lr * gw[i];
        }
        bias -= lr * (gb / n);
    }
    let mut draft = FittedModel {
        version: ensembly_schema::MODEL_VERSION.into(),
        mean,
        std,
        weights: w,
        bias,
        band: Band {
            t_skip: 0.35,
            t_apply: 0.65,
        },
        trained_on: rows.len(),
    };
    let p: Vec<f64> = xs.iter().map(|x| predict_raw(&draft, x)).collect();
    let gates: Vec<GateResult> = rows
        .iter()
        .map(|r| audit(&r.url, &r.posting_text, flags))
        .collect();
    draft.band = pick_band(&y, &p, &gates, target_prec);
    Ok(draft)
}

/// Choose (t_skip, t_apply) on the given labelled rows only.
///
/// Like [`fit`], this has no split check. Passing held-out scores here
/// moves the band. Callers must pass train (or a dedicated tuning set)
/// only; [`super::run_eval`] already does that.
pub fn pick_band(y: &[f64], p: &[f64], gates: &[GateResult], target_prec: f64) -> Band {
    let mut best: Option<(i32, i32, i32, i32, i32, Band)> = None;
    let mut t_apply = 0.50;
    while t_apply <= 0.98 + 1e-9 {
        let mut t_skip = 0.02;
        while t_skip < t_apply - 1e-9 {
            let band = Band { t_skip, t_apply };
            let preds: Vec<ModelDecision> = p
                .iter()
                .enumerate()
                .map(|(i, &pi)| decide_from_p(pi, &gates[i], &band))
                .collect();
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
                0.0
            } else {
                apply_pred.iter().filter(|&&i| y[i] == 1.0).count() as f64 / apply_pred.len() as f64
            };
            let acc = if answered.is_empty() {
                0.0
            } else {
                answered
                    .iter()
                    .filter(|&&i| {
                        (preds[i] == ModelDecision::Apply && y[i] == 1.0)
                            || (preds[i] == ModelDecision::Skip && y[i] == 0.0)
                    })
                    .count() as f64
                    / answered.len() as f64
            };
            let abstain = 1.0 - (answered.len() as f64 / y.len().max(1) as f64);
            let meets = if prec + 1e-12 >= target_prec && !apply_pred.is_empty() {
                1
            } else {
                0
            };
            let in_band = if (0.15..=0.30).contains(&abstain) {
                1
            } else {
                0
            };
            let key = (
                meets,
                in_band,
                (prec * 10000.0) as i32,
                (acc * 10000.0) as i32,
                apply_pred.len() as i32,
                band,
            );
            best = Some(match best {
                None => key,
                Some(cur) => {
                    if (key.0, key.1, key.2, key.3, key.4) > (cur.0, cur.1, cur.2, cur.3, cur.4) {
                        key
                    } else {
                        cur
                    }
                }
            });
            t_skip += 0.04;
        }
        t_apply += 0.04;
    }
    best.map(|b| b.5).unwrap_or(Band {
        t_skip: 0.35,
        t_apply: 0.65,
    })
}

pub fn score(model: &FittedModel, row: &LabelRow, flags: &FlagList) -> Score {
    let gate = audit(&row.url, &row.posting_text, flags);
    let p = predict_raw(model, &features(row));
    let decision = decide_from_p(p, &gate, &model.band);
    let reason = if gate.verdict == GateVerdict::Fail {
        GATE_REASON.to_string()
    } else if gate.verdict == GateVerdict::Uncertain {
        "scam-gate-uncertain".into()
    } else {
        format!(
            "{} p(apply)={p:.3} -> {}",
            ensembly_schema::TRIAGE_QUESTION,
            decision.as_str()
        )
    };
    Score {
        p_apply: p,
        decision,
        reason,
        gate,
    }
}

/// Untuned keyword rule. Thresholds were not fit on labels.
pub fn baseline(row: &LabelRow, flags: &FlagList) -> Score {
    static INTERN: OnceLock<Regex> = OnceLock::new();
    static LANG: OnceLock<Regex> = OnceLock::new();
    let gate = audit(&row.url, &row.posting_text, flags);
    if gate.verdict == GateVerdict::Fail {
        return Score {
            p_apply: 0.05,
            decision: ModelDecision::Skip,
            reason: GATE_REASON.into(),
            gate,
        };
    }
    let text = blob(row);
    if rx(
        &LANG,
        r"(?i)(?:fluent|native|professional)\s+(?:german|swedish|french|japanese|mandarin)|(?:german|swedish|japanese)\s+(?:c1|c2|required)",
    )
    .is_match(&text)
    {
        return Score {
            p_apply: 0.15,
            decision: ModelDecision::Skip,
            reason: "untuned keyword: language requirement".into(),
            gate,
        };
    }
    if rx(&INTERN, r"(?i)\b(?:intern|internship|student|new grad)\b").is_match(&text) {
        return Score {
            p_apply: 0.15,
            decision: ModelDecision::Skip,
            reason: "untuned keyword: intern or student role".into(),
            gate,
        };
    }
    Score {
        p_apply: 0.75,
        decision: ModelDecision::Apply,
        reason: "untuned keyword: default apply (no skip cue)".into(),
        gate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ensembly_schema::FitLabel;

    fn row(id: &str, text: &str, label: FitLabel) -> LabelRow {
        let url = format!("https://jobs.example.test/posting/{id}");
        LabelRow {
            id: id.into(),
            link_key: url.clone(),
            company: "Birch Fictional".into(),
            role: "Engineer".into(),
            location: Some("Stockholm".into()),
            url,
            posting_text: text.into(),
            decision: Some(label),
            reason_tag: None,
            outcome: None,
            provenance: "synthetic-fixture".into(),
        }
    }

    #[test]
    fn intern_feature_fires() {
        let r = row("1", "Internship in rust", FitLabel::Skip);
        let x = features(&r);
        assert_eq!(x[3], 1.0);
    }

    #[test]
    fn gate_fail_skips_regardless_of_p() {
        let gate = GateResult {
            verdict: GateVerdict::Fail,
            reason: Some(GATE_REASON.into()),
        };
        let d = decide_from_p(
            0.99,
            &gate,
            &Band {
                t_skip: 0.2,
                t_apply: 0.8,
            },
        );
        assert_eq!(d, ModelDecision::Skip);
    }
}
