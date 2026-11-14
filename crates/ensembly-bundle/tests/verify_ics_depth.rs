use ensembly_bundle::verify_ics_content;

#[test]
fn verify_rejects_unclosed_vevent() {
    let bad = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nSUMMARY:x\r\nEND:VCALENDAR\r\n";
    let err = verify_ics_content(bad).unwrap_err();
    assert!(err.contains("VEVENT"), "{err}");
}

#[test]
fn verify_accepts_minimal_valid_calendar() {
    let ok = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//t//\r\nBEGIN:VEVENT\r\nUID:1\r\nDTSTAMP:20260101T000000Z\r\nSUMMARY:x\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    verify_ics_content(ok).unwrap();
}
