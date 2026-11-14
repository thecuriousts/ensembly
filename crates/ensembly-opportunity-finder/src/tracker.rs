//! `apps export --tracker`: TRACKER.md from the application ledger.
//!
//! Replaces the ops script gen_tracker.py and must stay byte-equal to it for
//! the same inputs (golden test `tests/tracker_golden.rs`). Current state is
//! the last ledger row per tracker link key. Posted, Lane, Linear, Trello and
//! Pack come from a side file joined by canonical link (fallback: company +
//! role when unique). Side rows with no ledger match, plus any `--prepared`
//! rows, render as "Prepared, not submitted".
use crate::application_ledger::Row;
use std::path::Path;

pub const COLS: [&str; 10] = [
    "Company",
    "Role",
    "Apply URL",
    "Posted",
    "Lane",
    "Status",
    "Submitted at",
    "Linear",
    "Trello",
    "Pack",
];
const JOIN_COLS: [usize; 5] = [3, 4, 7, 8, 9];
const URL: usize = 2;
pub const TRACKER_FILE: &str = "TRACKER.md";

/// One table row, cells in `COLS` order.
pub type Cells = [String; 10];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideFormat {
    /// A hand tracker: the `| Company | … |` table; lines after it are notes.
    Markdown,
    /// One JSON object per line, keys from `COLS` (missing keys are empty).
    Jsonl,
    /// Header row with `COLS` names, then rows (RFC 4180 quoting).
    Csv,
}

impl SideFormat {
    pub fn from_path(p: &Path) -> Self {
        match p.extension().and_then(|e| e.to_str()) {
            Some("jsonl") | Some("json") => SideFormat::Jsonl,
            Some("csv") => SideFormat::Csv,
            _ => SideFormat::Markdown,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Side {
    pub rows: Vec<Cells>,
    /// Lines after the markdown table, carried verbatim.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tracker {
    pub ledger: Vec<Cells>,
    pub prepared: Vec<Cells>,
    pub notes: Vec<String>,
    /// Companies of ledger rows that matched no side row.
    pub unmatched: Vec<String>,
}

/// Python `str.isspace` (Unicode whitespace plus the \x1c-\x1f separators).
fn py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

fn py_strip(s: &str) -> &str {
    s.trim_matches(py_space)
}

/// Python `str.splitlines()` (no keepends).
fn py_splitlines(text: &str) -> Vec<&str> {
    let breaks = |c: char| "\n\r\u{b}\u{c}\u{1c}\u{1d}\u{1e}\u{85}\u{2028}\u{2029}".contains(c);
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if !breaks(c) {
            continue;
        }
        out.push(&text[start..i]);
        let crlf = c == '\r' && matches!(it.peek(), Some((_, '\n')));
        if crlf {
            it.next();
        }
        start = i + c.len_utf8() + usize::from(crlf);
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// Tracker link key: first http(s) URL in the cell, no `#…` or `?…`, no
/// trailing `/`, `.,;`, `/application` or `/apply`, lowercased.
pub fn link_key(cell: &str) -> String {
    let re = regex::Regex::new(r"https?://[^\s|)]+").unwrap();
    let Some(m) = re.find(cell) else {
        return String::new();
    };
    let u = m.as_str();
    let u = u.split('#').next().unwrap_or("");
    let u = u.split('?').next().unwrap_or("");
    let u = u.trim_end_matches('/').trim_end_matches(['.', ',', ';']);
    let u = u.strip_suffix("/application").unwrap_or(u);
    let u = u.strip_suffix("/apply").unwrap_or(u);
    u.trim_end_matches('/').to_lowercase()
}

/// Lowercase ASCII words with parentheticals removed.
fn norm_text(s: &str) -> String {
    let paren = regex::Regex::new(r"\([^)]*\)").unwrap();
    let lower = paren.replace_all(s, " ").to_lowercase();
    let word = regex::Regex::new(r"[a-z0-9]+").unwrap();
    word.find_iter(&lower)
        .map(|m| m.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn cr_key(company: &str, role: &str) -> (String, String) {
    (norm_text(company), norm_text(role))
}

fn instant(t: Option<&str>) -> Option<(i64, u32)> {
    let d = chrono::DateTime::parse_from_rfc3339(t?).ok()?;
    Some((d.timestamp(), d.timestamp_subsec_nanos()))
}

/// Last row per tracker link key; sorted by (no time, instant, link).
pub fn current_rows(rows: &[Row]) -> Vec<Row> {
    let mut latest: std::collections::HashMap<String, &Row> = std::collections::HashMap::new();
    for r in rows {
        latest.insert(link_key(&r.link), r);
    }
    let mut v: Vec<Row> = latest.into_values().cloned().collect();
    v.sort_by_cached_key(|r| {
        let t = instant(r.submit_time.as_deref());
        (t.is_none(), t.unwrap_or((0, 0)), r.link.clone())
    });
    v
}

fn header_or_rule(line: &str, first: &str) -> bool {
    first == "Company" || line.chars().all(|c| matches!(c, '|' | '-' | ' '))
}

/// Split one `|` line into cells; extra pipes are folded into the URL cell.
fn table_cells(line: &str) -> Vec<String> {
    let inner = py_strip(line).trim_matches('|');
    let mut cells: Vec<String> = inner.split('|').map(|c| py_strip(c).to_string()).collect();
    if cells.len() > COLS.len() {
        let extra = cells.len() - COLS.len();
        let url = cells[2..3 + extra].join(" ");
        cells.splice(2..3 + extra, [url]);
    }
    cells
}

fn to_cells(v: Vec<String>) -> Cells {
    let mut it = v.into_iter();
    std::array::from_fn(|_| it.next().unwrap_or_default())
}

/// Parse a markdown hand tracker the way gen_tracker.py does.
pub fn parse_markdown(text: &str) -> Side {
    let mut side = Side::default();
    let (mut in_table, mut after) = (false, false);
    for line in py_splitlines(text) {
        if line.starts_with('|') {
            let cells = table_cells(line);
            if header_or_rule(line, &cells[0]) {
                in_table = true;
            } else if cells.len() >= COLS.len() {
                side.rows.push(to_cells(cells));
            }
            continue;
        }
        after |= in_table;
        if after {
            side.notes.push(line.to_string());
        }
    }
    side
}

fn parse_jsonl(text: &str) -> anyhow::Result<Side> {
    let mut side = Side::default();
    for (i, line) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let v: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(line).map_err(|e| anyhow::anyhow!("line {}: {e}", i + 1))?;
        if let Some(k) = v.keys().find(|k| !COLS.contains(&k.as_str())) {
            anyhow::bail!("line {}: unknown column {k:?}", i + 1);
        }
        let cell = |c: &str| v.get(c).and_then(|x| x.as_str()).unwrap_or("").to_string();
        side.rows.push(std::array::from_fn(|j| cell(COLS[j])));
    }
    Ok(side)
}

/// Minimal RFC 4180 reader: quoted fields, doubled quotes, CRLF or LF.
fn csv_records(text: &str) -> Vec<Vec<String>> {
    let (mut recs, mut rec, mut field) = (Vec::new(), Vec::new(), String::new());
    let (mut quoted, mut chars) = (false, text.chars().peekable());
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') => quoted = true,
            (false, ',') => rec.push(std::mem::take(&mut field)),
            (false, '\r') => {}
            (false, '\n') => {
                rec.push(std::mem::take(&mut field));
                recs.push(std::mem::take(&mut rec));
            }
            (false, c) => field.push(c),
        }
    }
    if !field.is_empty() || !rec.is_empty() {
        rec.push(field);
        recs.push(rec);
    }
    recs
}

fn parse_csv(text: &str) -> anyhow::Result<Side> {
    let mut recs = csv_records(text).into_iter();
    let header = recs.next().unwrap_or_default();
    if let Some(h) = header.iter().find(|h| !COLS.contains(&h.as_str())) {
        anyhow::bail!("csv header: unknown column {h:?}");
    }
    let idx: Vec<Option<usize>> = COLS
        .iter()
        .map(|c| header.iter().position(|h| h == c))
        .collect();
    let rows = recs
        .filter(|r| r.iter().any(|f| !f.is_empty()))
        .map(|r| {
            std::array::from_fn(|j| idx[j].and_then(|i| r.get(i).cloned()).unwrap_or_default())
        })
        .collect();
    Ok(Side {
        rows,
        notes: Vec::new(),
    })
}

pub fn parse_side(text: &str, format: SideFormat) -> anyhow::Result<Side> {
    match format {
        SideFormat::Markdown => Ok(parse_markdown(text)),
        SideFormat::Jsonl => parse_jsonl(text),
        SideFormat::Csv => parse_csv(text),
    }
}

/// "2026-10-27 10:00 CET"; "unknown" when flagged; else "-".
fn fmt_time(r: &Row) -> String {
    let Some(t) = r.submit_time.as_deref() else {
        let unknown = r.submit_time_unknown == Some(true);
        return if unknown { "unknown" } else { "-" }.to_string();
    };
    let Ok(d) = chrono::DateTime::parse_from_rfc3339(t) else {
        return t.to_string();
    };
    let zone = match d.offset().local_minus_utc() {
        3600 => "CET".to_string(),
        7200 => "CEST".to_string(),
        _ => d.format("%z").to_string(),
    };
    format!("{} {zone}", d.format("%Y-%m-%d %H:%M"))
}

/// `prepared` -> `Prepared`, `silent-closed` -> `Silent-closed`.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| {
            f.to_uppercase()
                .chain(c.flat_map(char::to_lowercase))
                .collect()
        })
        .unwrap_or_default()
}

struct Index {
    by_link: std::collections::HashMap<String, Vec<usize>>,
    by_cr: std::collections::HashMap<(String, String), Vec<usize>>,
}

impl Index {
    fn new(side: &[Cells]) -> Self {
        let mut ix = Index {
            by_link: Default::default(),
            by_cr: Default::default(),
        };
        for (i, o) in side.iter().enumerate() {
            ix.by_link.entry(link_key(&o[URL])).or_default().push(i);
            ix.by_cr.entry(cr_key(&o[0], &o[1])).or_default().push(i);
        }
        ix
    }

    /// First unused side row by link (posting, then apply URL), else the
    /// only unused side row with the same company + role. An absent
    /// apply_url keys as "" like the Python script, so it can take a side
    /// row that has no URL.
    fn find(&self, r: &Row, used: &[bool]) -> Option<usize> {
        let none = Vec::new();
        let get = |k: String| self.by_link.get(&k).unwrap_or(&none);
        let apply = link_key(r.apply_url.as_deref().unwrap_or(""));
        let by_link = get(link_key(&r.link)).iter().chain(get(apply).iter());
        if let Some(&i) = by_link.clone().find(|&&i| !used[i]) {
            return Some(i);
        }
        let cands: Vec<usize> = self
            .by_cr
            .get(&cr_key(&r.company, &r.role))
            .unwrap_or(&none)
            .iter()
            .copied()
            .filter(|&i| !used[i])
            .collect();
        (cands.len() == 1).then(|| cands[0])
    }
}

fn ledger_cells(r: &Row, hit: Option<&Cells>) -> Cells {
    let mut c: Cells = Default::default();
    c[0] = r.company.clone();
    c[1] = r.role.clone();
    c[URL] = r.apply_url.clone().unwrap_or_else(|| r.link.clone());
    c[5] = capitalize(r.status.as_str());
    c[6] = fmt_time(r);
    for j in JOIN_COLS {
        c[j] = hit.map(|h| h[j].clone()).unwrap_or_else(|| "-".into());
    }
    c
}

/// Join current ledger state with the side rows; unmatched side rows and the
/// extra `prepared` rows form the prepared section.
pub fn build(rows: &[Row], join: &Side, prepared: &[Cells]) -> Tracker {
    let ix = Index::new(&join.rows);
    let mut used = vec![false; join.rows.len()];
    let (mut ledger, mut unmatched) = (Vec::new(), Vec::new());
    for r in current_rows(rows) {
        let hit = ix.find(&r, &used);
        if let Some(i) = hit {
            used[i] = true;
        } else {
            unmatched.push(r.company.clone());
        }
        ledger.push(ledger_cells(&r, hit.map(|i| &join.rows[i])));
    }
    let mut rest: Vec<Cells> = (0..join.rows.len())
        .filter(|&i| !used[i])
        .map(|i| join.rows[i].clone())
        .collect();
    rest.extend(prepared.iter().cloned());
    Tracker {
        ledger,
        prepared: rest,
        notes: join.notes.clone(),
        unmatched,
    }
}

fn table(rows: &[Cells]) -> Vec<String> {
    let mut l = vec![
        format!("| {} |", COLS.join(" | ")),
        format!("|{}", "---|".repeat(COLS.len())),
    ];
    for r in rows {
        let cells: Vec<String> = r
            .iter()
            .map(|c| {
                if c.is_empty() {
                    "-".into()
                } else {
                    c.replace('|', "/")
                }
            })
            .collect();
        l.push(format!("| {} |", cells.join(" | ")));
    }
    l
}

/// TRACKER.md text. `stamp` is "YYYY-MM-DD HH:MM" Stockholm time.
/// The header names gen_tracker.py so the bytes match the script it replaces.
pub fn render(t: &Tracker, stamp: &str) -> String {
    let mut l: Vec<String> = vec![
        "# Applications tracker (generated from the Ensembly ledger; do not hand-edit)".into(),
        format!("Generated {stamp} Stockholm (CET) by gen_tracker.py. Ledger is the source of truth; Posted/Lane/Linear/Trello/Pack joined from the hand tracker. Box only, never git."),
        String::new(),
        format!("## Applications in the ledger ({})", t.ledger.len()),
        String::new(),
    ];
    l.extend(table(&t.ledger));
    l.extend([
        String::new(),
        format!("## Prepared, not submitted ({})", t.prepared.len()),
        "Rows from the hand tracker with no ledger entry (packs ready, never submitted). Status kept as written.".into(),
        String::new(),
    ]);
    l.extend(table(&t.prepared));
    if t.notes.iter().any(|n| !py_strip(n).is_empty()) {
        l.extend([
            "".into(),
            "## Notes carried from the hand tracker".into(),
            "".into(),
        ]);
        l.extend(t.notes.iter().cloned());
    }
    format!("{}\n", l.join("\n").trim_end_matches(py_space))
}

/// Summary printed by the CLI (same keys as gen_tracker.py).
pub fn summary(t: &Tracker, join_rows: usize, ledger_current: usize) -> serde_json::Value {
    let mut not_ready: Vec<&str> = t
        .prepared
        .iter()
        .filter(|o| !o[5].starts_with("Ready to submit"))
        .map(|o| o[0].as_str())
        .collect();
    not_ready.sort();
    not_ready.dedup();
    serde_json::json!({
        "old_rows": join_rows,
        "ledger_current": ledger_current,
        "generated": t.ledger.len(),
        "prepared_not_submitted": t.prepared.len(),
        "prepared_not_ready": not_ready,
        "ledger_without_hand_row": t.unmatched,
    })
}

/// Replace `path` atomically (temp file in the same directory, synced, then
/// renamed); shared with the ledger and audit exports.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    crate::application_ledger::write_atomic(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_key_matches_the_script() {
        for (i, o) in [
            (
                "https://ats.example.test/b/1/application see note",
                "https://ats.example.test/b/1",
            ),
            (
                "https://x.example.test/a/#apply",
                "https://x.example.test/a",
            ),
            (
                "https://jobs.example.test/s/1/apply",
                "https://jobs.example.test/s/1",
            ),
            (
                "https://Jobs.Example.test/W/9.",
                "https://jobs.example.test/w/9",
            ),
            (
                "see (https://x.example.test/q?a=1)",
                "https://x.example.test/q",
            ),
            ("no link", ""),
        ] {
            assert_eq!(link_key(i), o, "{i}");
        }
    }

    #[test]
    fn norm_text_drops_parentheticals_and_punctuation() {
        assert_eq!(norm_text("Engineer, EMEA (London)"), "engineer emea");
        assert_eq!(cr_key("Acme AB", "Dev"), ("acme ab".into(), "dev".into()));
    }

    #[test]
    fn splitlines_and_strip_follow_python() {
        assert_eq!(
            py_splitlines("a\r\nb\rc\n\nd\u{2028}e\n"),
            ["a", "b", "c", "", "d", "e"]
        );
        assert_eq!(py_strip("\u{1f} x \u{a0}"), "x");
    }

    #[test]
    fn side_files_in_jsonl_and_csv_read_like_markdown() {
        let md = "| Company | Role | Apply URL | Posted | Lane | Status | Submitted at | Linear | Trello | Pack |\n|---|\n| A | R | https://a.example.test/1 | p | l | Ready | - | L-1 | t | k |\n";
        let jsonl = r#"{"Company":"A","Role":"R","Apply URL":"https://a.example.test/1","Posted":"p","Lane":"l","Status":"Ready","Submitted at":"-","Linear":"L-1","Trello":"t","Pack":"k"}"#;
        let csv = "Company,Role,Apply URL,Posted,Lane,Status,Submitted at,Linear,Trello,Pack\nA,R,https://a.example.test/1,p,l,Ready,-,L-1,t,k\n";
        let m = parse_side(md, SideFormat::Markdown).unwrap().rows;
        assert_eq!(m.len(), 1);
        assert_eq!(parse_side(jsonl, SideFormat::Jsonl).unwrap().rows, m);
        assert_eq!(parse_side(csv, SideFormat::Csv).unwrap().rows, m);
        let quoted = "Company,Role\n\"B, \"\"x\"\"\",R\n";
        assert_eq!(
            parse_side(quoted, SideFormat::Csv).unwrap().rows[0][0],
            "B, \"x\""
        );
        assert!(parse_side(r#"{"Phone":"x"}"#, SideFormat::Jsonl).is_err());
        assert!(parse_side("Nope\n1\n", SideFormat::Csv).is_err());
    }

    #[test]
    fn prepared_side_rows_join_the_prepared_section() {
        let extra: Cells = std::array::from_fn(|j| {
            if j == 0 {
                "Extra".into()
            } else {
                String::new()
            }
        });
        let t = build(&[], &Side::default(), std::slice::from_ref(&extra));
        assert_eq!(t.prepared, vec![extra]);
        let md = render(&t, "s");
        assert!(md.contains("## Prepared, not submitted (1)"), "{md}");
        assert!(md.contains("| Extra | - | - |"), "{md}");
        assert!(md.ends_with("|\n"));
    }

    #[test]
    fn write_atomic_replaces_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(TRACKER_FILE);
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"two");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
