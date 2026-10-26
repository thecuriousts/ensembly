//! Application ledger (Stage 1) — Ensembly is SoT for job applications.
//! Append-only JSONL; later row for the same `link` supersedes earlier ones.
use serde::{Deserialize, Serialize};
use std::path::Path;

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
    if !r.link.starts_with("https://") || r.link.len() <= "https://".len() {
        anyhow::bail!("link must be an https URL");
    }
    match (&r.submit_time, r.status) {
        (None, Status::Prepared) => {}
        (None, s) => anyhow::bail!("submit_time is required when status is {}", s.as_str()),
        (Some(t), _) => check_submit_time(t)?,
    }
    Ok(())
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
        latest.insert(r.link.as_str(), r);
    }
    let mut v: Vec<Row> = latest.into_values().cloned().collect();
    v.sort_by(|a, b| {
        let key = |r: &Row| {
            (
                r.submit_time.is_none(),
                r.submit_time.clone(),
                r.link.clone(),
            )
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
    let mut s = String::from("company,role,link,status,submit_time,cv_version\n");
    for r in current_rows(rows) {
        let f = [
            r.company.as_str(),
            r.role.as_str(),
            r.link.as_str(),
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
        "| Company | Role | Status | Submitted | CV version | Link |\n|---|---|---|---|---|---|\n",
    );
    for r in current_rows(rows) {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            md_cell(&r.company),
            md_cell(&r.role),
            r.status.as_str(),
            md_cell(r.submit_time.as_deref().unwrap_or("—")),
            md_cell(&r.cv_version),
            md_cell(&r.link),
        ));
    }
    s
}

/// Append one validated row. An exact duplicate of an existing row is refused
/// (re-running the same append is a no-op error, never a double row).
pub fn append_row(path: &Path, r: &Row) -> anyhow::Result<()> {
    use std::io::Write;
    validate_row(r)?;
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
    fn csv_export_has_header_and_rows_and_escapes() {
        let rows = parse_ledger(SAMPLE).unwrap();
        let csv = export_csv(&rows);
        assert!(csv.starts_with("company,role,link,status,submit_time,cv_version\n"));
        assert!(csv.contains("\"Engineer, Widgets\""), "{csv}");
        assert!(csv.lines().count() > 1, "header-only export must fail");
    }

    #[test]
    fn markdown_export_has_table_rows_and_escapes_pipes() {
        let md = export_markdown(&parse_ledger(SAMPLE).unwrap());
        assert!(md.contains("| Company | Role | Status | Submitted | CV version | Link |"));
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
