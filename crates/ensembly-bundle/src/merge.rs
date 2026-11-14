use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvRow {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsBundleMerge {
    pub format: String,
    pub schema_version: i64,
    pub exported_at: String,
    pub kv: Vec<KvRow>,
}

/// Union kv rows by key; on conflict keep the row with the lexicographically greatest `updated_at`
/// (ties → incoming wins). Never drops a key present in either side.
pub fn merge_ops_bundles(existing: &OpsBundleMerge, incoming: &OpsBundleMerge) -> OpsBundleMerge {
    let mut map: BTreeMap<String, KvRow> = BTreeMap::new();
    for row in &existing.kv {
        map.insert(row.key.clone(), row.clone());
    }
    for row in &incoming.kv {
        map.entry(row.key.clone())
            .and_modify(|prev| {
                if row.updated_at >= prev.updated_at {
                    *prev = row.clone();
                }
            })
            .or_insert(row.clone());
    }
    let mut kv: Vec<KvRow> = map.into_values().collect();
    kv.sort_by(|a, b| a.key.cmp(&b.key));
    OpsBundleMerge {
        format: incoming.format.clone(),
        schema_version: incoming.schema_version.max(existing.schema_version),
        exported_at: incoming.exported_at.clone(),
        kv,
    }
}

/// Append lines from both ledgers; dedupe identical lines; preserve order (existing then new uniqued).
pub fn merge_jsonl_text(existing: &str, incoming: &str) -> String {
    let mut seen = BTreeSet::new();
    let mut out = String::new();
    for block in [existing, incoming] {
        for line in block.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            if seen.insert(t.to_string()) {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(t);
            }
        }
    }
    if out.is_empty() {
        String::new()
    } else {
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(k: &str, u: &str) -> KvRow {
        KvRow {
            key: k.into(),
            value: format!("v-{k}"),
            updated_at: u.into(),
        }
    }

    #[test]
    fn merge_ops_never_drops_keys() {
        let a = OpsBundleMerge {
            format: "f".into(),
            schema_version: 1,
            exported_at: "t".into(),
            kv: vec![row("alpha", "2026-01-01T00:00:00Z"), row("beta", "2026-01-01T00:00:00Z")],
        };
        let b = OpsBundleMerge {
            format: "f".into(),
            schema_version: 1,
            exported_at: "t2".into(),
            kv: vec![row("beta", "2026-02-01T00:00:00Z"), row("gamma", "2026-02-01T00:00:00Z")],
        };
        let m = merge_ops_bundles(&a, &b);
        let keys: Vec<_> = m.kv.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec!["alpha", "beta", "gamma"]);
        assert_eq!(
            m.kv.iter().find(|r| r.key == "beta").unwrap().updated_at,
            "2026-02-01T00:00:00Z"
        );
    }

    #[test]
    fn merge_jsonl_dedupes_lines() {
        let merged = merge_jsonl_text("{\"a\":1}\n", "{\"a\":1}\n{\"b\":2}\n");
        assert_eq!(merged, "{\"a\":1}\n{\"b\":2}\n");
    }
}