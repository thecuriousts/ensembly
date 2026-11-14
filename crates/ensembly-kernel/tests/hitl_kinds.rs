use ensembly_kernel::{hitl, privacy::classify_item, privacy::Classifiable};

#[test]
fn external_form_submit_and_legacy_alias_share_hitl() {
    for kind in [hitl::EXTERNAL_FORM_SUBMIT, hitl::JOB_APPLICATION_SUBMIT] {
        let c = classify_item(&Classifiable {
            kind: Some(kind.into()),
            ..Default::default()
        });
        assert!(c.hitl, "{kind}");
    }
}
