//! Port of evalbench-python `check()`. A must-have is evidenced only if one evidence
//! item has a non-empty proof AND its lowercased claim contains one of the must-have's
//! lowercased keywords. Extra JSON keys are ignored, as in Python.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MustHave {
    pub name: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posting {
    pub must_haves: Vec<MustHave>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub claim: String,
    #[serde(default)]
    pub proof: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pack {
    pub evidence: Vec<Evidence>,
}

pub fn parse_posting(json: &str) -> Result<Posting, serde_json::Error> {
    let _ = json;
    unimplemented!("E1: parse_posting")
}

pub fn parse_pack(json: &str) -> Result<Pack, serde_json::Error> {
    let _ = json;
    unimplemented!("E1: parse_pack")
}

/// One flag per must-have, in posting order. Proof counts when `Some(s)` and `s` is
/// non-empty (no trim, D6). Lowercase with `str::to_lowercase` (D5). `""` keyword
/// matches any proved claim (D2, Python parity).
pub fn evidenced(posting: &Posting, pack: &Pack) -> Vec<bool> {
    let _ = (posting, pack);
    unimplemented!("E1: evidenced")
}

/// Indices of `false` flags, ascending.
pub fn gaps_from_flags(flags: &[bool]) -> Vec<usize> {
    let _ = flags;
    unimplemented!("E1: gaps_from_flags")
}

/// Gap names in posting order, duplicates kept.
pub fn gaps(posting: &Posting, pack: &Pack) -> Vec<String> {
    let _ = (posting, pack);
    unimplemented!("E1: gaps")
}
