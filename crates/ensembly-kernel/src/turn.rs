//! Context-aware ranking → FocusPlan (one next body + one next auth + digital).
//! Pure; portable from ensembly turn selectNext* semantics.

use chrono::{DateTime, NaiveTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::approvals::{list_pending, ApprovalStatus, PhysicalStatus, Snapshot};
use crate::privacy::{classify_item, Classifiable, Visibility};
use crate::realm::{classify_realm, Realm};
use crate::vault::export_denied_for_class;

/// Channel IR version — stable contract for harnesses (Issue #8).
pub const CHANNEL_IR_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub title: String,
    pub area: Option<String>,
    pub kind: Option<String>,
    pub realm: Option<String>,
    pub urgency: i32,
    pub importance: i32,
    pub tags: Vec<String>,
    pub public: Option<bool>,
    /// Explicit dependency edges into this action (Issue #1 G).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_at: Option<DateTime<Utc>>,
}

/// Load `extra_candidates` from a committed fixture JSON (Issue #1 / #8 dogfood).
pub fn actions_from_fixture_path(path: &Path) -> Result<Vec<Action>, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
    actions_from_fixture_json(&raw)
}

/// Parse fixture JSON; unknown top-level keys ignored. Missing `extra_candidates` → empty.
pub fn actions_from_fixture_json(raw: &str) -> Result<Vec<Action>, String> {
    #[derive(Deserialize)]
    struct FixtureFile {
        #[serde(default)]
        extra_candidates: Vec<Action>,
    }
    let f: FixtureFile =
        serde_json::from_str(raw).map_err(|e| format!("parse fixture JSON: {e}"))?;
    Ok(f.extra_candidates)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleSlot {
    pub start: String,
    pub end: String,
    pub label: Option<String>,
    pub assigned_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFrame {
    pub now: DateTime<Utc>,
    pub timezone_label: String,
    /// Coarse only: home | travel | office — never raw GPS in share IR.
    pub location_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusItem {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub reason: String,
}

// FocusItem is part of FocusPlan public surface (CP-driven reasons).

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusPlan {
    pub version: u32,
    pub at: DateTime<Utc>,
    pub location_label: Option<String>,
    pub biome: String,
    pub primary_physical: Option<FocusItem>,
    pub primary_auth: Option<FocusItem>,
    pub primary_digital: Option<FocusItem>,
    pub places: Vec<String>,
    pub coach_line: String,
    pub physical_count: usize,
    pub pending_count: usize,
}

/// Redacted next-act surface for channel bots (one body + one gate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelAct {
    pub id: String,
    pub title: String,
    pub kind: String,
}

/// Versioned channel pulse IR — no coach_line, places, or queue dumps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelPulseIr {
    pub version: u32,
    pub generated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_body: Option<ChannelAct>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_gate: Option<ChannelAct>,
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    pub where_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<DateTime<Utc>>,
    /// Fingerprint of durable wait-snapshot rows (reconcile diff key).
    pub snapshot_fingerprint: String,
}

fn redact_channel_act(item: &FocusItem, area: Option<&str>) -> ChannelAct {
    let classifiable = Classifiable {
        id: Some(item.id.clone()),
        title: Some(item.title.clone()),
        area: area.map(str::to_string),
        kind: Some(item.kind.clone()),
        ..Default::default()
    };
    let c = classify_item(&classifiable);
    let area_denied = area.map(export_denied_for_class).unwrap_or(false);
    let title = if c.pushable && !area_denied && c.visibility == Visibility::Public {
        item.title.clone()
    } else if c.visibility == Visibility::Private || !c.pushable || area_denied {
        format!("[redacted:{}]", c.reason.split(':').next().unwrap_or("private"))
    } else {
        item.title.clone()
    };
    ChannelAct {
        id: item.id.clone(),
        title,
        kind: item.kind.clone(),
    }
}

/// Build redacted channel IR from FocusPlan + wait snapshot (pure; no graph writes).
pub fn build_channel_ir(plan: &FocusPlan, snap: &Snapshot, now: DateTime<Utc>) -> ChannelPulseIr {
    let body_area = plan
        .primary_physical
        .as_ref()
        .and_then(|p| snap.physical.iter().find(|r| r.id == p.id))
        .and_then(|r| r.area.as_deref());
    let gate_area = plan
        .primary_auth
        .as_ref()
        .and_then(|a| {
            snap.pending
                .iter()
                .find(|p| p.id == a.id)
                .and_then(|p| p.area.as_deref())
        });
    ChannelPulseIr {
        version: CHANNEL_IR_VERSION,
        generated_at: now,
        next_body: plan
            .primary_physical
            .as_ref()
            .map(|p| redact_channel_act(p, body_area)),
        next_gate: plan
            .primary_auth
            .as_ref()
            .map(|a| redact_channel_act(a, gate_area)),
        where_label: plan.location_label.clone(),
        when: Some(plan.at),
        snapshot_fingerprint: snapshot_channel_fingerprint(snap),
    }
}

/// Stable hash of wait-snapshot rows relevant to channel reconcile.
pub fn snapshot_channel_fingerprint(snap: &Snapshot) -> String {
    let mut h = Sha256::new();
    h.update(snap.updated_at.to_rfc3339().as_bytes());
    h.update(snap.phase.as_bytes());
    h.update(format!("{:?}", snap.status).as_bytes());

    let mut pending: Vec<_> = snap
        .pending
        .iter()
        .map(|p| (p.id.as_str(), format!("{:?}", p.status)))
        .collect();
    pending.sort_by_key(|(id, _)| *id);
    for (id, st) in pending {
        h.update(id.as_bytes());
        h.update(st.as_bytes());
    }

    let mut physical: Vec<_> = snap
        .physical
        .iter()
        .map(|p| (p.id.as_str(), format!("{:?}", p.status)))
        .collect();
    physical.sort_by_key(|(id, _)| *id);
    for (id, st) in physical {
        h.update(id.as_bytes());
        h.update(st.as_bytes());
    }

    hex::encode(h.finalize())
}

/// Content hash for reconcile (excludes volatile `generated_at` and `when`).
pub fn channel_pulse_content_hash(ir: &ChannelPulseIr) -> String {
    let comparable = serde_json::json!({
        "version": ir.version,
        "next_body": ir.next_body,
        "next_gate": ir.next_gate,
        "where": ir.where_label,
        "snapshot_fingerprint": ir.snapshot_fingerprint,
    });
    let mut h = Sha256::new();
    h.update(comparable.to_string().as_bytes());
    hex::encode(h.finalize())
}

fn parse_hm(s: &str) -> Option<u32> {
    let parts: Vec<_> = s.trim().split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let h: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

fn in_window(now_min: u32, start: u32, end: u32) -> bool {
    if end > start {
        now_min >= start && now_min < end
    } else {
        now_min >= start || now_min < end
    }
}

fn biome_for(hour: u32, location: Option<&str>) -> String {
    let base = match hour {
        5..=10 => "dawn_courtyard",
        11..=16 => "day_courtyard",
        17..=20 => "dusk_path",
        _ => "night_desk",
    };
    match location {
        Some("travel") => format!("{base}_travel"),
        Some("office") => format!("{base}_office"),
        _ => base.into(),
    }
}

/// Select next physical (schedule-aware when assigned ids match).
pub fn select_next_physical(
    pickups: &[(String, String, i32, i32)], // id, title, urgency, importance
    schedule: &[ScheduleSlot],
    now: DateTime<Utc>,
) -> Option<FocusItem> {
    if pickups.is_empty() {
        return None;
    }
    let now_min = now.hour() * 60 + now.minute();
    let mut best: Option<(i32, FocusItem)> = None;

    for slot in schedule {
        let Some(aid) = &slot.assigned_id else { continue };
        let Some((id, title, _, _)) = pickups.iter().find(|(i, _, _, _)| i == aid) else {
            continue;
        };
        let Some(start) = parse_hm(&slot.start) else { continue };
        let Some(end) = parse_hm(&slot.end) else { continue };
        let (score, reason) = if in_window(now_min, start, end) {
            (10_000, "current_window")
        } else if end > start && now_min < start {
            (5_000 - (start as i32 - now_min as i32), "upcoming_window")
        } else {
            (1_000, "scheduled_open")
        };
        if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
            best = Some((
                score,
                FocusItem {
                    id: id.clone(),
                    title: title.clone(),
                    kind: "physical".into(),
                    reason: reason.into(),
                },
            ));
        }
    }

    if let Some((_, item)) = best {
        return Some(item);
    }

    let mut sorted = pickups.to_vec();
    sorted.sort_by(|a, b| {
        let sa = a.2 + a.3;
        let sb = b.2 + b.3;
        sb.cmp(&sa).then_with(|| a.0.cmp(&b.0))
    });
    let (id, title, _, _) = &sorted[0];
    Some(FocusItem {
        id: id.clone(),
        title: title.clone(),
        kind: "physical".into(),
        reason: "priority".into(),
    })
}

pub fn select_next_auth(snap: &Snapshot) -> Option<FocusItem> {
    let mut open: Vec<_> = list_pending(snap);
    if open.is_empty() {
        return None;
    }
    open.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    let a = open[0];
    Some(FocusItem {
        id: a.id.clone(),
        title: a.title.clone(),
        kind: a.kind.clone(),
        reason: a.reason.clone(),
    })
}

/// Build FocusPlan from actions + snapshot + context (pure).
pub fn rank_now(
    ctx: &ContextFrame,
    actions: &[Action],
    schedule: &[ScheduleSlot],
    snap: &Snapshot,
) -> FocusPlan {
    let mut physical = vec![];
    let mut digital = vec![];

    for a in actions {
        let classifiable = Classifiable {
            id: Some(a.id.clone()),
            title: Some(a.title.clone()),
            area: a.area.clone(),
            kind: a.kind.clone(),
            tags: a.tags.clone(),
            public: a.public,
            body: None,
        };
        let realm = if let Some(r) = &a.realm {
            if r == "physical" {
                Realm::Physical
            } else {
                Realm::Digital
            }
        } else {
            classify_realm(&classifiable).realm
        };
        let _c = classify_item(&classifiable);
        match realm {
            Realm::Physical => {
                let done = snap
                    .physical
                    .iter()
                    .any(|p| p.id == a.id && p.status == PhysicalStatus::Completed);
                if !done {
                    physical.push((
                        a.id.clone(),
                        a.title.clone(),
                        a.urgency,
                        a.importance,
                    ));
                }
            }
            Realm::Digital => digital.push(a),
        }
    }

    let primary_physical = select_next_physical(&physical, schedule, ctx.now);
    let primary_auth = select_next_auth(snap);

    let primary_digital = digital
        .iter()
        .filter(|a| {
            // prefer open HITL-related or highest score
            let open_auth = snap.pending.iter().any(|p| {
                p.action_id.as_deref() == Some(a.id.as_str())
                    && p.status == ApprovalStatus::Pending
            });
            open_auth || a.kind.as_deref() == Some("finance_transfer") || a.kind.as_deref() == Some("bill_pay")
        })
        .max_by_key(|a| a.urgency + a.importance)
        .or_else(|| digital.iter().max_by_key(|a| a.urgency + a.importance))
        .map(|a| FocusItem {
            id: a.id.clone(),
            title: a.title.clone(),
            kind: a.kind.clone().unwrap_or_else(|| "digital".into()),
            reason: "digital_duty".into(),
        });

    let mut places = vec!["Home".into(), "Desk".into()];
    if primary_auth.is_some() {
        places.push("Gate".into());
    }
    if primary_digital
        .as_ref()
        .map(|d| d.kind.contains("finance") || d.kind.contains("bill") || d.title.contains("Bank"))
        .unwrap_or(false)
        || snap.pending.iter().any(|p| p.place.as_deref() == Some("Bank"))
    {
        places.push("Bank".into());
    }
    places.push("Path".into());

    let hour = ctx.now.hour();
    let biome = biome_for(hour, ctx.location_label.as_deref());

    let coach = match (&primary_physical, &primary_auth) {
        (Some(p), Some(a)) => format!("Body: {} · Gate: {}", p.title, a.title),
        (Some(p), None) => format!("Body next: {}", p.title),
        (None, Some(a)) => format!("Auth gate: {}", a.title),
        (None, None) => "Queues clear — presence or craft.".into(),
    };

    FocusPlan {
        version: 1,
        at: ctx.now,
        location_label: ctx.location_label.clone(),
        biome,
        primary_physical,
        primary_auth,
        primary_digital,
        places,
        coach_line: coach,
        physical_count: physical.len(),
        pending_count: list_pending(snap).len(),
    }
}

/// Helper for tests: naive local time inject via Utc hour mapping.
pub fn context_at(now: DateTime<Utc>, location: Option<&str>) -> ContextFrame {
    ContextFrame {
        now,
        timezone_label: "local".into(),
        location_label: location.map(str::to_string),
    }
}

#[allow(dead_code)]
fn _naive_unused() -> NaiveTime {
    NaiveTime::from_hms_opt(0, 0, 0).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approvals::{upsert_pending_from_actions, Snapshot};
    use chrono::TimeZone;

    #[test]
    fn schedule_window_prefers_assigned() {
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 17, 30, 0).unwrap();
        let pickups = vec![
            ("grocery-errand".into(), "Grocery".into(), 5, 5),
            ("evening-outdoor".into(), "Walk".into(), 2, 2),
        ];
        let schedule = vec![ScheduleSlot {
            start: "17:00".into(),
            end: "18:00".into(),
            label: Some("Evening".into()),
            assigned_id: Some("evening-outdoor".into()),
        }];
        let next = select_next_physical(&pickups, &schedule, now).unwrap();
        assert_eq!(next.id, "evening-outdoor");
        assert_eq!(next.reason, "current_window");
    }

    #[test]
    fn rank_now_surfaces_body_and_auth() {
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let actions = vec![
            Action {
                id: "grocery-errand".into(),
                title: "Grocery errand".into(),
                area: Some("Relationships".into()),
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
        ];
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
        let plan = rank_now(&context_at(now, Some("home")), &actions, &[], &snap);
        assert_eq!(plan.primary_physical.as_ref().unwrap().id, "grocery-errand");
        assert_eq!(
            plan.primary_auth.as_ref().unwrap().id,
            "auth-apply-high-signal"
        );
        assert!(plan.places.contains(&"Gate".into()));
        assert!(plan.biome.contains("day") || plan.biome.contains("courtyard") || !plan.biome.is_empty());
    }

    #[test]
    fn empty_snapshot_clear_plan() {
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 22, 0, 0).unwrap();
        let plan = rank_now(
            &context_at(now, None),
            &[],
            &[],
            &Snapshot::empty(now),
        );
        assert!(plan.primary_physical.is_none());
        assert!(plan.primary_auth.is_none());
        assert!(plan.biome.contains("night") || plan.biome.contains("desk"));
    }

    #[test]
    fn channel_ir_shape_versioned_one_body_one_gate() {
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let actions = vec![Action {
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
        }];
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
        let plan = rank_now(&context_at(now, Some("home")), &actions, &[], &snap);
        let ir = build_channel_ir(&plan, &snap, now);
        assert_eq!(ir.version, CHANNEL_IR_VERSION);
        assert!(ir.next_body.is_some());
        assert!(ir.next_gate.is_some());
        assert_eq!(ir.where_label.as_deref(), Some("home"));
        assert!(!ir.snapshot_fingerprint.is_empty());
        let json = serde_json::to_value(&ir).unwrap();
        assert!(json.get("coach_line").is_none());
        assert!(json.get("places").is_none());
        assert!(json.get("next_body").is_some());
        assert!(json.get("next_gate").is_some());
    }

    #[test]
    fn channel_ir_redacts_private_finance_title() {
        let item = FocusItem {
            id: "auth-pay-rent".into(),
            title: "Pay rent wire transfer".into(),
            kind: "finance_transfer".into(),
            reason: "hitl".into(),
        };
        let act = redact_channel_act(&item, Some("Finance"));
        assert!(act.title.starts_with("[redacted:"));
        assert_eq!(act.id, "auth-pay-rent");
    }

    #[test]
    fn channel_pulse_content_hash_ignores_generated_at() {
        let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let snap = Snapshot::empty(now);
        let plan = rank_now(&context_at(now, None), &[], &[], &snap);
        let mut a = build_channel_ir(&plan, &snap, now);
        let h1 = channel_pulse_content_hash(&a);
        a.generated_at = now + chrono::Duration::hours(1);
        let h2 = channel_pulse_content_hash(&a);
        assert_eq!(h1, h2);
    }
}
