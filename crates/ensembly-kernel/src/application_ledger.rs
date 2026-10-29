//! Application ledger (Stage 1) — Ensembly is SoT for job applications.
//! Append-only JSONL; later row for the same `link` supersedes earlier ones.
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Real rows live here: under `data/`, which `.gitignore` keeps local-only.
pub const DEFAULT_LEDGER_PATH: &str = "data/local/applications/ledger.jsonl";
/// Explicit marker for rows migrated from records that never noted which CV was sent.
pub const LEGACY_UNKNOWN_CV: &str = "legacy-unknown";
pub const DEFAULT_EXPORT_DIR: &str = "data/local/applications/exports";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// `true` when the application was sent but the time was never recorded
    /// (then `submit_time` is null). Absent otherwise; never `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit_time_unknown: Option<bool>,
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
pub(crate) fn check_submit_time(t: &str) -> anyhow::Result<()> {
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
    match (&r.submit_time, r.submit_time_unknown, r.status) {
        (_, Some(false), _) => {
            anyhow::bail!("submit_time_unknown is either true or absent")
        }
        (Some(_), Some(true), _) => {
            anyhow::bail!("submit_time_unknown: true needs submit_time null")
        }
        (None, Some(true), Status::Prepared | Status::Withdrawn) => anyhow::bail!(
            "submit_time_unknown is only for sent statuses; {} already allows a null submit_time",
            r.status.as_str()
        ),
        (None, Some(true), _) => {}
        (None, None, Status::Prepared | Status::Withdrawn) => {}
        (None, None, s) => anyhow::bail!(
            "submit_time is required when status is {} (if it was never recorded, set submit_time_unknown: true)",
            s.as_str()
        ),
        (Some(t), None, _) => check_submit_time(t)?,
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

/// Export text for the submit time: the time, "unknown" when flagged, else `none`.
fn time_cell<'a>(r: &'a Row, none: &'a str) -> &'a str {
    match (&r.submit_time, r.submit_time_unknown) {
        (Some(t), _) => t,
        (None, Some(true)) => "unknown",
        _ => none,
    }
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
            time_cell(&r, ""),
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
            md_cell(time_cell(&r, "—")),
            md_cell(&r.cv_version),
            md_cell(&r.link),
            md_cell(r.apply_url.as_deref().unwrap_or("—")),
        ));
    }
    s
}

/// Row with surrounding whitespace trimmed and the link reduced to its identity
/// key, so cosmetic variants of the same state compare equal.
fn same_state_key(r: &Row) -> Row {
    let t = |s: &str| s.trim().to_string();
    Row {
        company: t(&r.company),
        role: t(&r.role),
        link: link_key(r.link.trim()).to_string(),
        apply_url: r
            .apply_url
            .as_deref()
            .map(|u| t(u).trim_end_matches('/').to_string()),
        status: r.status,
        submit_time: r.submit_time.as_deref().map(t),
        submit_time_unknown: r.submit_time_unknown,
        cv_version: t(&r.cv_version),
    }
}

/// Open a JSONL store for appending. If the file is non-empty and its last
/// byte is not a newline (for example after a hand edit), write one first so
/// the next row starts on its own line.
pub(crate) fn open_jsonl_append(path: &Path) -> anyhow::Result<std::fs::File> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)?;
    if f.metadata()?.len() > 0 {
        f.seek(SeekFrom::End(-1))?;
        let mut last = [0u8; 1];
        f.read_exact(&mut last)?;
        if last[0] != b'\n' {
            f.write_all(b"\n")?;
        }
    }
    Ok(f)
}

/// Append one validated row. A row equal to an existing one (ignoring surrounding
/// whitespace and a trailing slash or `/application` on the link) is refused with an
/// error (CLI exit 1), never written twice. No file lock: one writer at a time.
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
    let key = same_state_key(r);
    if existing.iter().any(|x| same_state_key(x) == key) {
        anyhow::bail!("identical row already in ledger for {}", r.link);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = open_jsonl_append(path)?;
    writeln!(f, "{}", serde_json::to_string(r)?)?;
    Ok(())
}

/// Replace `path` atomically: write a temp file in the same directory, flush
/// it to disk, then rename it over `path`. A reader sees the old or the new
/// file, never a half-written one; on error the temp file is removed.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("no file name in {}", path.display()))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = dir.join(format!(
        ".{}.tmp-{}-{nanos}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| -> anyhow::Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Write `applications.csv` + `applications.md` into `dir` (byte-identical on re-run).
pub fn write_exports(rows: &[Row], dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    write_atomic(&dir.join("applications.csv"), export_csv(rows).as_bytes())?;
    write_atomic(
        &dir.join("applications.md"),
        export_markdown(rows).as_bytes(),
    )?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
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

    fn unknown_time_row(status: &str, time: &str, flag: &str) -> String {
        format!(
            r#"{{"company":"Acme Fictional","role":"Engineer, Widgets","link":"https://example.test/j/u","status":"{status}","submit_time":{time},"submit_time_unknown":{flag},"cv_version":"legacy-unknown"}}"#
        )
    }

    #[test]
    fn sent_status_with_unknown_time_is_representable_and_exported_as_unknown() {
        for st in ["submitted", "interview", "rejected", "silent-closed"] {
            let rows = parse_ledger(&unknown_time_row(st, "null", "true"))
                .unwrap_or_else(|e| panic!("{st}: {e}"));
            assert_eq!(rows[0].submit_time_unknown, Some(true));
            assert!(
                export_csv(&rows).contains(&format!(",{st},unknown,legacy-unknown")),
                "{st}"
            );
            assert!(
                export_markdown(&rows).contains(&format!("| {st} | unknown |")),
                "{st}"
            );
        }
    }

    #[test]
    fn submit_time_unknown_is_strict() {
        // flag with a time, flag on statuses that already allow null, and explicit false are refused
        assert!(parse_ledger(&unknown_time_row(
            "rejected",
            "\"2026-09-04T17:12:00+02:00\"",
            "true"
        ))
        .is_err());
        assert!(parse_ledger(&unknown_time_row("prepared", "null", "true")).is_err());
        assert!(parse_ledger(&unknown_time_row("withdrawn", "null", "true")).is_err());
        assert!(parse_ledger(&unknown_time_row("rejected", "null", "false")).is_err());
        // plain null on a sent status still refused, and the error names the honest option
        let err = parse_ledger(&row("https://example.test/j/1", "rejected", None)).unwrap_err();
        assert!(err.to_string().contains("submit_time_unknown"), "{err}");
    }

    #[test]
    fn a_made_up_time_is_never_required() {
        for st in [
            "prepared",
            "submitted",
            "interview",
            "rejected",
            "withdrawn",
            "silent-closed",
        ] {
            let plain = row("https://example.test/j/1", st, None);
            let flagged = unknown_time_row(st, "null", "true");
            assert!(
                parse_ledger(&plain).is_ok() || parse_ledger(&flagged).is_ok(),
                "{st} needs a time"
            );
        }
    }

    #[test]
    fn append_refuses_whitespace_and_trailing_slash_variants_of_the_same_row() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("l.jsonl");
        let r: Row = serde_json::from_str(&row(
            "https://example.test/j/9",
            "submitted",
            Some("2026-10-26T20:08:00+01:00"),
        ))
        .unwrap();
        append_row(&p, &r).unwrap();
        let mut v = r.clone();
        v.link.push('/');
        v.role = format!(" {} ", v.role);
        assert!(
            append_row(&p, &v).is_err(),
            "variant appended as a new state"
        );
        assert_eq!(std::fs::read_to_string(&p).unwrap().lines().count(), 1);
    }

    const SCHEMA: &str = include_str!("../../../schemas/application-ledger-row.schema.json");

    fn str_array(v: &serde_json::Value) -> Vec<&str> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect()
    }

    /// Every field Rust can write is in the schema, and nothing else.
    fn assert_schema_keys_match_rust(props: &serde_json::Map<String, serde_json::Value>) {
        let full = r#"{"company":"A","role":"R","link":"https://x.test/1","apply_url":"https://x.test/1/application","status":"rejected","submit_time":null,"submit_time_unknown":true,"cv_version":"v"}"#;
        let full: serde_json::Value =
            serde_json::to_value(&parse_ledger(full).unwrap()[0]).unwrap();
        let mut rust_keys: Vec<_> = full.as_object().unwrap().keys().cloned().collect();
        let mut schema_keys: Vec<_> = props.keys().cloned().collect();
        rust_keys.sort();
        schema_keys.sort();
        assert_eq!(rust_keys, schema_keys);
    }

    /// Status enum == Rust Status.
    fn assert_status_enum_matches_rust(props: &serde_json::Map<String, serde_json::Value>) {
        let all = [
            Status::Prepared,
            Status::Submitted,
            Status::Interview,
            Status::Rejected,
            Status::Withdrawn,
            Status::SilentClosed,
        ];
        assert_eq!(
            str_array(&props["status"]["enum"]),
            all.iter().map(|s| s.as_str()).collect::<Vec<_>>()
        );
    }

    /// The submit_time pattern agrees with the Rust time check.
    fn assert_time_pattern_agrees(re: &regex::Regex) {
        for (t, ok) in [
            ("2026-10-20T09:15:00+02:00", true),
            ("2026-10-20T09:15+02:00", false),
            ("2026-10-20T09:15:00Z", false),
            ("2026-10-20T09:15:00+05:30", false),
        ] {
            assert_eq!(re.is_match(t), ok, "schema {t}");
            assert_eq!(check_submit_time(t).is_ok(), ok, "rust {t}");
        }
    }

    /// One sample line satisfies required + known keys + patterns.
    fn assert_line_fits_schema(line: &str, schema: &serde_json::Value, time_re: &regex::Regex) {
        let props = schema["properties"].as_object().unwrap();
        let link_re = regex::Regex::new(props["link"]["pattern"].as_str().unwrap()).unwrap();
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let o = v.as_object().unwrap();
        for k in str_array(&schema["required"]) {
            assert!(o.contains_key(k), "{k} missing: {line}");
        }
        assert!(
            o.keys().all(|k| props.contains_key(k)),
            "key not in schema: {line}"
        );
        assert!(link_re.is_match(o["link"].as_str().unwrap()));
        if let Some(t) = o["submit_time"].as_str() {
            assert!(time_re.is_match(t), "{t}");
        }
    }

    #[test]
    fn schema_matches_rust_rules_and_accepts_the_sample() {
        let schema: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert_schema_keys_match_rust(props);
        assert_status_enum_matches_rust(props);
        let re = regex::Regex::new(props["submit_time"]["pattern"].as_str().unwrap()).unwrap();
        assert_time_pattern_agrees(&re);
        let lines: Vec<&str> = SAMPLE.lines().filter(|l| !l.trim().is_empty()).collect();
        for line in &lines {
            assert_line_fits_schema(line, &schema, &re);
        }
        assert!(lines.len() >= 3, "sample must not be empty");
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
    fn append_after_missing_final_newline_keeps_rows_apart() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("ledger.jsonl");
        std::fs::write(&p, SAMPLE.trim_end_matches('\n')).unwrap();
        let before = parse_ledger(SAMPLE).unwrap().len();
        let r: Row = serde_json::from_str(&row(
            "https://example.test/j/nl",
            "submitted",
            Some("2026-10-26T20:08:00+01:00"),
        ))
        .unwrap();
        append_row(&p, &r).unwrap();
        let rows = parse_ledger(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(rows.len(), before + 1);
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

    /// An atomic replace gives each export a new inode (temp file + rename);
    /// an in-place write would truncate the file a reader may have open.
    #[cfg(unix)]
    pub(crate) fn assert_replaced_atomically(dir: &Path, names: &[&str], write: &dyn Fn()) {
        use std::os::unix::fs::MetadataExt;
        write();
        let ino = |n: &str| std::fs::metadata(dir.join(n)).unwrap().ino();
        let before: Vec<u64> = names.iter().map(|n| ino(n)).collect();
        let held = std::fs::read(dir.join(names[0])).unwrap();
        write();
        for (n, b) in names.iter().zip(&before) {
            assert_ne!(ino(n), *b, "{n} was rewritten in place");
        }
        assert_eq!(std::fs::read(dir.join(names[0])).unwrap(), held);
        let left: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| !names.contains(&n.as_str()))
            .collect();
        assert!(left.is_empty(), "temp files left: {left:?}");
    }

    #[cfg(unix)]
    #[test]
    fn write_exports_replaces_files_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let rows = parse_ledger(SAMPLE).unwrap();
        assert_replaced_atomically(
            dir.path(),
            &["applications.csv", "applications.md"],
            &|| write_exports(&rows, dir.path()).unwrap(),
        );
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
