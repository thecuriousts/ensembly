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
    let _ = text;
    unimplemented!("E1: parse_answer")
}

/// `parse_answer` is ok and sorted(answer) == sorted(expected).
pub fn check_answer(expected: &[String], text: &str) -> bool {
    let _ = (expected, text);
    unimplemented!("E1: check_answer")
}
