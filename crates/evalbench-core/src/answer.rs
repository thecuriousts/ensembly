//! Frozen checker for model output. The trimmed text must be exactly
//! `{"gaps":[string, ...]}` (D7). Order is ignored; case and duplicates count.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnswerError {
    #[error("answer is not JSON: {0}")]
    NotJson(String),
    #[error("answer has the wrong shape: {0}")]
    WrongShape(String),
}

pub fn parse_answer(text: &str) -> Result<Vec<String>, AnswerError> {
    let value: serde_json::Value =
        serde_json::from_str(text.trim()).map_err(|e| AnswerError::NotJson(e.to_string()))?;
    let obj = value
        .as_object()
        .ok_or_else(|| AnswerError::WrongShape("not an object".into()))?;
    if obj.len() != 1 {
        return Err(AnswerError::WrongShape(
            "the object must hold only \"gaps\"".into(),
        ));
    }
    let gaps = obj
        .get("gaps")
        .ok_or_else(|| AnswerError::WrongShape("no \"gaps\" key".into()))?
        .as_array()
        .ok_or_else(|| AnswerError::WrongShape("\"gaps\" is not an array".into()))?;
    gaps.iter()
        .map(|g| {
            g.as_str()
                .map(str::to_string)
                .ok_or_else(|| AnswerError::WrongShape("a gap is not a string".into()))
        })
        .collect()
}

/// `parse_answer` is ok and sorted(answer) == sorted(expected).
pub fn check_answer(expected: &[String], text: &str) -> bool {
    match parse_answer(text) {
        Ok(mut got) => {
            let mut want = expected.to_vec();
            got.sort();
            want.sort();
            got == want
        }
        Err(_) => false,
    }
}
