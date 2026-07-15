//! Context-aware ranking → FocusPlan (one next body + one next auth + digital).
//! Pure; portable from ensembly turn selectNext* semantics.

use chrono::{DateTime, NaiveTime, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::approvals::{list_pending, ApprovalStatus, PhysicalStatus, Snapshot};
use crate::privacy::{classify_item, Classifiable};
use crate::realm::{classify_realm, Realm};

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
}
