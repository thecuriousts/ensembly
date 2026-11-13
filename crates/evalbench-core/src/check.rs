//! Port of evalbench-python `check()`. A must-have is evidenced only if one evidence
//! item has a non-empty proof AND its lowercased claim contains one of the must-have's
//! lowercased keywords. Extra JSON keys are ignored, as in Python.
//!
//! Parity with Python holds on well-formed input, which includes every frozen fixture
//! (the differential test checks them all). Outside that, the general rule: Rust's
//! schema is strict and requires every field with its type (posting:
//! `must_haves[].name` string and `keywords` string list; pack: `evidence` list,
//! `evidence[].claim` string, `proof` string or null). Python reads only the fields its
//! short-circuiting `any()` reaches and treats any truthy `proof` as a proof. So any
//! schema-invalid input Python tolerates makes Rust fail to parse (CLI exit 2) instead
//! of giving a gap list. `tests/python_stricter.rs` pins examples of this class against
//! the vendored `check.py`; they are examples, not a complete list.

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
    serde_json::from_str(json)
}

pub fn parse_pack(json: &str) -> Result<Pack, serde_json::Error> {
    serde_json::from_str(json)
}

/// One flag per must-have, in posting order. Proof counts when `Some(s)` and `s` is
/// non-empty (no trim, D6). Lowercase with `str::to_lowercase` (D5). `""` keyword
/// matches any proved claim (D2, Python parity).
pub fn evidenced(posting: &Posting, pack: &Pack) -> Vec<bool> {
    let proved: Vec<String> = pack
        .evidence
        .iter()
        .filter(|e| e.proof.as_deref().is_some_and(|p| !p.is_empty()))
        .map(|e| e.claim.to_lowercase())
        .collect();
    posting
        .must_haves
        .iter()
        .map(|mh| {
            let kws: Vec<String> = mh.keywords.iter().map(|k| k.to_lowercase()).collect();
            proved
                .iter()
                .any(|claim| kws.iter().any(|k| claim.contains(k.as_str())))
        })
        .collect()
}

/// Indices of `false` flags, ascending.
pub fn gaps_from_flags(flags: &[bool]) -> Vec<usize> {
    flags
        .iter()
        .enumerate()
        .filter(|(_, ok)| !**ok)
        .map(|(i, _)| i)
        .collect()
}

/// Gap names in posting order, duplicates kept.
pub fn gaps(posting: &Posting, pack: &Pack) -> Vec<String> {
    gaps_from_flags(&evidenced(posting, pack))
        .into_iter()
        .map(|i| posting.must_haves[i].name.clone())
        .collect()
}
