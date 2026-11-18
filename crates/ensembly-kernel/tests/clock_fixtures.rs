//! Parser fixtures for clock E1 (chronyc CSV + sntp text variants).

use ensembly_kernel::{parse_chronyc_tracking_csv, parse_sntp_output, CLOCK_UNKNOWN_ERR_US};
use std::fs;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/clock/");

#[test]
fn chronyc_fixture_file() {
    let line = fs::read_to_string(format!("{FIXTURES}chronyc-tracking.csv")).unwrap();
    let parsed = parse_chronyc_tracking_csv(&line).unwrap();
    assert!(parsed.synchronized);
    assert_eq!(parsed.off_us, 250);
    assert_eq!(parsed.err_us, 11_750);
}

#[test]
fn chronyc_unsync_fixture_file() {
    let line = fs::read_to_string(format!("{FIXTURES}chronyc-not-sync.csv")).unwrap();
    let parsed = parse_chronyc_tracking_csv(&line).unwrap();
    assert!(!parsed.synchronized);
    assert_eq!(parsed.err_us, CLOCK_UNKNOWN_ERR_US);
}

#[test]
fn sntp_fixtures_on_disk() {
    let pm = fs::read_to_string(format!("{FIXTURES}sntp-plusminus.txt")).unwrap();
    let parsed = parse_sntp_output(&pm).unwrap();
    assert_eq!(parsed.off_us, 1234);
    assert_eq!(parsed.err_us, 5678);
    let od = fs::read_to_string(format!("{FIXTURES}sntp-offset-delay.txt")).unwrap();
    let parsed = parse_sntp_output(&od).unwrap();
    assert_eq!(parsed.off_us, 2000);
    assert_eq!(parsed.err_us, 6000);
    let unsync = fs::read_to_string(format!("{FIXTURES}sntp-unsync.txt")).unwrap();
    let parsed = parse_sntp_output(&unsync).unwrap();
    assert!(!parsed.synchronized);
    assert_eq!(parsed.err_us, CLOCK_UNKNOWN_ERR_US);
}
