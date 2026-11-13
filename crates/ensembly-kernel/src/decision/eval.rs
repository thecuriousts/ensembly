//! Load labels, fit on train only, and report metrics.
//!
//! Held-out precision and accuracy are omitted until every held-out row
//! in the file is labelled. The report never invents a label.

use anyhow::{bail, Context};
use ensembly_schema::{
    decode_line, FitLabel, LabelRow, ModelDecision, SplitBucket, TRIAGE_QUESTION,
};
use serde::Serialize;
use std::path::Path;

use super::gate::FlagList;
use super::metrics::{answered_metrics, AnsweredMetrics};
use super::model::{baseline, fit, score, FittedModel};

#[derive(Debug, Clone, Serialize)]
pub struct SplitReport {
    pub bucket: String,
    pub n: usize,
    pub n_labelled: usize,
    pub complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<MetricJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<MetricJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricJson {
    pub n: usize,
    pub n_answered: usize,
    pub n_abstain: usize,
    pub abstain_rate: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_precision: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_precision_wilson: Option<WilsonJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy_answered: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy_wilson: Option<WilsonJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_calibration_error: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WilsonJson {
    pub n: usize,
    pub successes: usize,
    pub estimate: f64,
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvalReport {
    pub triage_question: String,
    pub claim: String,
    pub trained_on: usize,
    pub band: super::model::Band,
    pub splits: Vec<SplitReport>,
}

pub fn load_labels(path: &Path) -> anyhow::Result<Vec<LabelRow>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut rows = Vec::new();
    for (i, raw) in text.split('\n').enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        let decoded = decode_line::<LabelRow>(raw)
            .map_err(|e| anyhow::anyhow!("label line {}: {e}", i + 1))?;
        rows.push(decoded.value);
    }
    Ok(rows)
}

pub fn run_eval(
    rows: &[LabelRow],
    flags: &FlagList,
    target_prec: f64,
) -> anyhow::Result<EvalReport> {
    let train: Vec<&LabelRow> = rows
        .iter()
        .filter(|r| r.split() == SplitBucket::Train && r.decision.is_some())
        .collect();
    if train.is_empty() {
        bail!("no labelled train rows; the model is not fit");
    }
    let model = fit(&train, flags, target_prec).map_err(|e| anyhow::anyhow!(e))?;
    let splits = [SplitBucket::Train, SplitBucket::Val, SplitBucket::HeldOut]
        .into_iter()
        .map(|bucket| report_split(rows, bucket, &model, flags))
        .collect();
    Ok(EvalReport {
        triage_question: TRIAGE_QUESTION.into(),
        claim: "No accuracy is claimed on unlabelled rows or on rows used to pick the threshold."
            .into(),
        trained_on: model.trained_on,
        band: model.band.clone(),
        splits,
    })
}

fn report_split(
    rows: &[LabelRow],
    bucket: SplitBucket,
    model: &FittedModel,
    flags: &FlagList,
) -> SplitReport {
    let group: Vec<&LabelRow> = rows.iter().filter(|r| r.split() == bucket).collect();
    let labelled: Vec<&LabelRow> = group
        .iter()
        .copied()
        .filter(|r| r.decision.is_some())
        .collect();
    let complete = !group.is_empty() && labelled.len() == group.len();
    if bucket == SplitBucket::HeldOut && !complete {
        return SplitReport {
            bucket: bucket.as_str().into(),
            n: group.len(),
            n_labelled: labelled.len(),
            complete: false,
            model: None,
            baseline: None,
            note: Some(
                "held-out labels are incomplete; precision and accuracy are not reported".into(),
            ),
        };
    }
    if labelled.is_empty() {
        return SplitReport {
            bucket: bucket.as_str().into(),
            n: group.len(),
            n_labelled: 0,
            complete,
            model: None,
            baseline: None,
            note: Some("no labelled rows in this split".into()),
        };
    }
    SplitReport {
        bucket: bucket.as_str().into(),
        n: group.len(),
        n_labelled: labelled.len(),
        complete,
        model: Some(metrics_json(&labelled, model, flags, false)),
        baseline: Some(metrics_json(&labelled, model, flags, true)),
        note: if bucket == SplitBucket::Train || bucket == SplitBucket::Val {
            Some(
                "threshold was chosen on train; do not treat this split as a held-out claim".into(),
            )
        } else {
            None
        },
    }
}

fn metrics_json(
    rows: &[&LabelRow],
    model: &FittedModel,
    flags: &FlagList,
    use_baseline: bool,
) -> MetricJson {
    let y: Vec<bool> = rows
        .iter()
        .map(|r| r.decision == Some(FitLabel::Apply))
        .collect();
    let scored: Vec<_> = rows
        .iter()
        .map(|r| {
            if use_baseline {
                baseline(r, flags)
            } else {
                score(model, r, flags)
            }
        })
        .collect();
    let preds: Vec<ModelDecision> = scored.iter().map(|s| s.decision).collect();
    let p: Vec<f64> = scored.iter().map(|s| s.p_apply).collect();
    from_answered(&answered_metrics(&y, &preds, &p))
}

fn from_answered(m: &AnsweredMetrics) -> MetricJson {
    MetricJson {
        n: m.n,
        n_answered: m.n_answered,
        n_abstain: m.n_abstain,
        abstain_rate: m.abstain_rate,
        apply_precision: m.apply_precision,
        apply_precision_wilson: m.apply_precision_wilson.as_ref().map(|w| WilsonJson {
            n: w.n,
            successes: w.successes,
            estimate: w.estimate,
            low: w.low,
            high: w.high,
        }),
        accuracy_answered: m.accuracy_answered,
        accuracy_wilson: m.accuracy_wilson.as_ref().map(|w| WilsonJson {
            n: w.n,
            successes: w.successes,
            estimate: w.estimate,
            low: w.low,
            high: w.high,
        }),
        expected_calibration_error: m.expected_calibration_error,
    }
}
