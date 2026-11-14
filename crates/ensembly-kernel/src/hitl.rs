//! Human-in-the-loop digital action kinds (default core — not job-hunt-specific).

/// High-stakes submit to an external form (applications, permits, etc.).
pub const EXTERNAL_FORM_SUBMIT: &str = "external_form_submit";

/// Legacy alias kept for fixtures and harnesses written before the rename.
pub const JOB_APPLICATION_SUBMIT: &str = "job_application_submit";

/// Kinds that require explicit authorization before execution.
pub fn hitl_kinds() -> &'static [&'static str] {
    &[
        "external_email_send",
        EXTERNAL_FORM_SUBMIT,
        "calendar_mutate",
        "finance_transfer",
        "git_push_shared",
        "publish_private_data",
    ]
}

pub fn is_hitl_kind(kind: &str) -> bool {
    kind == JOB_APPLICATION_SUBMIT || hitl_kinds().contains(&kind)
}

/// Auth-gate kinds mirrored in the dep graph and channel pulse reconcile.
pub fn action_requires_auth_gate(kind: Option<&str>) -> bool {
    matches!(
        kind,
        Some(EXTERNAL_FORM_SUBMIT)
            | Some(JOB_APPLICATION_SUBMIT)
            | Some("finance_transfer")
            | Some("external_email_send")
            | Some("calendar_mutate")
            | Some("git_push_shared")
            | Some("publish_private_data")
            | Some("bill_pay")
    )
}
