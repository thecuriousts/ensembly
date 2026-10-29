use crate::Checked;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Prepared,
    Submitted,
    Interview,
    Rejected,
    Withdrawn,
    SilentClosed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LedgerRow {
    pub company: String,
    pub role: String,
    pub link: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_url: Option<String>,
    pub status: Status,
    pub submit_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit_time_unknown: Option<bool>,
    pub cv_version: String,
}

impl Checked for LedgerRow {
    fn check(&self) -> Result<(), String> {
        check_row_submit_time(self)
    }
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Prepared => "prepared",
            Status::Submitted => "submitted",
            Status::Interview => "interview",
            Status::Rejected => "rejected",
            Status::Withdrawn => "withdrawn",
            Status::SilentClosed => "silent-closed",
        }
    }
}

fn allows_null_submit_time(status: Status) -> bool {
    matches!(status, Status::Prepared | Status::Withdrawn)
}

fn reject_bad_unknown_flag(
    status: Status,
    time: Option<&str>,
    flag: Option<bool>,
) -> Result<(), String> {
    if flag == Some(false) {
        return Err("submit_time_unknown is either true or absent".into());
    }
    if time.is_some() && flag == Some(true) {
        return Err("submit_time_unknown: true needs submit_time null".into());
    }
    if time.is_none() && flag == Some(true) && allows_null_submit_time(status) {
        let name = status.as_str();
        return Err(format!(
            "submit_time_unknown is only for sent statuses; {name} already allows a null submit_time"
        ));
    }
    Ok(())
}

pub fn check_submit_time(t: &str) -> Result<(), String> {
    chrono::DateTime::parse_from_rfc3339(t)
        .map_err(|e| format!("submit_time {t:?} is not RFC 3339 with offset: {e}"))?;
    if t.ends_with("+01:00") || t.ends_with("+02:00") {
        Ok(())
    } else {
        Err(format!(
            "submit_time {t:?} must carry the Europe/Stockholm offset (+01:00 or +02:00)"
        ))
    }
}

pub fn check_row_submit_time(r: &LedgerRow) -> Result<(), String> {
    let time = r.submit_time.as_deref();
    reject_bad_unknown_flag(r.status, time, r.submit_time_unknown)?;
    if let Some(t) = time {
        return check_submit_time(t);
    }
    if r.submit_time_unknown == Some(true) || allows_null_submit_time(r.status) {
        Ok(())
    } else {
        let name = r.status.as_str();
        Err(format!(
            "submit_time is required when status is {name} (if it was never recorded, set submit_time_unknown: true)"
        ))
    }
}

pub fn link_key(link: &str) -> &str {
    let l = link.trim_end_matches('/');
    l.strip_suffix("/application").unwrap_or(l)
}
