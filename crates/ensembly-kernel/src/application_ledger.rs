//! Application ledger (Stage 1) — Ensembly is SoT for job applications.
//! Append-only JSONL; later row for the same `link` supersedes earlier ones.
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Real rows live here: under `data/`, which `.gitignore` keeps local-only.
pub const DEFAULT_LEDGER_PATH: &str = "data/local/applications/ledger.jsonl";
/// Explicit marker for rows migrated from records that never noted which CV was sent.
pub const LEGACY_UNKNOWN_CV: &str = "legacy-unknown";
pub const DEFAULT_EXPORT_DIR: &str = "data/local/applications/exports";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Prepared,
    Submitted,
    Interview,
    Rejected,
    Withdrawn,
    SilentClosed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub company: String,
    pub role: String,
    pub link: String,
    /// Optional application-form URL (e.g. `…/application`); `link` stays the canonical posting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_url: Option<String>,
    pub status: Status,
    pub submit_time: Option<String>,
    pub cv_version: String,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Prepared => "prepared",
            Status::Submitted => "submitted",
            Status::Interview => "interview",
            Status::Rejected => "rejected",
            Status::Withdrawn => "withdrawn",
            Status::SilentClosed => "silent-closed",
        }
    }
}

/// Europe/Stockholm is +01:00 (CET) or +02:00 (CEST). `Z` and other offsets are refused
/// so every row reads in the operator's zone without conversion.
fn check_submit_time(t: &str) -> anyhow::Result<()> {
    chrono::DateTime::parse_from_rfc3339(t)
        .map_err(|e| anyhow::anyhow!("submit_time {t:?} is not RFC 3339 with offset: {e}"))?;
    if !(t.ends_with("+01:00") || t.ends_with("+02:00")) {
        anyhow::bail!(
            "submit_time {t:?} must carry the Europe/Stockholm offset (+01:00 or +02:00)"
        );
    }
    Ok(())
}

pub fn validate_row(r: &Row) -> anyhow::Result<()> {
    for (name, v) in [
        ("company", &r.company),
        ("role", &r.role),
        ("cv_version", &r.cv_version),
    ] {
        if v.trim().is_empty() {
            anyhow::bail!("{name} must not be blank");
        }
    }
    const AD_HOC_UNKNOWN: [&str; 8] = ["unknown", "n/a", "na", "none", "?", "-", "legacy", "tbd"];
    if AD_HOC_UNKNOWN.contains(&r.cv_version.trim().to_lowercase().as_str()) {
        anyhow::bail!("cv_version {:?}: use the explicit marker \"{LEGACY_UNKNOWN_CV}\" when the CV is not recorded", r.cv_version);
    }
    if !r.link.starts_with("https://") || r.link.len() <= "https://".len() {
        anyhow::bail!("link must be an https URL");
    }
    if let Some(u) = &r.apply_url {
        if !u.starts_with("https://") || u.len() <= "https://".len() {
            anyhow::bail!("apply_url must be an https URL");
        }
    }
    match (&r.submit_time, r.status) {
        (None, Status::Prepared | Status::Withdrawn) => {}
        (None, s) => anyhow::bail!("submit_time is required when status is {}", s.as_str()),
        (Some(t), _) => check_submit_time(t)?,
    }
    Ok(())
}

/// Identity key of an application: the posting URL without a trailing `/application`
/// (ATS form suffix) or `/`. Lets a corrected row with the canonical `link`
/// supersede an older row that stored the form URL, without editing history.
pub fn link_key(link: &str) -> &str {
    let l = link.trim_end_matches('/');
    l.strip_suffix("/application").unwrap_or(l)
}

/// Parse JSONL (blank lines ignored). Errors name the 1-based line.
pub fn parse_ledger(text: &str) -> anyhow::Result<Vec<Row>> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let r: Row =
            serde_json::from_str(line).map_err(|e| anyhow::anyhow!("line {}: {e}", i + 1))?;
        validate_row(&r).map_err(|e| anyhow::anyhow!("line {}: {e}", i + 1))?;
        out.push(r);
    }
    Ok(out)
}

/// Current state: last row per `link` wins; sorted by (submit_time, link), prepared (no time) last.
pub fn current_rows(rows: &[Row]) -> Vec<Row> {
    let mut latest: std::collections::BTreeMap<&str, &Row> = std::collections::BTreeMap::new();
    for r in rows {
        latest.insert(link_key(&r.link), r);
    }
    let mut v: Vec<Row> = latest.into_values().cloned().collect();
    v.sort_by(|a, b| {
        let key = |r: &Row| {
            let instant = r
                .submit_time
                .as_deref()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .map(|d| d.timestamp());
            (instant.is_none(), instant, r.link.clone())
        };
        key(a).cmp(&key(b))
    });
    v
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn export_csv(rows: &[Row]) -> String {
    let mut s = String::from("company,role,link,apply_url,status,submit_time,cv_version\n");
    for r in current_rows(rows) {
        let f = [
            r.company.as_str(),
            r.role.as_str(),
            r.link.as_str(),
            r.apply_url.as_deref().unwrap_or(""),
            r.status.as_str(),
            r.submit_time.as_deref().unwrap_or(""),
            r.cv_version.as_str(),
        ];
        s.push_str(&f.iter().map(|x| csv_field(x)).collect::<Vec<_>>().join(","));
        s.push('\n');
    }
    s
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}

pub fn export_markdown(rows: &[Row]) -> String {
    let mut s = String::from(
        "| Company | Role | Status | Submitted | CV version | Link | Apply |\n|---|---|---|---|---|---|---|\n",
    );
    for r in current_rows(rows) {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            md_cell(&r.company),
            md_cell(&r.role),
            r.status.as_str(),
            md_cell(r.submit_time.as_deref().unwrap_or("—")),
            md_cell(&r.cv_version),
            md_cell(&r.link),
            md_cell(r.apply_url.as_deref().unwrap_or("—")),
        ));
    }
    s
}

/// Append one validated row. An exact duplicate of an existing row is refused
/// (re-running the same append is a no-op error, never a double row).
pub fn append_row(path: &Path, r: &Row) -> anyhow::Result<()> {
    use std::io::Write;
    validate_row(r)?;
    if link_key(&r.link) != r.link.trim_end_matches('/') {
        anyhow::bail!("link must be the canonical posting URL; put the form URL in apply_url");
    }
    let existing = match std::fs::read_to_string(path) {
        Ok(t) => parse_ledger(&t)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    if existing.iter().any(|x| x == r) {
        anyhow::bail!("identical row already in ledger for {}", r.link);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{}", serde_json::to_string(r)?)?;
    Ok(())
}

/// Write `applications.csv` + `applications.md` into `dir` (byte-identical on re-run).
pub fn write_exports(rows: &[Row], dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("applications.csv"), export_csv(rows))?;
    std::fs::write(dir.join("applications.md"), export_markdown(rows))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../../../fixtures/application-ledger-sample.jsonl");

    fn row(link: &str, status: &str, t: Option<&str>) -> String {
        let t = t
            .map(|s| format!("\"{s}\""))
            .unwrap_or_else(|| "null".into());
        format!(
            r#"{{"company":"Acme Fictional","role":"Engineer, Widgets","link":"{link}","status":"{status}","submit_time":{t},"cv_version":"cv-sample-1"}}"#
        )
    }

    #[test]
    fn sample_fixture_parses_to_nonempty_rows() {
        let rows = parse_ledger(SAMPLE).expect("sample valid");
        assert!(rows.len() >= 3, "empty parse must fail this test");
        assert_eq!(rows[0].company, "Northwind Fictional AB");
    }

    #[test]
    fn rejects_unknown_status() {
        let err = parse_ledger(&row(
            "https://example.test/j/1",
            "ghosted",
            Some("2026-10-26T18:59:00+01:00"),
        ))
        .unwrap_err();
        assert!(err.to_string().contains("line 1"), "{err}");
    }

    #[test]
    fn rejects_non_stockholm_offset_and_missing_offset() {
        for t in [
            "2026-10-26T18:59:00Z",
            "2026-10-26T18:59:00+05:30",
            "2026-10-26T18:59:00",
        ] {
            assert!(
                parse_ledger(&row("https://example.test/j/1", "submitted", Some(t))).is_err(),
                "{t}"
            );
        }
        assert!(parse_ledger(&row(
            "https://example.test/j/1",
            "submitted",
            Some("2026-07-01T10:00:00+02:00")
        ))
        .is_ok());
    }

    #[test]
    fn submit_time_required_unless_prepared() {
        assert!(parse_ledger(&row("https://example.test/j/1", "submitted", None)).is_err());
        assert!(parse_ledger(&row("https://example.test/j/1", "prepared", None)).is_ok());
    }

    #[test]
    fn rejects_unknown_fields_and_non_https_link() {
        let extra = r#"{"company":"A","role":"R","link":"https://x.test/1","status":"prepared","submit_time":null,"cv_version":"v","phone":"000"}"#;
        assert!(parse_ledger(extra).is_err());
        assert!(parse_ledger(&row("http://example.test/j/1", "prepared", None)).is_err());
        let blank = r#"{"company":"A","role":" ","link":"https://x.test/1","status":"prepared","submit_time":null,"cv_version":"v"}"#;
        assert!(parse_ledger(blank).is_err());
    }

    #[test]
    fn later_row_for_same_link_supersedes() {
        let text = format!(
            "{}\n{}\n",
            row(
                "https://example.test/j/1",
                "submitted",
                Some("2026-10-26T18:59:00+01:00")
            ),
            row(
                "https://example.test/j/1",
                "rejected",
                Some("2026-10-26T18:59:00+01:00")
            )
        );
        let cur = current_rows(&parse_ledger(&text).unwrap());
        assert_eq!(cur.len(), 1);
        assert_eq!(cur[0].status, Status::Rejected);
    }

    #[test]
    fn current_rows_sort_by_instant_not_string() {
        // 01:00+02:00 (23:00Z) is earlier than 00:30+01:00 (23:30Z) though it sorts later as a string
        let text = format!(
            "{}\n{}\n",
            row(
                "https://example.test/j/a",
                "submitted",
                Some("2026-10-25T00:30:00+01:00")
            ),
            row(
                "https://example.test/j/b",
                "submitted",
                Some("2026-10-25T01:00:00+02:00")
            )
        );
        let cur = current_rows(&parse_ledger(&text).unwrap());
        assert_eq!(cur[0].link, "https://example.test/j/b");
    }

    #[test]
    fn apply_url_is_optional_and_must_be_https() {
        let with = r#"{"company":"A","role":"R","link":"https://x.test/j/1","apply_url":"https://x.test/j/1/application","status":"prepared","submit_time":null,"cv_version":"v"}"#;
        let rows = parse_ledger(with).unwrap();
        assert_eq!(
            rows[0].apply_url.as_deref(),
            Some("https://x.test/j/1/application")
        );
        let bad = with.replace("\"apply_url\":\"https://", "\"apply_url\":\"http://");
        assert!(parse_ledger(&bad).is_err());
        // absent apply_url is not serialised, so pre-apply_url rows round-trip byte-for-byte
        let line = row("https://example.test/j/1", "prepared", None);
        let r = &parse_ledger(&line).unwrap()[0];
        assert_eq!(serde_json::to_string(r).unwrap(), line);
    }

    #[test]
    fn application_suffix_row_is_superseded_by_canonical_link_row() {
        let text = format!(
            "{}\n{}\n",
            row(
                "https://jobs.example.test/acme/1/application",
                "submitted",
                Some("2026-10-26T18:59:00+01:00")
            ),
            r#"{"company":"Acme Fictional","role":"Engineer, Widgets","link":"https://jobs.example.test/acme/1","apply_url":"https://jobs.example.test/acme/1/application","status":"submitted","submit_time":"2026-10-26T18:59:00+01:00","cv_version":"cv-sample-1"}"#
        );
        let cur = current_rows(&parse_ledger(&text).unwrap());
        assert_eq!(cur.len(), 1, "{cur:?}");
        assert_eq!(cur[0].link, "https://jobs.example.test/acme/1");
        assert!(cur[0].apply_url.is_some());
    }

    #[test]
    fn append_refuses_application_suffix_in_link() {
        let dir = tempfile::tempdir().unwrap();
        let r: Row = serde_json::from_str(&row(
            "https://example.test/j/9/application",
            "submitted",
            Some("2026-10-26T20:08:00+01:00"),
        ))
        .unwrap();
        let err = append_row(&dir.path().join("l.jsonl"), &r).unwrap_err();
        assert!(err.to_string().contains("apply_url"), "{err}");
    }

    #[test]
    fn exports_carry_apply_url_column() {
        let rows = parse_ledger(SAMPLE).unwrap();
        let csv = export_csv(&rows);
        assert!(
            csv.starts_with("company,role,link,apply_url,status,submit_time,cv_version\n"),
            "{csv}"
        );
        assert!(csv.contains("https://jobs.example.test/northwind/1001/apply"));
        let md = export_markdown(&rows);
        assert!(
            md.contains("| Company | Role | Status | Submitted | CV version | Link | Apply |"),
            "{md}"
        );
    }

    #[test]
    fn legacy_unknown_is_the_only_unknown_cv_marker() {
        let mk = |cv: &str| {
            format!(
                r#"{{"company":"A","role":"R","link":"https://x.test/1","status":"rejected","submit_time":"2026-09-04T17:12:00+02:00","cv_version":"{cv}"}}"#
            )
        };
        assert!(parse_ledger(&mk(LEGACY_UNKNOWN_CV)).is_ok());
        assert_eq!(LEGACY_UNKNOWN_CV, "legacy-unknown");
        for ad_hoc in [
            "unknown", "Unknown", "n/a", "none", "?", "-", "legacy", "tbd",
        ] {
            let err = parse_ledger(&mk(ad_hoc)).unwrap_err();
            assert!(
                err.to_string().contains("legacy-unknown"),
                "{ad_hoc}: {err}"
            );
        }
    }

    #[test]
    fn withdrawn_may_omit_submit_time_but_sent_statuses_may_not() {
        assert!(parse_ledger(&row("https://example.test/j/1", "withdrawn", None)).is_ok());
        assert!(parse_ledger(&row(
            "https://example.test/j/1",
            "withdrawn",
            Some("2026-09-06T16:55:00+02:00")
        ))
        .is_ok());
        for st in ["submitted", "interview", "rejected", "silent-closed"] {
            assert!(
                parse_ledger(&row("https://example.test/j/1", st, None)).is_err(),
                "{st}"
            );
        }
        let text = format!(
            "{}\n{}\n",
            row("https://example.test/j/w", "withdrawn", None),
            row(
                "https://example.test/j/s",
                "submitted",
                Some("2026-09-06T16:55:00+02:00")
            )
        );
        let rows = parse_ledger(&text).unwrap();
        let csv = export_csv(&rows);
        assert!(
            csv.lines()
                .last()
                .unwrap()
                .contains(",withdrawn,,cv-sample-1"),
            "{csv}"
        );
        assert_eq!(current_rows(&rows)[1].link, "https://example.test/j/w");
    }

    #[test]
    fn csv_export_has_header_and_rows_and_escapes() {
        let rows = parse_ledger(SAMPLE).unwrap();
        let csv = export_csv(&rows);
        assert!(csv.contains("\"Engineer, Widgets\""), "{csv}");
        assert!(csv.lines().count() > 1, "header-only export must fail");
    }

    #[test]
    fn markdown_export_has_table_rows_and_escapes_pipes() {
        let md = export_markdown(&parse_ledger(SAMPLE).unwrap());
        assert!(md.contains("Platform \\| Infra"), "{md}");
        assert!(md.lines().filter(|l| l.starts_with("| ")).count() >= 4);
    }

    #[test]
    fn exports_are_deterministic_regardless_of_input_order() {
        let rows = parse_ledger(SAMPLE).unwrap();
        let mut rev = rows.clone();
        rev.reverse();
        // reversing changes supersede order only for duplicate links; sample has none
        assert_eq!(export_csv(&rows), export_csv(&rows));
        assert_eq!(export_csv(&rows), export_csv(&rev));
        assert_eq!(export_markdown(&rows), export_markdown(&rev));
    }

    #[test]
    fn append_validates_and_writes_one_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("ledger.jsonl");
        let r: Row = serde_json::from_str(&row(
            "https://example.test/j/9",
            "submitted",
            Some("2026-10-26T20:08:00+01:00"),
        ))
        .unwrap();
        append_row(&p, &r).unwrap();
        append_row(&p, &r).unwrap_err(); // exact duplicate refused (idempotent append)
        let text = std::fs::read_to_string(&p).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert_eq!(parse_ledger(&text).unwrap()[0], r);
        let mut bad = r.clone();
        bad.submit_time = None;
        assert!(append_row(&p, &bad).is_err());
    }

    #[test]
    fn write_exports_twice_is_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let rows = parse_ledger(SAMPLE).unwrap();
        write_exports(&rows, dir.path()).unwrap();
        let a = (
            std::fs::read(dir.path().join("applications.csv")).unwrap(),
            std::fs::read(dir.path().join("applications.md")).unwrap(),
        );
        write_exports(&rows, dir.path()).unwrap();
        let b = (
            std::fs::read(dir.path().join("applications.csv")).unwrap(),
            std::fs::read(dir.path().join("applications.md")).unwrap(),
        );
        assert_eq!(a, b);
        assert!(!a.0.is_empty() && !a.1.is_empty());
    }
}
