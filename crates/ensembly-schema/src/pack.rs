use crate::Checked;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PackRef {
    pub id: String,
    pub company: String,
    pub role: String,
    pub link: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_url: Option<String>,
    pub domain: String,
    pub cv_version: String,
    pub sha256: String,
}

impl Checked for PackRef {}
