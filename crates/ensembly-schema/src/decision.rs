//! E3 decision-model types: labels, frozen splits, and propose-and-wait events.
//!
//! Human labels are `apply` or `skip` only. The model may also abstain.
//! A missing decision on a row is unlabelled, never inferred.

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::link_key;
use crate::Checked;

/// Fixed triage question. Do not change after the split is frozen.
pub const TRIAGE_QUESTION: &str = "Should this posting be labelled apply?";

/// Frozen split seed. Assignments use this value and [`assign_split`].
pub const SPLIT_SEED: &str = "e3-2026-11-13";

/// Model version checkpointed on each proposal.
pub const MODEL_VERSION: &str = "e3-v0";

/// Default wait before a proposal expires with no action.
pub const DEFAULT_TIMEOUT_HOURS: i64 = 72;

/// Human or confirmed label. Never `abstain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FitLabel {
    Apply,
    Skip,
}

impl FitLabel {
    pub fn as_str(self) -> &'static str {
        match self {
            FitLabel::Apply => "apply",
            FitLabel::Skip => "skip",
        }
    }
}

impl fmt::Display for FitLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Model output, including abstain (defer to the operator).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelDecision {
    Apply,
    Skip,
    Abstain,
}

impl ModelDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            ModelDecision::Apply => "apply",
            ModelDecision::Skip => "skip",
            ModelDecision::Abstain => "abstain",
        }
    }
}

/// Scam-gate verdict. Fail always skips and is never learned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GateVerdict {
    Pass,
    Fail,
    Uncertain,
}

impl GateVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            GateVerdict::Pass => "pass",
            GateVerdict::Fail => "fail",
            GateVerdict::Uncertain => "uncertain",
        }
    }
}

/// Frozen split bucket. Assigned from the link key and [`SPLIT_SEED`] only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SplitBucket {
    Train,
    Val,
    HeldOut,
}

impl SplitBucket {
    pub fn as_str(self) -> &'static str {
        match self {
            SplitBucket::Train => "train",
            SplitBucket::Val => "val",
            SplitBucket::HeldOut => "held-out",
        }
    }
}

/// Assign train / val / held-out from the canonical link and the frozen seed.
///
/// 0..59 train, 60..79 val, 80..99 held-out. The rule is fixed before labelling.
pub fn assign_split(link: &str) -> SplitBucket {
    let key = link_key(link);
    let digest = Sha256::digest(format!("{SPLIT_SEED}\t{key}").as_bytes());
    let n = u32::from_str_radix(&hex_8(&digest), 16).expect("hex") % 100;
    if n < 60 {
        SplitBucket::Train
    } else if n < 80 {
        SplitBucket::Val
    } else {
        SplitBucket::HeldOut
    }
}

fn hex_8(digest: &[u8]) -> String {
    hex_encode(&digest[..4])
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// SHA-256 hex of posting snapshot bytes used in a proposal rid.
pub fn snapshot_hash(company: &str, role: &str, url: &str, posting_text: &str) -> String {
    let key = link_key(url);
    let blob = format!("{company}\t{role}\t{key}\t{posting_text}");
    hex_encode(&Sha256::digest(blob.as_bytes()))
}

/// Idempotent proposal id: hash(link_key, "propose", model_version, snapshot_hash).
pub fn proposal_rid(link: &str, model_version: &str, snapshot_hash: &str) -> String {
    let key = link_key(link);
    let blob = format!("{key}\tpropose\t{model_version}\t{snapshot_hash}");
    hex_encode(&Sha256::digest(blob.as_bytes()))
}

/// One posting to label, or a labelled fixture row.
///
/// `decision` is empty until the operator labels it. Nothing infers a label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelRow {
    pub id: String,
    pub link_key: String,
    pub company: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub url: String,
    #[serde(default)]
    pub posting_text: String,
    /// Present only after a person labels the row. Never inferred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<FitLabel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    pub provenance: String,
}

impl LabelRow {
    pub fn split(&self) -> SplitBucket {
        assign_split(&self.link_key)
    }

    pub fn snapshot_hash(&self) -> String {
        snapshot_hash(&self.company, &self.role, &self.url, &self.posting_text)
    }
}

impl Checked for LabelRow {
    fn check(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("id is required".into());
        }
        if self.company.trim().is_empty() {
            return Err("company is required".into());
        }
        if self.role.trim().is_empty() {
            return Err("role is required".into());
        }
        if !self.url.starts_with("https://") {
            return Err("url must be https".into());
        }
        if link_key(&self.link_key) != link_key(&self.url)
            && link_key(&self.link_key) != self.link_key.trim()
        {
            // link_key field should be the canonical form of url
        }
        if link_key(&self.link_key) != link_key(&self.url) {
            return Err("link_key must match the canonical url".into());
        }
        if self.provenance.trim().is_empty() {
            return Err("provenance is required".into());
        }
        if self.decision.is_none() && self.reason_tag.is_some() {
            return Err("reason_tag requires a decision".into());
        }
        Ok(())
    }
}

/// Append-only propose-and-wait event. The model never writes a posting status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionEvent {
    pub event_type: DecisionEventType,
    pub rid: String,
    pub link_key: String,
    pub event_time: String,
    pub writer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
    pub snapshot_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<ModelDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p_apply: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    pub provenance: String,
    pub triage_question: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DecisionEventType {
    Proposed,
    Confirmed,
    Overridden,
    Expired,
    Skipped,
}

impl DecisionEventType {
    pub fn as_str(self) -> &'static str {
        match self {
            DecisionEventType::Proposed => "proposed",
            DecisionEventType::Confirmed => "confirmed",
            DecisionEventType::Overridden => "overridden",
            DecisionEventType::Expired => "expired",
            DecisionEventType::Skipped => "skipped",
        }
    }
}

impl Checked for DecisionEvent {
    fn check(&self) -> Result<(), String> {
        if self.rid.trim().is_empty() {
            return Err("rid is required".into());
        }
        if self.link_key.trim().is_empty() {
            return Err("link_key is required".into());
        }
        if self.event_time.trim().is_empty() {
            return Err("event_time is required".into());
        }
        if self.writer.trim().is_empty() {
            return Err("writer is required".into());
        }
        if self.snapshot_hash.trim().is_empty() {
            return Err("snapshot_hash is required".into());
        }
        if self.provenance.trim().is_empty() {
            return Err("provenance is required".into());
        }
        if self.triage_question != TRIAGE_QUESTION {
            return Err("triage_question must be the frozen wording".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_is_stable_and_covers_all_buckets() {
        assert_eq!(
            assign_split("https://jobs.example.test/posting/1"),
            SplitBucket::Train
        );
        assert_eq!(
            assign_split("https://jobs.example.test/posting/6"),
            SplitBucket::Val
        );
        assert_eq!(
            assign_split("https://jobs.example.test/posting/10"),
            SplitBucket::HeldOut
        );
        assert_eq!(
            assign_split("https://jobs.example.test/posting/1/"),
            assign_split("https://jobs.example.test/posting/1")
        );
    }

    #[test]
    fn rid_is_stable() {
        let h = snapshot_hash(
            "Birch Fictional",
            "Clerk",
            "https://jobs.example.test/a",
            "text",
        );
        let a = proposal_rid("https://jobs.example.test/a", MODEL_VERSION, &h);
        let b = proposal_rid("https://jobs.example.test/a/", MODEL_VERSION, &h);
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }
}
