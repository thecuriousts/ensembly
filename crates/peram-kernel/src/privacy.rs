//! Privacy classification — public vs private; default-deny.
//! Port of ensembly `src/privacy.js` semantics (parity tests).

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Classification {
    pub visibility: Visibility,
    pub reason: String,
    pub hitl: bool,
    pub pushable: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Classifiable {
    pub id: Option<String>,
    pub title: Option<String>,
    pub area: Option<String>,
    pub public: Option<bool>,
    pub kind: Option<String>,
    pub tags: Vec<String>,
    pub body: Option<String>,
}

fn hitl_kinds() -> &'static [&'static str] {
    &[
        "external_email_send",
        "job_application_submit",
        "calendar_mutate",
        "finance_transfer",
        "git_push_shared",
        "publish_private_data",
    ]
}

fn private_res() -> &'static [Regex] {
    static RE: OnceLock<Vec<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        [
            r"(?i)medical|dna|diagnosis|prescription|hospital|clinic",
            r"(?i)debt|a-kassa|pension|bank|transfer|salary|tax|invoice|account\s*#",
            r"(?i)passport|visa|oci\s*number|ssn|personnummer",
            r"(?i)address|phone\s*number|private\s*calendar",
            r"(?i)family\s*health|infant\s*medical|wife\s*medical",
        ]
        .into_iter()
        .map(|p| Regex::new(p).expect("privacy regex"))
        .collect()
    })
}

fn public_res() -> &'static [Regex] {
    static RE: OnceLock<Vec<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        [
            r"(?i)oss|open.?source|github|pr\b|commit|publish\s*skill|portfolio|devprofile",
            r"(?i)blog|build\s*in\s*public|x\.com|tweet",
        ]
        .into_iter()
        .map(|p| Regex::new(p).expect("public regex"))
        .collect()
    })
}

/// Classify a single action or project item (default-deny).
pub fn classify_item(item: &Classifiable) -> Classification {
    let title = item
        .title
        .clone()
        .or_else(|| item.id.clone())
        .unwrap_or_default();
    let body = item.body.clone().unwrap_or_default();
    let tags = item.tags.join(" ");
    let text = format!("{title} {body} {tags}");
    let area = item.area.clone().unwrap_or_default();
    let kind = item.kind.clone().unwrap_or_default();

    let high_stakes = hitl_kinds().contains(&kind.as_str())
        || Regex::new(r"(?i)submit|send_email|wire_transfer|force.?push")
            .unwrap()
            .is_match(&text);

    if item.public == Some(false) {
        return Classification {
            visibility: Visibility::Private,
            reason: "explicit public:false".into(),
            hitl: high_stakes,
            pushable: false,
        };
    }

    if area == "Finance" {
        return Classification {
            visibility: Visibility::Private,
            reason: "area Finance is local-only by default".into(),
            hitl: true,
            pushable: false,
        };
    }

    for re in private_res() {
        if re.is_match(&text) {
            return Classification {
                visibility: Visibility::Private,
                reason: format!("matched private pattern {}", re.as_str()),
                hitl: true,
                pushable: false,
            };
        }
    }

    if high_stakes {
        let financey = Regex::new(r"(?i)finance|transfer|bank")
            .unwrap()
            .is_match(&text)
            || kind == "finance_transfer";
        let visibility = if financey || item.public == Some(false) {
            Visibility::Private
        } else if item.public == Some(true) {
            Visibility::Public
        } else {
            Visibility::Private
        };
        return Classification {
            visibility,
            reason: format!("high-stakes kind/action requires HITL ({kind})"),
            hitl: true,
            pushable: item.public == Some(true) && !financey,
        };
    }

    if item.public == Some(true) || public_res().iter().any(|re| re.is_match(&text)) {
        return Classification {
            visibility: Visibility::Public,
            reason: if item.public == Some(true) {
                "explicit public:true".into()
            } else {
                "public craft/share signal".into()
            },
            hitl: false,
            pushable: true,
        };
    }

    Classification {
        visibility: Visibility::Private,
        reason: "default-deny: not marked public".into(),
        hitl: false,
        pushable: false,
    }
}

/// Paths that must remain local-only (contract).
pub fn private_path_patterns() -> Vec<&'static str> {
    vec![
        "private/",
        "data/local/",
        "data/private/",
        "*.local.json",
        "*.private.md",
        "*.private.json",
        ".env",
        ".env.*",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finance_area_private_hitl() {
        let c = classify_item(&Classifiable {
            title: Some("Review tax".into()),
            area: Some("Finance".into()),
            ..Default::default()
        });
        assert_eq!(c.visibility, Visibility::Private);
        assert!(c.hitl);
        assert!(!c.pushable);
    }

    #[test]
    fn public_oss_pushable() {
        let c = classify_item(&Classifiable {
            title: Some("Ship OSS skill".into()),
            public: Some(true),
            area: Some("Systems".into()),
            ..Default::default()
        });
        assert_eq!(c.visibility, Visibility::Public);
        assert!(c.pushable);
        assert!(!c.hitl);
    }

    #[test]
    fn default_deny() {
        let c = classify_item(&Classifiable {
            title: Some("Vague chore".into()),
            ..Default::default()
        });
        assert_eq!(c.visibility, Visibility::Private);
        assert!(!c.pushable);
    }

    #[test]
    fn bank_keyword_private() {
        let c = classify_item(&Classifiable {
            title: Some("Bank transfer setup".into()),
            ..Default::default()
        });
        assert_eq!(c.visibility, Visibility::Private);
        assert!(c.hitl);
    }
}
