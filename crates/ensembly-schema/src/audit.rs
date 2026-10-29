use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
#[rustfmt::skip]
pub enum Verdict { Pass, Flag, Block }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
#[rustfmt::skip]
pub enum CheckResult { Pass, Flag, Block, Unknown }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[rustfmt::skip]
pub enum CheckId { ApplyDomain, DomainAge, FundingOwnership, Headcount, ReviewsNews, DuplicateGhost, PaymentIdAsk, AgencyRepost }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct Check { pub id: CheckId, pub result: CheckResult, pub note: String, pub sources: Vec<String> }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[rustfmt::skip]
pub struct Audit {
    pub company: String, pub domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_url: Option<String>,
    pub verdict: Verdict, pub checks: Vec<Check>, pub audited_at: String, pub audited_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reused_from: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[rustfmt::skip]
pub enum GateStatus { Pass, Flag, Block, Unaudited, Stale }
// 0 pass, 3 flag (not approvable), 4 block, 5 unaudited or stale, 7 pulse refused write.
// 1 and 2 stay errors and usage.
pub const EXIT_PASS: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_FLAG: i32 = 3;
pub const EXIT_BLOCK: i32 = 4;
pub const EXIT_UNAUDITED_OR_STALE: i32 = 5;
pub const EXIT_PULSE_REFUSED_WRITE: i32 = 7;
pub fn exit_code(s: GateStatus) -> i32 {
    match s {
        GateStatus::Pass => EXIT_PASS,
        GateStatus::Flag => EXIT_FLAG,
        GateStatus::Block => EXIT_BLOCK,
        GateStatus::Unaudited | GateStatus::Stale => EXIT_UNAUDITED_OR_STALE,
    }
}
pub fn canonical_domain(s: &str) -> String {
    let s = s.trim().to_lowercase();
    let s = s.split_once("://").map(|(_, r)| r.to_string()).unwrap_or(s);
    let host = s.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    let host = host.trim_end_matches('.');
    host.strip_prefix("www.").unwrap_or(host).to_string()
}
