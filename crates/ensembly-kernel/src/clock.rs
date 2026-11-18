//! Clock-offset sampler for optional `clock` metadata on future record events.
//!
//! Microsecond fields are **resolution**, not accuracy. `err_us` is an honest upper bound
//! from the source (chrony, sntp, or peer), not measured precision.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Upper bound when the offset is unknown (`src: "none"`). Not “zero error”.
pub const CLOCK_UNKNOWN_ERR_US: u64 = u64::MAX;

pub const DEFAULT_CLOCK_MAX_AGE_S: u32 = 60;
pub const DEFAULT_SNTP_SERVER: &str = "time.apple.com";
pub const DEFAULT_CLOCK_CACHE_NAME: &str = "clock/sample.json";
pub const DEFAULT_CLOCK_SAMPLE_TIMEOUT: Duration = Duration::from_secs(2);

const PEER_ALIASES: [&str; 4] = ["laptop-1", "laptop-2", "mac-mini", "box"];

/// Payload shape future events may attach.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockSample {
    pub off_us: i64,
    pub err_us: u64,
    pub src: String,
    pub age_s: u32,
}

impl ClockSample {
    pub fn none(age_s: u32) -> Self {
        Self {
            off_us: 0,
            err_us: CLOCK_UNKNOWN_ERR_US,
            src: "none".into(),
            age_s,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ClockCacheRecord {
    sampled_at: String,
    off_us: i64,
    err_us: u64,
    src: String,
}

#[derive(Clone)]
pub struct SampleOpts<'a> {
    pub root: &'a Path,
    pub config_data_dir: Option<&'a Path>,
    pub max_age_s: u32,
    pub timeout: Duration,
    pub peer: Option<&'a str>,
    pub sntp_server: &'a str,
    pub transport: Option<&'a dyn PeerClockTransport>,
    pub force_refresh: bool,
    /// When false, skip chronyc/sntp subprocesses (unit tests).
    pub platform_probe: bool,
}

impl Default for SampleOpts<'_> {
    fn default() -> Self {
        Self {
            root: Path::new(""),
            config_data_dir: None,
            max_age_s: DEFAULT_CLOCK_MAX_AGE_S,
            timeout: DEFAULT_CLOCK_SAMPLE_TIMEOUT,
            peer: None,
            sntp_server: DEFAULT_SNTP_SERVER,
            transport: None,
            force_refresh: false,
            platform_probe: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ClockStatusReport {
    pub cache_path: String,
    pub cache_present: bool,
    pub sample: Option<ClockSample>,
    pub chronyc: &'static str,
    pub sntp: &'static str,
    pub platform: String,
}

/// Round-trip timestamps for the NTP offset formula (all microseconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerExchange {
    pub t1_us: i64,
    pub t2_us: i64,
    pub t3_us: i64,
    pub t4_us: i64,
    pub peer_off_us: i64,
    pub peer_err_us: u64,
}

pub trait PeerClockTransport: Send + Sync {
    fn round_trip(&self) -> Result<PeerExchange, PeerTransportError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerTransportError(pub &'static str);

/// CLI / default until a later slice wires SSH.
#[derive(Debug, Default)]
pub struct UnwiredPeerTransport;

impl PeerClockTransport for UnwiredPeerTransport {
    fn round_trip(&self) -> Result<PeerExchange, PeerTransportError> {
        Err(PeerTransportError("peer transport not wired"))
    }
}

pub fn resolve_clock_cache_path(root: &Path, config_data: Option<&Path>) -> PathBuf {
    if let Some(dir) = config_data.filter(|p| p.is_dir()) {
        return dir.join(DEFAULT_CLOCK_CACHE_NAME);
    }
    root.join("data/local").join(DEFAULT_CLOCK_CACHE_NAME)
}

/// Returns the latest sample, refreshing the on-disk cache when stale (see `SampleOpts`).
pub fn sample_latest(opts: &SampleOpts<'_>) -> io::Result<ClockSample> {
    let cache_path = resolve_clock_cache_path(opts.root, opts.config_data_dir);
    let cached = read_cache(&cache_path)?;
    let cache_age = cached
        .as_ref()
        .and_then(|c| age_seconds_from_rfc3339(&c.sampled_at));

    if !opts.force_refresh {
        if let Some(c) = &cached {
            if let Some(age) = cache_age {
                if age < opts.max_age_s {
                    return Ok(record_to_sample(c, age));
                }
            }
        }
    }

    let fresh = sample_within_timeout(opts);
    match fresh {
        Ok(record) => {
            write_cache(&cache_path, &record)?;
            Ok(record_to_sample(&record, 0))
        }
        Err(_) => {
            if let Some(c) = cached {
                let age = cache_age.unwrap_or(0);
                Ok(record_to_sample(&c, age))
            } else {
                Ok(ClockSample::none(0))
            }
        }
    }
}

pub fn clock_status(root: &Path, config_data: Option<&Path>) -> io::Result<ClockStatusReport> {
    let cache_path = resolve_clock_cache_path(root, config_data);
    let cached = read_cache(&cache_path)?;
    let sample = cached.as_ref().map(|c| {
        let age = age_seconds_from_rfc3339(&c.sampled_at).unwrap_or(0);
        record_to_sample(c, age)
    });
    Ok(ClockStatusReport {
        cache_path: cache_path.display().to_string(),
        cache_present: cached.is_some(),
        sample,
        chronyc: if chronyc_bin().is_some() {
            "present"
        } else {
            "missing"
        },
        sntp: if sntp_bin().is_some() {
            "present"
        } else {
            "missing"
        },
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    })
}

/// NTP-style offset and error bound from a four-timestamp exchange.
pub fn peer_offset_and_err(ex: &PeerExchange) -> (i64, u64) {
    let delay_us = (ex.t4_us - ex.t1_us) - (ex.t3_us - ex.t2_us);
    let off_us = ((ex.t2_us - ex.t1_us) + (ex.t3_us - ex.t4_us)) / 2;
    let half_delay = (delay_us / 2).max(0) as u64;
    let err_us = half_delay.saturating_add(ex.peer_err_us);
    (off_us, err_us)
}

pub fn chrony_err_us(offset_s: f64, root_delay_s: f64, root_dispersion_s: f64) -> u64 {
    let bound_s = offset_s.abs() + root_dispersion_s + root_delay_s / 2.0;
    secs_to_us_ceil(bound_s)
}

pub fn parse_chronyc_tracking_csv(line: &str) -> Option<(i64, u64)> {
    let fields = parse_csv_line(line.trim());
    if fields.len() < 11 {
        return None;
    }
    let offset_s: f64 = fields[4].parse().ok()?;
    let root_delay_s: f64 = fields[9].parse().ok()?;
    let root_dispersion_s: f64 = fields[10].parse().ok()?;
    let off_us = secs_to_us_round(offset_s);
    let err_us = chrony_err_us(offset_s, root_delay_s, root_dispersion_s);
    Some((off_us, err_us))
}

/// Parse macOS / BSD `sntp` client text. **Fixtures only until verified on real macOS hardware.**
pub fn parse_sntp_output(text: &str) -> Option<(i64, u64)> {
    let normalized = text.replace(',', ".");
    if let Some((off, err)) = parse_sntp_plusminus(&normalized) {
        return Some((off, err));
    }
    parse_sntp_offset_line(&normalized)
}

fn parse_sntp_plusminus(text: &str) -> Option<(i64, u64)> {
    let re = regex::Regex::new(r"(?i)([+-]?\d+(?:\.\d+)?)\s*/?\s*\+/-\s*(\d+(?:\.\d+)?)").ok()?;
    let caps = re.captures(text)?;
    let off_s: f64 = caps.get(1)?.as_str().parse().ok()?;
    let err_s: f64 = caps.get(2)?.as_str().parse().ok()?;
    Some((secs_to_us_round(off_s), secs_to_us_ceil(err_s)))
}

fn parse_sntp_offset_line(text: &str) -> Option<(i64, u64)> {
    let off_re = regex::Regex::new(r"(?i)offset\s+([+-]?\d+(?:\.\d+)?)").ok()?;
    let off_s: f64 = off_re.captures(text)?.get(1)?.as_str().parse().ok()?;
    let err_s = regex::Regex::new(r"(?i)delay\s+(\d+(?:\.\d+)?)")
        .ok()?
        .captures(text)
        .and_then(|c| c.get(1)?.as_str().parse::<f64>().ok())
        .map(|d| d / 2.0)
        .unwrap_or(0.0);
    Some((secs_to_us_round(off_s), secs_to_us_ceil(err_s)))
}

pub fn secs_to_us_ceil(secs: f64) -> u64 {
    if !secs.is_finite() || secs <= 0.0 {
        return 0;
    }
    (secs * 1_000_000.0).ceil() as u64
}

pub fn secs_to_us_round(secs: f64) -> i64 {
    if !secs.is_finite() {
        return 0;
    }
    (secs * 1_000_000.0).round() as i64
}

pub fn validate_ntp_hostname(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

pub fn validate_peer_alias(alias: &str) -> bool {
    PEER_ALIASES.contains(&alias)
}

fn record_to_sample(r: &ClockCacheRecord, age_s: u32) -> ClockSample {
    ClockSample {
        off_us: r.off_us,
        err_us: r.err_us,
        src: r.src.clone(),
        age_s,
    }
}

fn age_seconds_from_rfc3339(stamp: &str) -> Option<u32> {
    let parsed = DateTime::parse_from_rfc3339(stamp).ok()?;
    let age = Utc::now().signed_duration_since(parsed.with_timezone(&Utc));
    if age.num_seconds() < 0 {
        return Some(0);
    }
    Some(age.num_seconds().min(u32::MAX as i64) as u32)
}

fn read_cache(path: &Path) -> io::Result<Option<ClockCacheRecord>> {
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(None);
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_cache(path: &Path, record: &ClockCacheRecord) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(record)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)
}

fn sample_within_timeout(opts: &SampleOpts<'_>) -> Result<ClockCacheRecord, &'static str> {
    if opts.peer.is_some() {
        return sample_now(opts);
    }
    let root = opts.root.to_path_buf();
    let config = opts.config_data_dir.map(|p| p.to_path_buf());
    let sntp_server = opts.sntp_server.to_string();
    let max_age_s = opts.max_age_s;
    let force_refresh = opts.force_refresh;
    let platform_probe = opts.platform_probe;
    let timeout = opts.timeout;
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let inner = SampleOpts {
            root: &root,
            config_data_dir: config.as_deref(),
            max_age_s,
            timeout,
            peer: None,
            sntp_server: &sntp_server,
            transport: None,
            force_refresh,
            platform_probe,
        };
        let _ = tx.send(sample_now(&inner));
    });
    match rx.recv_timeout(timeout) {
        Ok(r) => r,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err("timeout"),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err("timeout"),
    }
}

#[allow(clippy::collapsible_if)]
fn sample_now(opts: &SampleOpts<'_>) -> Result<ClockCacheRecord, &'static str> {
    if let Some(alias) = opts.peer {
        if !validate_peer_alias(alias) {
            return Err("invalid peer alias");
        }
        let transport = opts.transport.ok_or("peer transport missing")?;
        let ex = transport
            .round_trip()
            .map_err(|_| "peer transport failed")?;
        let (off_us, err_us) = peer_offset_and_err(&ex);
        return Ok(new_record(off_us, err_us, format!("peer:{alias}")));
    }
    if opts.platform_probe {
        if let Some((off_us, err_us)) = sample_chrony() {
            return Ok(new_record(off_us, err_us, "chrony".into()));
        }
    }
    if opts.platform_probe && cfg!(target_os = "macos") && validate_ntp_hostname(opts.sntp_server) {
        if let Some((off_us, err_us)) = sample_sntp(opts.sntp_server) {
            return Ok(new_record(off_us, err_us, "sntp".into()));
        }
    }
    Err("no source")
}

fn new_record(off_us: i64, err_us: u64, src: String) -> ClockCacheRecord {
    ClockCacheRecord {
        sampled_at: Utc::now().to_rfc3339(),
        off_us,
        err_us,
        src,
    }
}

fn sample_chrony() -> Option<(i64, u64)> {
    let bin = chronyc_bin()?;
    let mut cmd = Command::new(&bin);
    cmd.args(["-c", "tracking"]);
    let output = output_within(cmd, DEFAULT_CLOCK_SAMPLE_TIMEOUT)?;
    let line = output.lines().next()?;
    parse_chronyc_tracking_csv(line)
}

fn sample_sntp(server: &str) -> Option<(i64, u64)> {
    let bin = sntp_bin()?;
    let mut cmd = Command::new(&bin);
    cmd.args(["-t", "2", server]);
    let output = output_within(cmd, DEFAULT_CLOCK_SAMPLE_TIMEOUT)?;
    parse_sntp_output(&output)
}

fn chronyc_bin() -> Option<PathBuf> {
    resolve_on_path("chronyc")
}

fn sntp_bin() -> Option<PathBuf> {
    resolve_on_path("sntp")
}

fn resolve_on_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn output_within(mut cmd: Command, limit: Duration) -> Option<String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                let out = child.wait_with_output().ok()?;
                let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                if text.trim().is_empty() {
                    text = String::from_utf8_lossy(&out.stderr).into_owned();
                }
                return Some(text);
            }
            Ok(Some(_)) => return None,
            Ok(None) if start.elapsed() < limit => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(cur.clone());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    fields.push(cur);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrony_fixture_parses_offset_and_err() {
        let line = "CB00710F,3,203.190.158.190,12.345678,0.000250,-0.000125,1.2,0.1,0.05,0.020000,0.001500,64,0";
        let (off_us, err_us) = parse_chronyc_tracking_csv(line).expect("parse");
        assert_eq!(off_us, 250);
        // |0.00025| + 0.0015 + 0.01 = 0.01175 s -> 11750 us ceil
        assert_eq!(err_us, 11_750);
    }

    #[test]
    fn chrony_err_arithmetic_ceil() {
        assert_eq!(chrony_err_us(-0.00025, 0.02, 0.0015), 11_750);
    }

    #[test]
    fn sntp_plusminus_fixture() {
        let text = "+0.001234 +/- 0.005678 seconds";
        let (off, err) = parse_sntp_output(text).unwrap();
        assert_eq!(off, 1234);
        assert_eq!(err, 5678);
    }

    #[test]
    fn sntp_offset_delay_fixture() {
        let text = "offset +0.002000 sec, delay 0.012000";
        let (off, err) = parse_sntp_output(text).unwrap();
        assert_eq!(off, 2000);
        assert_eq!(err, 6000);
    }

    #[test]
    fn peer_formula_matches_ntp() {
        let ex = PeerExchange {
            t1_us: 1_000_000,
            t2_us: 1_002_000,
            t3_us: 1_003_000,
            t4_us: 1_005_000,
            peer_off_us: 0,
            peer_err_us: 100,
        };
        let (off, err) = peer_offset_and_err(&ex);
        assert_eq!(off, 0);
        // delay = (5-1) - (3-2) = 3 ms; half = 2000 + 100
        assert_eq!(err, 2100);
    }

    #[test]
    fn cache_staleness_returns_old_sample_when_refresh_misses() {
        let dir = tempfile::tempdir().unwrap();
        let cache = resolve_clock_cache_path(dir.path(), None);
        let old = ClockCacheRecord {
            sampled_at: "2000-01-01T00:00:00Z".into(),
            off_us: 7,
            err_us: 11,
            src: "chrony".into(),
        };
        write_cache(&cache, &old).unwrap();
        let opts = SampleOpts {
            root: dir.path(),
            config_data_dir: None,
            max_age_s: 60,
            timeout: Duration::from_millis(50),
            peer: None,
            sntp_server: DEFAULT_SNTP_SERVER,
            transport: None,
            force_refresh: false,
            platform_probe: false,
        };
        let sample = sample_latest(&opts).unwrap();
        assert_eq!(sample.off_us, 7);
        assert_eq!(sample.src, "chrony");
        assert!(sample.age_s > 1000);
    }

    struct FakeTransport {
        ex: PeerExchange,
    }

    impl PeerClockTransport for FakeTransport {
        fn round_trip(&self) -> Result<PeerExchange, PeerTransportError> {
            Ok(self.ex)
        }
    }

    #[test]
    fn peer_sample_via_fake_transport() {
        let dir = tempfile::tempdir().unwrap();
        let transport = FakeTransport {
            ex: PeerExchange {
                t1_us: 0,
                t2_us: 50,
                t3_us: 60,
                t4_us: 100,
                peer_off_us: 0,
                peer_err_us: 5,
            },
        };
        let opts = SampleOpts {
            root: dir.path(),
            config_data_dir: None,
            max_age_s: 0,
            timeout: Duration::from_secs(1),
            peer: Some("laptop-1"),
            sntp_server: DEFAULT_SNTP_SERVER,
            transport: Some(&transport),
            force_refresh: true,
            platform_probe: true,
        };
        let sample = sample_latest(&opts).unwrap();
        assert_eq!(sample.src, "peer:laptop-1");
        assert!(sample.err_us >= 5);
    }

    #[test]
    fn timeout_returns_last_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = resolve_clock_cache_path(dir.path(), None);
        let record = ClockCacheRecord {
            sampled_at: Utc::now().to_rfc3339(),
            off_us: 42,
            err_us: 99,
            src: "chrony".into(),
        };
        write_cache(&cache_path, &record).unwrap();
        let opts = SampleOpts {
            root: dir.path(),
            config_data_dir: None,
            max_age_s: 0,
            timeout: Duration::from_millis(50),
            peer: None,
            sntp_server: DEFAULT_SNTP_SERVER,
            transport: None,
            force_refresh: true,
            platform_probe: false,
        };
        let sample = sample_latest(&opts).unwrap();
        assert_eq!(sample.off_us, 42);
        assert_eq!(sample.src, "chrony");
    }

    #[test]
    fn none_semantics() {
        let s = ClockSample::none(3);
        assert_eq!(s.off_us, 0);
        assert_eq!(s.err_us, CLOCK_UNKNOWN_ERR_US);
        assert_eq!(s.src, "none");
        assert_eq!(s.age_s, 3);
    }
}
