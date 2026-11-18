//! Idempotency key derivation and content comparison for the record log.

use crate::record::{EventEnvelope, Source};
use sha2::{Digest, Sha256};

const KEY_MAX: usize = 200;

/// Compare semantic content for dedupe: ignore bus-stamped metadata.
pub fn same_idempotent_content<P: PartialEq>(a: &EventEnvelope<P>, b: &EventEnvelope<P>) -> bool {
    a.aggregate == b.aggregate
        && a.key == b.key
        && a.version == b.version
        && a.event_time == b.event_time
        && a.event_time_prec == b.event_time_prec
        && a.source == b.source
        && a.prev_hash == b.prev_hash
        && semantic_schema_v(a.schema_v) == semantic_schema_v(b.schema_v)
        && a.payload == b.payload
        && a.node == b.node
        && a.writer == b.writer
        && a.v2_optional.cause == b.v2_optional.cause
        && a.v2_optional.depth == b.v2_optional.depth
        && a.v2_optional.rule == b.v2_optional.rule
        && a.v2_optional.ext == b.v2_optional.ext
}

fn semantic_schema_v(v: u32) -> u32 {
    if v == 0 {
        1
    } else {
        v
    }
}

pub fn clamp_key(key: &str) -> Result<String, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("idempotency key is required".into());
    }
    if key.chars().count() > KEY_MAX {
        return Err(format!("idempotency key longer than {KEY_MAX} chars"));
    }
    Ok(key.to_string())
}

/// `d:` deterministic key when the source carries no stable id.
pub fn deterministic_key(
    node: &str,
    writer: &str,
    aggregate: &str,
    kind: &str,
    payload: &serde_json::Value,
    event_time: &str,
) -> String {
    let payload_canon = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
    let blob = format!("{node}\t{writer}\t{aggregate}\t{kind}\t{payload_canon}\t{event_time}");
    let hash = Sha256::digest(blob.as_bytes());
    format!("d:{}", hex::encode(&hash[..16]))
}

pub fn source_key(source: Source, stable_id: &str, rev: Option<&str>) -> String {
    let id = sha256_alias(stable_id);
    let src = source_wire_name(source);
    match rev {
        Some(r) => format!("src:{src}:{id}:{r}"),
        None => format!("src:{src}:{id}"),
    }
}

fn source_wire_name(source: Source) -> &'static str {
    match source {
        Source::Owner => "owner",
        Source::CollabFinder => "collab-finder",
        Source::CosCopy => "cos-copy",
        Source::Gmail => "gmail",
        Source::Linear => "linear",
        Source::Ats => "ats",
        Source::Human => "human",
        Source::Document => "document",
        Source::LegacyUnknown => "legacy-unknown",
    }
}

fn sha256_alias(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}
