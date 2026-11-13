//! Frozen answer checker and the frozen prompt.
mod common;
use common::*;
use evalbench_core::answer::{check_answer, parse_answer, AnswerError};
use evalbench_core::runner::{render_prompt, PROMPT_ID, PROMPT_TEMPLATE};

const PROMPT_SHA256: &str = "db35a8ff4710d105449cb25302d914fdc59cb2449d68be1aa72d9f10cf5efc40";

#[test]
fn answer_passes_only_on_exact_gap_set() {
    let want = names(&["RAG", "Observability"]);
    assert!(check_answer(&want, r#"{"gaps":["RAG","Observability"]}"#));
    assert!(
        check_answer(&want, " {\"gaps\":[\"Observability\",\"RAG\"]}\n"),
        "order and outer whitespace ignored"
    );
    assert!(
        !check_answer(&want, r#"{"gaps":["rag","Observability"]}"#),
        "case counts"
    );
    assert!(
        !check_answer(&want, r#"{"gaps":["RAG","RAG","Observability"]}"#),
        "duplicates count"
    );
    assert!(!check_answer(&want, r#"{"gaps":["RAG"]}"#));
    assert!(check_answer(&[], r#"{"gaps":[]}"#));
    assert_eq!(
        parse_answer(r#"{"gaps":["b","a"]}"#),
        Ok(names(&["b", "a"]))
    );
}

#[test]
fn unparseable_answer_fails() {
    let want = names(&["RAG"]);
    for bad in [
        "```json\n{\"gaps\":[\"RAG\"]}\n```",
        "The gaps are: RAG",
        r#"{"missing":["RAG"]}"#,
        r#"{"gaps":["RAG"],"note":"x"}"#,
        r#"{"gaps":["RAG",1]}"#,
        r#"["RAG"]"#,
        "",
    ] {
        assert!(!check_answer(&want, bad), "{bad:?} must fail");
        assert!(parse_answer(bad).is_err(), "{bad:?} must not parse");
    }
    assert!(matches!(parse_answer("nope"), Err(AnswerError::NotJson(_))));
    assert!(matches!(
        parse_answer(r#"{"gaps":"RAG"}"#),
        Err(AnswerError::WrongShape(_))
    ));
}

/// Guard (passes today): the prompt is frozen. Changing it needs a new PROMPT_ID.
#[test]
fn prompt_template_is_frozen() {
    assert_eq!(PROMPT_ID, "gaps-v1");
    assert_eq!(sha256_hex(PROMPT_TEMPLATE.as_bytes()), PROMPT_SHA256);
    assert_eq!(PROMPT_TEMPLATE.matches("{{posting}}").count(), 1);
    assert_eq!(PROMPT_TEMPLATE.matches("{{pack}}").count(), 1);
}

#[test]
fn render_prompt_inserts_raw_files_once() {
    let set = frozen_set();
    let t = &set.tasks[0];
    let p = render_prompt(t);
    assert!(!p.contains("{{posting}}") && !p.contains("{{pack}}"));
    assert_eq!(p.matches(t.posting_raw.as_str()).count(), 1);
    assert_eq!(p.matches(t.pack_raw.as_str()).count(), 1);
    assert!(p.starts_with("You check a job application pack"));
}
