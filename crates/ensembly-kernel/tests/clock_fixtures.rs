//! Parser fixtures for clock E1 (chronyc CSV + sntp text variants).

use ensembly_kernel::{parse_chronyc_tracking_csv, parse_sntp_output};
use std::fs;

#[test]
fn chronyc_fixture_file() {
    let line = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/clock/chronyc-tracking.csv"
    ))
    .unwrap();
    let (off_us, err_us) = parse_chronyc_tracking_csv(&line).unwrap();
    assert_eq!(off_us, 250);
    assert_eq!(err_us, 11_750);
}

#[test]
fn sntp_fixtures_on_disk() {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/clock/");
    let pm = fs::read_to_string(format!("{base}sntp-plusminus.txt")).unwrap();
    let (off, err) = parse_sntp_output(&pm).unwrap();
    assert_eq!(off, 1234);
    assert_eq!(err, 5678);
    let od = fs::read_to_string(format!("{base}sntp-offset-delay.txt")).unwrap();
    let (off, err) = parse_sntp_output(&od).unwrap();
    assert_eq!(off, 2000);
    assert_eq!(err, 6000);
}
