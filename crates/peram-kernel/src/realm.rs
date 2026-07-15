//! Physical vs digital realm classification.
//! Port of ensembly `src/realm.js` core semantics.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::privacy::Classifiable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Realm {
    Physical,
    Digital,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealmInfo {
    pub realm: Realm,
    pub reason: String,
}

fn physical_kinds() -> &'static [&'static str] {
    &[
        "physical_errand",
        "physical_presence",
        "outdoor",
        "caregiving_in_person",
        "health_body",
        "household",
    ]
}

fn physical_res() -> &'static [Regex] {
    static RE: OnceLock<Vec<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        [
            r"(?i)\boutdoor\b",
            r"(?i)\bwalk\b",
            r"(?i)\bgrocer",
            r"(?i)\berrand\b",
            r"(?i)\bpick\s*up\b",
            r"(?i)\bin[- ]person\b",
            r"(?i)\bcook\b|\bmeal\s+prep\b",
        ]
        .into_iter()
        .map(|p| Regex::new(p).expect("physical regex"))
        .collect()
    })
}

/// Classify realm for an item.
pub fn classify_realm(item: &Classifiable) -> RealmInfo {
    // explicit realm via kind tags is preferred when encoded in tags
    if item.tags.iter().any(|t| t.eq_ignore_ascii_case("physical")) {
        return RealmInfo {
            realm: Realm::Physical,
            reason: "tag physical".into(),
        };
    }
    if item.tags.iter().any(|t| t.eq_ignore_ascii_case("digital")) {
        return RealmInfo {
            realm: Realm::Digital,
            reason: "tag digital".into(),
        };
    }

    let kind = item.kind.as_deref().unwrap_or("");
    if physical_kinds().contains(&kind) {
        return RealmInfo {
            realm: Realm::Physical,
            reason: format!("kind {kind}"),
        };
    }

    let text = format!(
        "{} {} {}",
        item.title.as_deref().unwrap_or(""),
        item.body.as_deref().unwrap_or(""),
        item.id.as_deref().unwrap_or("")
    );
    for re in physical_res() {
        if re.is_match(&text) {
            return RealmInfo {
                realm: Realm::Physical,
                reason: format!("matched {}", re.as_str()),
            };
        }
    }

    let area = item.area.as_deref().unwrap_or("");
    if area == "Relationships" || area == "Health" {
        return RealmInfo {
            realm: Realm::Physical,
            reason: format!("area {area} defaults physical when ambiguous"),
        };
    }

    RealmInfo {
        realm: Realm::Digital,
        reason: "default digital (agent-automatable)".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errand_is_physical() {
        let r = classify_realm(&Classifiable {
            id: Some("grocery-errand".into()),
            title: Some("Grocery errand".into()),
            kind: Some("physical_errand".into()),
            ..Default::default()
        });
        assert_eq!(r.realm, Realm::Physical);
    }

    #[test]
    fn career_default_digital() {
        let r = classify_realm(&Classifiable {
            title: Some("Prepare application".into()),
            area: Some("Career".into()),
            kind: Some("job_application_submit".into()),
            ..Default::default()
        });
        assert_eq!(r.realm, Realm::Digital);
    }
}
