//! Read-only plain views from `life_state` inside an ops bundle JSON (SoT remains ops JSON).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OpsBundleJson {
    kv: Vec<KvRowJson>,
}

#[derive(Debug, Deserialize)]
struct KvRowJson {
    key: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct LifeStateJson {
    graph: GraphJson,
}

#[derive(Debug, Deserialize)]
struct GraphJson {
    nodes: BTreeMap<String, TaskNodeJson>,
}

#[derive(Debug, Deserialize)]
struct TaskNodeJson {
    id: String,
    title: String,
    status: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    deadline_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedViews {
    pub todos_csv: Vec<u8>,
    pub chores_csv: Vec<u8>,
    pub deadlines_ics: Vec<u8>,
}

fn is_chore(kind: Option<&str>) -> bool {
    matches!(kind, Some(k) if k.eq_ignore_ascii_case("chore"))
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn build_csv(rows: &[&TaskNodeJson]) -> Vec<u8> {
    let mut out = String::from("id,title,status,kind,deadline_at\n");
    for n in rows {
        let kind = n.kind.as_deref().unwrap_or("");
        let deadline = n
            .deadline_at
            .map(|d| d.to_rfc3339())
            .unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_escape(&n.id),
            csv_escape(&n.title),
            csv_escape(&n.status),
            csv_escape(kind),
            csv_escape(&deadline),
        ));
    }
    out.into_bytes()
}

fn fmt_ics_utc(dt: DateTime<Utc>) -> String {
    dt.format("%Y%m%dT%H%M%SZ").to_string()
}

fn build_ics(chores: &[&TaskNodeJson], stamp: DateTime<Utc>) -> Vec<u8> {
    let stamp = fmt_ics_utc(stamp);
    let mut out = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//ensembly//portable-bundle//EN\r\n");
    for n in chores {
        let Some(dl) = n.deadline_at else {
            continue;
        };
        out.push_str("BEGIN:VEVENT\r\n");
        out.push_str(&format!("UID:{}@ensembly-bundle\r\n", n.id));
        out.push_str(&format!("DTSTAMP:{stamp}\r\n"));
        out.push_str(&format!("DTSTART:{}\r\n", fmt_ics_utc(dl)));
        out.push_str(&format!("SUMMARY:{}\r\n", n.title.replace('\n', " ")));
        out.push_str("END:VEVENT\r\n");
    }
    out.push_str("END:VCALENDAR\r\n");
    out.into_bytes()
}

/// Derive todos/chores CSV and chore deadline ICS from ops bundle bytes.
pub fn derive_views_from_ops_bundle(
    ops_json: &[u8],
    ics_stamp: DateTime<Utc>,
) -> Result<DerivedViews, String> {
    let bundle: OpsBundleJson =
        serde_json::from_slice(ops_json).map_err(|e| format!("ops bundle json: {e}"))?;
    let life = bundle
        .kv
        .iter()
        .find(|r| r.key == "life_state")
        .ok_or_else(|| "ops bundle has no life_state kv row".to_string())?;
    let state: LifeStateJson =
        serde_json::from_str(&life.value).map_err(|e| format!("life_state json: {e}"))?;
    let mut nodes: Vec<&TaskNodeJson> = state.graph.nodes.values().collect();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let chores: Vec<&TaskNodeJson> = nodes
        .iter()
        .copied()
        .filter(|n| is_chore(n.kind.as_deref()))
        .collect();
    let todos: Vec<&TaskNodeJson> = nodes
        .iter()
        .copied()
        .filter(|n| !is_chore(n.kind.as_deref()))
        .collect();
    Ok(DerivedViews {
        todos_csv: build_csv(&todos),
        chores_csv: build_csv(&chores),
        deadlines_ics: build_ics(&chores, ics_stamp),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn csv_and_ics_well_formed() {
        let dl = Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();
        let ops = serde_json::json!({
            "kv": [{
                "key": "life_state",
                "value": serde_json::json!({
                    "graph": {
                        "nodes": {
                            "t1": {"id":"t1","title":"Inbox","status":"open","kind":"todo"},
                            "c1": {"id":"c1","title":"Dishes","status":"open","kind":"chore","deadline_at": dl},
                        },
                        "edges": []
                    }
                }).to_string(),
                "updated_at": "2026-01-01T00:00:00Z"
            }]
        });
        let raw = serde_json::to_vec(&ops).unwrap();
        let v = derive_views_from_ops_bundle(&raw, Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap())
            .unwrap();
        let todos = std::str::from_utf8(&v.todos_csv).unwrap();
        assert!(todos.contains("t1,Inbox"));
        let chores = std::str::from_utf8(&v.chores_csv).unwrap();
        assert!(chores.contains("c1,Dishes"));
        let ics = std::str::from_utf8(&v.deadlines_ics).unwrap();
        assert!(ics.contains("BEGIN:VCALENDAR"));
        assert!(ics.contains("BEGIN:VEVENT"));
        assert!(ics.contains("20260615T120000Z"));
    }
}