//! Port of evalbench-python `check()`. A must-have is evidenced only if one evidence
//! item has a non-empty proof AND its lowercased claim contains one of the must-have's
//! lowercased keywords. Extra JSON keys are ignored, as in Python.
//!
//! Parity with Python on every input Python accepts, except three documented cases
//! where Rust is stricter and fails to parse (CLI exit 2) instead of giving a gap list.
//! Python's `any()` short-circuits, so it never reads the field. Pinned in
//! `tests/python_stricter.rs`:
//! 1. An evidence item with no `claim` and no proof: Python reports the gap; Rust
//!    rejects the pack (missing field `claim`).
//! 2. Zero must-haves with a pack that has no `evidence` key: Python gives no gaps;
//!    Rust rejects the pack (missing field `evidence`).
//! 3. A truthy non-string `proof` (for example `true` or `1`): Python counts it as a
//!    proof; Rust rejects the pack (`proof` must be a string or null).

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
