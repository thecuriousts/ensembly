//! Strict JSON schemas for local fact classification and conflict checks.
//!
//! Fixtures and recorded outputs must be synthetic. Nothing here may hold
//! personal data, host names, or home paths.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Default floor: below this, the router asks the cloud fallback (if configured).
pub const DEFAULT_CONFIDENCE_FLOOR: f32 = 0.6;

/// Allowed fact-classification labels.
pub const FACT_LABELS: &[&str] = &["fact", "opinion", "instruction", "uncertain", "other"];

/// Allowed conflict relations.
pub const CONFLICT_RELATIONS: &[&str] = &["contradicts", "compatible", "underspecified"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactLabel {
    Fact,
    Opinion,
    Instruction,
    Uncertain,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictRelation {
    Contradicts,
    Compatible,
    Underspecified,
}

/// Classifier output. `rationale` is a short, non-identifying reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactClass {
    pub label: FactLabel,
    pub confidence: f32,
    pub rationale: String,
}

/// Conflict-check output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictClass {
    pub relation: ConflictRelation,
    pub confidence: f32,
    pub rationale: String,
}

impl FactClass {
    pub fn validate(&self) -> Result<(), String> {
        validate_confidence(self.confidence)?;
        validate_rationale(&self.rationale)?;
        Ok(())
    }
}

impl ConflictClass {
    pub fn validate(&self) -> Result<(), String> {
        validate_confidence(self.confidence)?;
        validate_rationale(&self.rationale)?;
        Ok(())
    }
}

pub fn validate_confidence(value: f32) -> Result<(), String> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(format!("confidence {value} is not in 0.0..=1.0"))
    }
}

pub fn validate_rationale(text: &str) -> Result<(), String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("rationale is empty".into());
    }
    if trimmed.chars().count() > 280 {
        return Err("rationale is longer than 280 characters".into());
    }
    Ok(())
}

/// JSON Schema used with llama.cpp `--json-schema` for fact classification.
pub fn fact_json_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "label": { "type": "string", "enum": FACT_LABELS },
            "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
            "rationale": { "type": "string", "minLength": 1, "maxLength": 280 }
        },
        "required": ["label", "confidence", "rationale"],
        "additionalProperties": false
    })
}

/// JSON Schema used with llama.cpp `--json-schema` for conflict checks.
pub fn conflict_json_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "relation": { "type": "string", "enum": CONFLICT_RELATIONS },
            "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
            "rationale": { "type": "string", "minLength": 1, "maxLength": 280 }
        },
        "required": ["relation", "confidence", "rationale"],
        "additionalProperties": false
    })
}

pub fn parse_fact_class(raw: &str) -> Result<FactClass, String> {
    let value = extract_json_object(raw)?;
    let parsed: FactClass = serde_json::from_value(value)
        .map_err(|e| format!("fact JSON does not match schema: {e}"))?;
    parsed.validate()?;
    Ok(parsed)
}

pub fn parse_conflict_class(raw: &str) -> Result<ConflictClass, String> {
    let value = extract_json_object(raw)?;
    let parsed: ConflictClass = serde_json::from_value(value)
        .map_err(|e| format!("conflict JSON does not match schema: {e}"))?;
    parsed.validate()?;
    Ok(parsed)
}

/// Pull the last JSON object out of model output (thinking text may precede it).
pub fn extract_json_object(raw: &str) -> Result<Value, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err("model output is empty".into());
    }
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        if value.is_object() {
            return Ok(value);
        }
    }
    if let Some(start) = text.rfind('{') {
        let mut depth = 0i32;
        for (idx, ch) in text[start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let end = start + idx + ch.len_utf8();
                        let slice = &text[start..end];
                        if let Ok(value) = serde_json::from_str::<Value>(slice) {
                            if value.is_object() {
                                return Ok(value);
                            }
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    Err("no JSON object in model output".into())
}

pub fn classify_system_prompt() -> &'static str {
    "You classify a single statement. Reply with one JSON object only, no markdown, no analysis. \
     label must be one of: fact, opinion, instruction, uncertain, other. \
     confidence is a number from 0 to 1. rationale is one short sentence."
}

pub fn conflict_system_prompt() -> &'static str {
    "You compare two statements. Reply with one JSON object only, no markdown, no analysis. \
     relation must be one of: contradicts, compatible, underspecified. \
     confidence is a number from 0 to 1. rationale is one short sentence."
}

pub fn classify_user_prompt(statement: &str) -> String {
    format!(
        "Classify this statement:\n{}\n\nJSON keys: label, confidence, rationale.",
        statement.trim()
    )
}

pub fn conflict_user_prompt(left: &str, right: &str) -> String {
    format!(
        "Statement A:\n{}\n\nStatement B:\n{}\n\nJSON keys: relation, confidence, rationale.",
        left.trim(),
        right.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_object() {
        let got = parse_fact_class(
            r#"{"label":"fact","confidence":0.91,"rationale":"States a countable stock figure."}"#,
        )
        .unwrap();
        assert_eq!(got.label, FactLabel::Fact);
        assert!((got.confidence - 0.91).abs() < f32::EPSILON);
    }

    #[test]
    fn extracts_object_after_thinking() {
        let raw = "thinking about it\n{\"relation\":\"contradicts\",\"confidence\":0.88,\"rationale\":\"Counts disagree.\"}\n";
        let got = parse_conflict_class(raw).unwrap();
        assert_eq!(got.relation, ConflictRelation::Contradicts);
    }

    #[test]
    fn rejects_out_of_range_confidence() {
        let err = parse_fact_class(r#"{"label":"fact","confidence":1.4,"rationale":"too sure"}"#)
            .unwrap_err();
        assert!(err.contains("0.0..=1.0"), "{err}");
    }

    #[test]
    fn rejects_unknown_label() {
        let err = parse_fact_class(r#"{"label":"true","confidence":0.5,"rationale":"nope"}"#)
            .unwrap_err();
        assert!(err.contains("schema"), "{err}");
    }
}
