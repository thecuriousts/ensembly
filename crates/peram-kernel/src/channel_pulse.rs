//! Channel pulse publish + weekday reconcile (Issue #8).
//! Read-only observation of wait-snapshot — never mutates G or gates.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::approvals::Snapshot;
use crate::runtime::Runtime;
use crate::store::OpsStore;
use crate::turn::{
    build_channel_ir, channel_pulse_content_hash, context_at, rank_now, Action, ChannelPulseIr,
    FocusPlan,
};

/// Default gitignored path for the redacted channel pulse artifact.
pub const DEFAULT_CHANNEL_PULSE_PATH: &str = "data/local/channel-pulse.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileReport {
    pub changed: bool,
    pub wrote: bool,
    pub content_hash: String,
    pub pulse_path: PathBuf,
}

/// Resolve FocusPlan from durable store (mirrors `peram turn` without persisting).
pub fn resolve_focus_plan(
    store: &OpsStore,
    actions: &[Action],
    location: Option<&str>,
) -> Result<(FocusPlan, Snapshot)> {
    let snap = store
        .load_snapshot()?
        .unwrap_or_else(|| Snapshot::empty(Utc::now()));
    let mut plan = rank_now(
        &context_at(Utc::now(), location),
        actions,
        &[],
        &snap,
    );
    if let Ok(Some(life)) = store.load_life_state() {
        let now = Utc::now();
        let mut rt = Runtime::new(now);
        rt.state = life;
        rt.snapshot = snap.clone();
        plan = rt.focus_plan(plan);
    }
    Ok((plan, snap))
}

/// Read an existing channel pulse file, if present.
pub fn read_channel_pulse(path: &Path) -> Result<Option<ChannelPulseIr>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read channel pulse {:?}", path))?;
    let ir: ChannelPulseIr =
        serde_json::from_str(&raw).with_context(|| format!("parse channel pulse {:?}", path))?;
    Ok(Some(ir))
}

/// Write channel pulse JSON (creates parent dirs).
pub fn write_channel_pulse(path: &Path, ir: &ChannelPulseIr) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create channel pulse dir {:?}", parent))?;
    }
    let json = serde_json::to_string_pretty(ir)?;
    fs::write(path, format!("{json}\n")).with_context(|| format!("write channel pulse {:?}", path))?;
    Ok(())
}

/// Diff current wait-snapshot projection vs last published pulse; write when changed.
pub fn reconcile_channel_pulse(
    store: &OpsStore,
    actions: &[Action],
    location: Option<&str>,
    pulse_path: &Path,
) -> Result<ReconcileReport> {
    let (plan, snap) = resolve_focus_plan(store, actions, location)?;
    let now = Utc::now();
    let current = build_channel_ir(&plan, &snap, now);
    let content_hash = channel_pulse_content_hash(&current);

    let prior = read_channel_pulse(pulse_path)?;
    let changed = prior
        .as_ref()
        .map(|p| channel_pulse_content_hash(p) != content_hash)
        .unwrap_or(true);

    let wrote = if changed {
        write_channel_pulse(pulse_path, &current)?;
        true
    } else {
        false
    };

    Ok(ReconcileReport {
        changed,
        wrote,
        content_hash,
        pulse_path: pulse_path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approvals::upsert_pending_from_actions;
    use crate::graph::DepGraph;
    use crate::turn::snapshot_channel_fingerprint;
    use chrono::TimeZone;
    use tempfile::tempdir;

    fn sample_actions() -> Vec<Action> {
        vec![
            Action {
                id: "grocery-errand".into(),
                title: "Grocery errand".into(),
                area: Some("Health".into()),
                kind: Some("physical_errand".into()),
                realm: Some("physical".into()),
                urgency: 4,
                importance: 4,
                tags: vec!["physical".into()],
                public: Some(false),
                depends_on: None,
                deadline_at: None,
            },
            Action {
                id: "apply-high-signal".into(),
                title: "Prepare FT application".into(),
                area: Some("Career".into()),
                kind: Some("job_application_submit".into()),
                realm: Some("digital".into()),
                urgency: 4,
                importance: 4,
                tags: vec!["digital".into()],
                public: Some(true),
                depends_on: None,
                deadline_at: None,
            },
        ]
    }

    #[test]
    fn reconcile_quiet_when_unchanged() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("ops.sqlite");
        let pulse = dir.path().join("channel-pulse.json");
        let store = OpsStore::open(&db).unwrap();
        let actions = sample_actions();
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let snap = upsert_pending_from_actions(
            &[(
                "apply-high-signal".into(),
                "Prepare FT application".into(),
                "job_application_submit".into(),
                true,
            )],
            None,
            now,
        );
        store.save_snapshot(&snap).unwrap();

        let first = reconcile_channel_pulse(&store, &actions, Some("home"), &pulse).unwrap();
        assert!(first.changed);
        assert!(first.wrote);

        let second = reconcile_channel_pulse(&store, &actions, Some("home"), &pulse).unwrap();
        assert!(!second.changed);
        assert!(!second.wrote);
    }

    #[test]
    fn reconcile_writes_redacted_pulse_shape() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("ops.sqlite");
        let pulse = dir.path().join("channel-pulse.json");
        let store = OpsStore::open(&db).unwrap();
        let actions = sample_actions();
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let snap = upsert_pending_from_actions(
            &[(
                "apply-high-signal".into(),
                "Prepare FT application".into(),
                "job_application_submit".into(),
                true,
            )],
            None,
            now,
        );
        store.save_snapshot(&snap).unwrap();

        reconcile_channel_pulse(&store, &actions, Some("home"), &pulse).unwrap();
        let raw = fs::read_to_string(&pulse).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["version"], 1);
        assert!(v.get("next_body").is_some());
        assert!(v.get("next_gate").is_some());
        assert!(v.get("generated_at").is_some());
        assert!(v.get("snapshot_fingerprint").is_some());
        assert!(v.get("coach_line").is_none());
    }

    #[test]
    fn reconcile_does_not_mutate_graph_or_gates() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("ops.sqlite");
        let pulse = dir.path().join("channel-pulse.json");
        let store = OpsStore::open(&db).unwrap();
        let actions = sample_actions();
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();

        let mut rt = Runtime::new(now);
        rt.load_actions(&actions, now).unwrap();
        store
            .save_runtime_pair(&rt.state, &rt.snapshot)
            .unwrap();
        let life_before = store.load_life_state().unwrap().unwrap();
        let snap_before = store.load_snapshot().unwrap().unwrap();
        let graph_before = DepGraph::from_actions(&actions, &Default::default()).unwrap();
        let node_count_before = graph_before.nodes.len();

        reconcile_channel_pulse(&store, &actions, None, &pulse).unwrap();

        let life_after = store.load_life_state().unwrap().unwrap();
        let snap_after = store.load_snapshot().unwrap().unwrap();
        let graph_after = DepGraph::from_actions(&actions, &Default::default()).unwrap();

        assert_eq!(life_before.fingerprint, life_after.fingerprint);
        assert_eq!(life_before.regime, life_after.regime);
        assert_eq!(life_before.graph.nodes.len(), life_after.graph.nodes.len());
        assert_eq!(
            snapshot_channel_fingerprint(&snap_before),
            snapshot_channel_fingerprint(&snap_after)
        );
        assert_eq!(node_count_before, graph_after.nodes.len());
    }
}
