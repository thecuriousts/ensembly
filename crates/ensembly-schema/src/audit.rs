use crate::Checked;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Pass,
    Flag,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum CheckResult {
    Pass,
    Flag,
    Block,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CheckId {
    ApplyDomain,
    DomainAge,
    FundingOwnership,
    Headcount,
    ReviewsNews,
    DuplicateGhost,
    PaymentIdAsk,
    AgencyRepost,
}

impl CheckId {
    pub const ALL: [CheckId; 8] = [
        CheckId::ApplyDomain,
        CheckId::DomainAge,
        CheckId::FundingOwnership,
        CheckId::Headcount,
        CheckId::ReviewsNews,
        CheckId::DuplicateGhost,
        CheckId::PaymentIdAsk,
        CheckId::AgencyRepost,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            CheckId::ApplyDomain => "apply_domain",
            CheckId::DomainAge => "domain_age",
            CheckId::FundingOwnership => "funding_ownership",
            CheckId::Headcount => "headcount",
            CheckId::ReviewsNews => "reviews_news",
            CheckId::DuplicateGhost => "duplicate_ghost",
            CheckId::PaymentIdAsk => "payment_id_ask",
            CheckId::AgencyRepost => "agency_repost",
        }
    }

    /// Checks whose `unknown` alone is enough to flag: without them the employer
    /// cannot be shown to exist as stated.
    fn unknown_flags(self) -> bool {
        matches!(
            self,
            CheckId::ApplyDomain | CheckId::DomainAge | CheckId::FundingOwnership
        )
    }
}

impl CheckResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckResult::Pass => "pass",
            CheckResult::Flag => "flag",
            CheckResult::Block => "block",
            CheckResult::Unknown => "unknown",
        }
    }
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Flag => "flag",
            Verdict::Block => "block",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub id: CheckId,
    pub result: CheckResult,
    pub note: String,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Audit {
    pub company: String,
    pub domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_url: Option<String>,
    pub verdict: Verdict,
    pub checks: Vec<Check>,
    pub audited_at: String,
    pub audited_by: String,
    /// Optional `audited_at` of the earlier audit whose result this row reuses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reused_from: Option<String>,
}

impl Checked for Audit {}

impl Audit {
    /// The verdict the checks give under the current rule. Equals `verdict`
    /// for every row except a stored legacy pass that now derives flag.
    pub fn effective_verdict(&self) -> Verdict {
        derive_verdict(&self.checks)
    }
}

/// block if any check is block; flag if any check is flag, any of
/// apply_domain / domain_age / funding_ownership is unknown, or headcount and
/// reviews_news are both unknown; else pass.
pub fn derive_verdict(checks: &[Check]) -> Verdict {
    let unknown = |id: CheckId| {
        checks
            .iter()
            .any(|c| c.id == id && c.result == CheckResult::Unknown)
    };
    let size_unknown = unknown(CheckId::Headcount) && unknown(CheckId::ReviewsNews);
    match derive_verdict_before_size_rule(checks) {
        Verdict::Pass if size_unknown => Verdict::Flag,
        other => other,
    }
}

/// The rule before 30 Oct 2026, without the headcount + reviews_news case.
/// Only used to keep rows stored under that rule readable.
pub fn derive_verdict_before_size_rule(checks: &[Check]) -> Verdict {
    if checks.iter().any(|c| c.result == CheckResult::Block) {
        Verdict::Block
    } else if checks.iter().any(|c| {
        c.result == CheckResult::Flag || (c.result == CheckResult::Unknown && c.id.unknown_flags())
    }) {
        Verdict::Flag
    } else {
        Verdict::Pass
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GateStatus {
    Pass,
    Flag,
    Block,
    Unaudited,
    Stale,
}

// 0 pass, 3 flag (not approvable), 4 block, 5 unaudited or stale, 7 pulse refused write.
// 1 and 2 stay errors and usage.
pub const EXIT_PASS: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_FLAG: i32 = 3;
pub const EXIT_BLOCK: i32 = 4;
pub const EXIT_UNAUDITED_OR_STALE: i32 = 5;
pub const EXIT_PULSE_REFUSED_WRITE: i32 = 7;

pub fn audit_exit_code(s: GateStatus) -> i32 {
    match s {
        GateStatus::Pass => EXIT_PASS,
        GateStatus::Flag => EXIT_FLAG,
        GateStatus::Block => EXIT_BLOCK,
        GateStatus::Unaudited | GateStatus::Stale => EXIT_UNAUDITED_OR_STALE,
    }
}

/// Statuses that produce `code`, or `None` when `code` is not an audit result.
/// `1`, `2`, `6`, and `255` are `None`. `5` is both unaudited and stale.
pub fn audit_exit_status(code: i32) -> Option<&'static [GateStatus]> {
    match code {
        EXIT_PASS => Some(&[GateStatus::Pass]),
        EXIT_FLAG => Some(&[GateStatus::Flag]),
        EXIT_BLOCK => Some(&[GateStatus::Block]),
        EXIT_UNAUDITED_OR_STALE => Some(&[GateStatus::Unaudited, GateStatus::Stale]),
        _ => None,
    }
}

#[cfg(test)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct AuditLine {
    #[serde(flatten)]
    body: Audit,
    v: crate::version_one::VersionOne,
}

/// Lowercase host without scheme, `www.`, path, query, port, or trailing dot.
pub fn canonical_domain(s: &str) -> String {
    let s = s.trim().to_lowercase();
    let s = s.split_once("://").map(|(_, r)| r.to_string()).unwrap_or(s);
    let host = s.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    let host = host.trim_end_matches('.');
    host.strip_prefix("www.").unwrap_or(host).to_string()
}
