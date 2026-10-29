//! Company audit store — append-only scam-risk / due-diligence audits per company,
//! checked before a pack is prepared or an application is sent.
//! Last row per canonical domain wins. The verdict is derived from the checks
//! and validated, never trusted.
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Real audits live here, under gitignored `data/`.
pub const DEFAULT_AUDIT_PATH: &str = "data/local/applications/company-audits.jsonl";
/// Audits older than this many days count as stale.
pub const STALE_AFTER_DAYS: i64 = 30;
/// An `audited_at` more than this many minutes after `now` reads as stale:
/// a future stamp would otherwise stay fresh for ever.
pub const FUTURE_SKEW_MINUTES: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Pass,
    Flag,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckResult {
    Pass,
    Flag,
    Block,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckId {
    ApplyDomain,
    DomainAge,
    FundingOwnership,
    Headcount,
    ReviewsNews,
    DuplicateGhost,
    PaymentIdAsk,
    AgencyRepost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub id: CheckId,
    pub result: CheckResult,
    pub note: String,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audit {
    pub company: String,
    pub domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_url: Option<String>,
    pub verdict: Verdict,
    pub checks: Vec<Check>,
    pub audited_at: String,
    pub audited_by: String,
    /// Optional `audited_at` of the earlier audit whose result this row reuses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reused_from: Option<String>,
}

/// Gate status for `audit status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GateStatus {
    Pass,
    Flag,
    Block,
    Unaudited,
    Stale,
}

impl CheckId {
    pub const ALL: [CheckId; 8] = [
        CheckId::ApplyDomain,
        CheckId::DomainAge,
        CheckId::FundingOwnership,
        CheckId::Headcount,
        CheckId::ReviewsNews,
        CheckId::DuplicateGhost,
        CheckId::PaymentIdAsk,
        CheckId::AgencyRepost,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            CheckId::ApplyDomain => "apply_domain",
            CheckId::DomainAge => "domain_age",
            CheckId::FundingOwnership => "funding_ownership",
            CheckId::Headcount => "headcount",
            CheckId::ReviewsNews => "reviews_news",
            CheckId::DuplicateGhost => "duplicate_ghost",
            CheckId::PaymentIdAsk => "payment_id_ask",
            CheckId::AgencyRepost => "agency_repost",
        }
    }
    /// Checks whose `unknown` alone is enough to flag: without them the employer
    /// cannot be shown to exist as stated.
    fn unknown_flags(self) -> bool {
        matches!(
            self,
            CheckId::ApplyDomain | CheckId::DomainAge | CheckId::FundingOwnership
        )
    }
}

impl CheckResult {
    pub fn as_str(self) -> &'static str {
        match self {
            CheckResult::Pass => "pass",
            CheckResult::Flag => "flag",
            CheckResult::Block => "block",
            CheckResult::Unknown => "unknown",
        }
    }
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Flag => "flag",
            Verdict::Block => "block",
        }
    }
}

/// block if any check is block; flag if any check is flag, any of
/// apply_domain / domain_age / funding_ownership is unknown, or headcount and
/// reviews_news are both unknown; else pass.
pub fn derive_verdict(checks: &[Check]) -> Verdict {
    let unknown = |id: CheckId| {
        checks
            .iter()
            .any(|c| c.id == id && c.result == CheckResult::Unknown)
    };
    let size_unknown = unknown(CheckId::Headcount) && unknown(CheckId::ReviewsNews);
    match derive_verdict_before_size_rule(checks) {
        Verdict::Pass if size_unknown => Verdict::Flag,
        v => v,
    }
}

/// The rule before 30 Oct 2026, without the headcount + reviews_news case.
/// Only used to keep rows stored under that rule readable.
fn derive_verdict_before_size_rule(checks: &[Check]) -> Verdict {
    if checks.iter().any(|c| c.result == CheckResult::Block) {
        Verdict::Block
    } else if checks.iter().any(|c| {
        c.result == CheckResult::Flag || (c.result == CheckResult::Unknown && c.id.unknown_flags())
    }) {
        Verdict::Flag
    } else {
        Verdict::Pass
    }
}

/// Lowercase host without scheme, `www.`, path, query, port or trailing dot.
pub fn canonical_domain(s: &str) -> String {
    let s = s.trim().to_lowercase();
    let s = s.split_once("://").map(|(_, r)| r.to_string()).unwrap_or(s);
    let host = s.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    let host = host.trim_end_matches('.');
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

fn email_re() -> regex::Regex {
    regex::Regex::new(r"[^\s@]+@[^\s@]+\.[A-Za-z]{2,}").unwrap()
}

/// Phone-like: 9+ digits written with a leading `+`, a leading 0, or separators
/// (space, dash, dot, parentheses). ISO dates are removed first; plain amounts
/// (no separators, no leading 0 or +) pass.
fn looks_like_phone(note: &str) -> bool {
    let dates = regex::Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap();
    let cleaned = dates.replace_all(note, " ");
    let cand = regex::Regex::new(r"\+?\(?\d[\d\s().-]{5,}\d").unwrap();
    let hit = cand.find_iter(&cleaned).any(|m| {
        let t = m.as_str();
        let digits = t.chars().filter(|c| c.is_ascii_digit()).count();
        let separated = t.chars().any(|c| matches!(c, ' ' | '-' | '.' | '(' | ')'));
        let prefixed = t.starts_with('+') || t.starts_with('0') || t.starts_with('(');
        digits >= 9 && (separated || prefixed)
    });
    hit
}

fn is_https(u: &str) -> bool {
    u.starts_with("https://") && u.len() > "https://".len() && !u.contains(char::is_whitespace)
}

fn validate_optional_fields(a: &Audit) -> anyhow::Result<()> {
    if let Some(u) = &a.apply_url {
        if !is_https(u) {
            anyhow::bail!("apply_url must be an https URL");
        }
    }
    if let Some(r) = &a.reused_from {
        crate::application_ledger::check_submit_time(r)
            .map_err(|e| anyhow::anyhow!("reused_from: {e}"))?;
    }
    Ok(())
}

impl Audit {
    /// The verdict the checks give under the current rule. Equals `verdict`
    /// for every row except a stored legacy pass that now derives flag.
    pub fn effective_verdict(&self) -> Verdict {
        derive_verdict(&self.checks)
    }
}

/// Strict check for new rows: the stated verdict must equal the derived one.
pub fn validate_audit(a: &Audit) -> anyhow::Result<()> {
    validate_audit_body(a)?;
    check_stated_verdict(a, derive_verdict(&a.checks))
}

/// Check for rows already in the store: as strict, except a stated `pass`
/// that was right under the earlier rule still reads (as flag, see
/// `effective_verdict`), so an append-only store never turns unreadable.
pub fn validate_stored_audit(a: &Audit) -> anyhow::Result<()> {
    validate_audit_body(a)?;
    let derived = derive_verdict(&a.checks);
    let legacy_pass =
        a.verdict == Verdict::Pass && derive_verdict_before_size_rule(&a.checks) == Verdict::Pass;
    if legacy_pass {
        return Ok(());
    }
    check_stated_verdict(a, derived)
}

fn check_stated_verdict(a: &Audit, derived: Verdict) -> anyhow::Result<()> {
    if derived != a.verdict {
        anyhow::bail!(
            "stated verdict {} disagrees with the checks (derived: {})",
            a.verdict.as_str(),
            derived.as_str()
        );
    }
    Ok(())
}

fn validate_audit_body(a: &Audit) -> anyhow::Result<()> {
    for (name, v) in [("company", &a.company), ("audited_by", &a.audited_by)] {
        if v.trim().is_empty() {
            anyhow::bail!("{name} must not be blank");
        }
    }
    let dom = canonical_domain(&a.domain);
    let dom_re = regex::Regex::new(r"^[a-z0-9-]+(\.[a-z0-9-]+)+$").unwrap();
    if dom != a.domain || !dom_re.is_match(&dom) {
        anyhow::bail!(
            "domain {:?} must be a canonical host like {:?}",
            a.domain,
            dom
        );
    }
    validate_optional_fields(a)?;
    crate::application_ledger::check_submit_time(&a.audited_at)
        .map_err(|e| anyhow::anyhow!("audited_at: {e}"))?;
    for id in CheckId::ALL {
        let n = a.checks.iter().filter(|c| c.id == id).count();
        if n != 1 {
            anyhow::bail!("check {} must appear exactly once (found {n})", id.as_str());
        }
    }
    if a.checks.len() != CheckId::ALL.len() {
        anyhow::bail!(
            "expected {} checks, found {}",
            CheckId::ALL.len(),
            a.checks.len()
        );
    }
    for c in &a.checks {
        let id = c.id.as_str();
        if c.note.trim().is_empty() {
            anyhow::bail!("check {id}: note must not be blank");
        }
        if email_re().is_match(&c.note) || looks_like_phone(&c.note) {
            anyhow::bail!(
                "check {id}: note looks like it contains personal data (email or phone number)"
            );
        }
        if let Some(bad) = c.sources.iter().find(|u| !is_https(u)) {
            anyhow::bail!("check {id}: source {bad:?} is not an https URL");
        }
        if c.result != CheckResult::Unknown && c.sources.is_empty() {
            anyhow::bail!(
                "check {id}: result {} needs at least one https source",
                c.result.as_str()
            );
        }
    }
    Ok(())
}

/// Parse a stored JSONL store (blank lines ignored). Errors name the 1-based line.
pub fn parse_audits(text: &str) -> anyhow::Result<Vec<Audit>> {
    parse_with(text, validate_stored_audit)
}

/// Parse new input rows with the strict rule.
pub fn parse_new_audits(text: &str) -> anyhow::Result<Vec<Audit>> {
    parse_with(text, validate_audit)
}

fn parse_with(text: &str, check: fn(&Audit) -> anyhow::Result<()>) -> anyhow::Result<Vec<Audit>> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let a: Audit =
            serde_json::from_str(line).map_err(|e| anyhow::anyhow!("line {}: {e}", i + 1))?;
        check(&a).map_err(|e| anyhow::anyhow!("line {}: {e}", i + 1))?;
        out.push(a);
    }
    Ok(out)
}

/// Last row per canonical domain, sorted by domain.
pub fn current_audits(rows: &[Audit]) -> Vec<Audit> {
    let mut m: std::collections::BTreeMap<String, &Audit> = std::collections::BTreeMap::new();
    for r in rows {
        m.insert(canonical_domain(&r.domain), r);
    }
    m.into_values().cloned().collect()
}

/// Look up by domain (anything containing a dot, URLs included) or by company
/// name (case-insensitive). Several domains for one company: the most recent audit.
pub fn status_for(
    rows: &[Audit],
    query: &str,
    now: chrono::DateTime<chrono::FixedOffset>,
) -> (GateStatus, Option<Audit>) {
    let cur = current_audits(rows);
    let q = query.trim();
    let hit = if q.contains('.') {
        let d = canonical_domain(q);
        cur.into_iter().find(|a| a.domain == d)
    } else {
        let q = q.to_lowercase();
        cur.into_iter()
            .filter(|a| a.company.trim().to_lowercase() == q)
            .max_by_key(|a| {
                chrono::DateTime::parse_from_rfc3339(&a.audited_at)
                    .map(|t| t.timestamp())
                    .unwrap_or(0)
            })
    };
    let Some(a) = hit else {
        return (GateStatus::Unaudited, None);
    };
    // Fail closed: rows built outside `validate` may carry a bad timestamp.
    let Ok(at) = chrono::DateTime::parse_from_rfc3339(&a.audited_at) else {
        return (GateStatus::Stale, Some(a));
    };
    let age = now.signed_duration_since(at);
    if age > chrono::Duration::days(STALE_AFTER_DAYS)
        || age < -chrono::Duration::minutes(FUTURE_SKEW_MINUTES)
    {
        return (GateStatus::Stale, Some(a));
    }
    let s = match a.effective_verdict() {
        Verdict::Pass => GateStatus::Pass,
        Verdict::Flag => GateStatus::Flag,
        Verdict::Block => GateStatus::Block,
    };
    (s, Some(a))
}

/// 0 pass, 3 flag, 4 block, 5 unaudited or stale. (1/2 stay free for errors/usage.)
pub fn exit_code(s: GateStatus) -> i32 {
    match s {
        GateStatus::Pass => 0,
        GateStatus::Flag => 3,
        GateStatus::Block => 4,
        GateStatus::Unaudited | GateStatus::Stale => 5,
    }
}

fn result_of(a: &Audit, id: CheckId) -> &'static str {
    a.checks
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.result.as_str())
        .unwrap_or("")
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn export_csv(rows: &[Audit]) -> String {
    let mut out = String::from("domain,company,verdict,audited_at,audited_by,apply_url");
    for id in CheckId::ALL {
        out.push(',');
        out.push_str(id.as_str());
    }
    out.push('\n');
    for a in current_audits(rows) {
        let mut f: Vec<String> = [
            a.domain.as_str(),
            a.company.as_str(),
            a.effective_verdict().as_str(),
            a.audited_at.as_str(),
            a.audited_by.as_str(),
            a.apply_url.as_deref().unwrap_or(""),
        ]
        .iter()
        .map(|x| csv_field(x))
        .collect();
        f.extend(CheckId::ALL.iter().map(|id| result_of(&a, *id).to_string()));
        out.push_str(&f.join(","));
        out.push('\n');
    }
    out
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}

pub fn export_markdown(rows: &[Audit]) -> String {
    let mut out = String::from("| Domain | Company | Verdict | Audited | By |");
    let mut sep = String::from("|---|---|---|---|---|");
    for id in CheckId::ALL {
        out.push_str(&format!(" {} |", id.as_str()));
        sep.push_str("---|");
    }
    out.push('\n');
    out.push_str(&sep);
    out.push('\n');
    for a in current_audits(rows) {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |",
            md_cell(&a.domain),
            md_cell(&a.company),
            a.effective_verdict().as_str(),
            md_cell(&a.audited_at),
            md_cell(&a.audited_by)
        ));
        for id in CheckId::ALL {
            out.push_str(&format!(" {} |", result_of(&a, id)));
        }
        out.push('\n');
    }
    out
}

/// Append one validated audit; an identical row (after trimming) is refused (exit 1).
pub fn append_audit(path: &Path, a: &Audit) -> anyhow::Result<()> {
    use std::io::Write;
    validate_audit(a)?;
    let existing = match std::fs::read_to_string(path) {
        Ok(t) => parse_audits(&t)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    if existing.iter().any(|x| x == a) {
        anyhow::bail!("identical audit already in store for {}", a.domain);
    }
    check_reuse(&existing, a)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = crate::application_ledger::open_jsonl_append(path)?;
    writeln!(f, "{}", serde_json::to_string(a)?)?;
    Ok(())
}

/// Batch append from JSONL text: every line is validated first (any error
/// writes nothing); rows already in the store are skipped. Returns
/// (appended, skipped). Re-running the same batch appends nothing.
pub fn append_audits(path: &Path, text: &str) -> anyhow::Result<(usize, usize)> {
    use std::io::Write;
    let incoming = parse_new_audits(text)?;
    let mut existing = match std::fs::read_to_string(path) {
        Ok(t) => parse_audits(&t)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let mut lines = String::new();
    let (mut appended, mut skipped) = (0, 0);
    for a in incoming {
        if existing.contains(&a) {
            skipped += 1;
            continue;
        }
        check_reuse(&existing, &a)?;
        lines.push_str(&serde_json::to_string(&a)?);
        lines.push('\n');
        existing.push(a);
        appended += 1;
    }
    if appended > 0 {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut f = crate::application_ledger::open_jsonl_append(path)?;
        f.write_all(lines.as_bytes())?;
    }
    Ok((appended, skipped))
}

fn instant(t: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(t)
        .ok()
        .map(|d| d.timestamp())
}

/// `reused_from`, when set, must be the `audited_at` of an audit of the same
/// domain that comes before `a` in the store and is earlier in time.
fn reuse_target_ok(prior: &[Audit], a: &Audit) -> bool {
    let Some(rf) = a.reused_from.as_deref() else {
        return true;
    };
    let earlier = matches!((instant(rf), instant(&a.audited_at)), (Some(f), Some(t)) if f < t);
    earlier
        && prior
            .iter()
            .any(|p| p.domain == a.domain && p.audited_at == rf)
}

fn check_reuse(prior: &[Audit], a: &Audit) -> anyhow::Result<()> {
    if !reuse_target_ok(prior, a) {
        anyhow::bail!(
            "reused_from {:?} for {} does not name an earlier stored audit of that domain",
            a.reused_from.as_deref().unwrap_or(""),
            a.domain
        );
    }
    Ok(())
}

/// Indices of stored rows whose `reused_from` names no earlier audit of the
/// same domain. Rows stored before this check existed may dangle; new ones cannot.
pub fn dangling_reuse(rows: &[Audit]) -> Vec<usize> {
    (0..rows.len())
        .filter(|&i| !reuse_target_ok(&rows[..i], &rows[i]))
        .collect()
}

/// Write `company-audits.csv` + `company-audits.md` (byte-identical on re-run).
pub fn write_exports(rows: &[Audit], dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("company-audits.csv"), export_csv(rows))?;
    std::fs::write(dir.join("company-audits.md"), export_markdown(rows))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../../../fixtures/company-audits-sample.jsonl");
    const SCHEMA: &str = include_str!("../../../schemas/company-audit-row.schema.json");

    fn sample() -> Vec<Audit> {
        parse_audits(SAMPLE).expect("sample valid")
    }
    fn first() -> Audit {
        sample()
            .into_iter()
            .next()
            .expect("sample must not be empty")
    }
    fn now(s: &str) -> chrono::DateTime<chrono::FixedOffset> {
        chrono::DateTime::parse_from_rfc3339(s).unwrap()
    }

    #[test]
    fn sample_parses_to_three_audits() {
        let rows = sample();
        assert_eq!(rows.len(), 3, "empty parse must fail");
        assert_eq!(rows[2].verdict, Verdict::Block);
    }

    #[test]
    fn verdict_is_derived() {
        let mut a = first();
        assert_eq!(derive_verdict(&a.checks), Verdict::Pass);
        a.checks[3].result = CheckResult::Flag; // headcount
        assert_eq!(derive_verdict(&a.checks), Verdict::Flag);
        a.checks[6].result = CheckResult::Block; // payment_id_ask
        assert_eq!(derive_verdict(&a.checks), Verdict::Block);
        let mut b = first();
        b.checks[3].result = CheckResult::Unknown; // headcount unknown alone -> still pass
        assert_eq!(derive_verdict(&b.checks), Verdict::Pass);
        for i in 0..3 {
            // apply_domain / domain_age / funding_ownership unknown -> flag
            let mut c = first();
            c.checks[i].result = CheckResult::Unknown;
            assert_eq!(derive_verdict(&c.checks), Verdict::Flag, "check {i}");
        }
    }

    fn both_size_checks_unknown(mut a: Audit) -> Audit {
        for i in [3, 4] {
            // headcount, reviews_news
            a.checks[i].result = CheckResult::Unknown;
            a.checks[i].sources.clear();
        }
        a
    }

    #[test]
    fn unknown_headcount_and_reviews_news_together_flag() {
        let a = both_size_checks_unknown(first());
        assert_eq!(derive_verdict(&a.checks), Verdict::Flag);
        // a new row stating pass is refused
        let err = validate_audit(&a).unwrap_err();
        assert!(err.to_string().contains("verdict"), "{err}");
        let mut b = a.clone();
        b.verdict = Verdict::Flag;
        validate_audit(&b).unwrap();
    }

    #[test]
    fn stored_legacy_pass_with_both_unknown_still_parses_but_reads_flag() {
        let a = both_size_checks_unknown(first());
        let line = serde_json::to_string(&a).unwrap(); // stated pass, old rule
        let rows = parse_audits(&line).expect("legacy store row stays readable");
        let t = now("2026-10-27T12:00:00+01:00");
        assert_eq!(
            status_for(&rows, "northwind.example.test", t).0,
            GateStatus::Flag
        );
        assert!(export_csv(&rows).contains(",flag,"), "export shows derived");
        // a batch append is strict: the same row is refused as new input
        let dir = tempfile::tempdir().unwrap();
        assert!(append_audits(&dir.path().join("a.jsonl"), &line).is_err());
    }

    #[test]
    fn stated_verdict_disagreeing_with_checks_is_refused() {
        let mut a = first();
        a.checks[6].result = CheckResult::Block;
        a.checks[6].sources = vec!["https://x.example.test/p".into()];
        let err = validate_audit(&a).unwrap_err(); // still says pass
        assert!(err.to_string().contains("verdict"), "{err}");
        a.verdict = Verdict::Block;
        validate_audit(&a).unwrap();
    }

    #[test]
    fn every_check_id_exactly_once() {
        let mut a = first();
        a.checks.pop();
        assert!(validate_audit(&a).is_err());
        let mut b = first();
        b.checks[7] = b.checks[0].clone();
        assert!(validate_audit(&b).is_err());
    }

    #[test]
    fn non_unknown_results_need_an_https_source() {
        let mut a = first();
        a.checks[0].sources.clear();
        assert!(validate_audit(&a).is_err());
        let mut b = first();
        b.checks[0].sources = vec!["http://x.example.test".into()];
        assert!(validate_audit(&b).is_err());
        let rows = sample(); // contoso: unknown check with no sources is fine
        assert!(rows[1]
            .checks
            .iter()
            .any(|c| c.result == CheckResult::Unknown && c.sources.is_empty()));
    }

    #[test]
    fn notes_refuse_personal_data_but_keep_dates_and_amounts() {
        for bad in [
            "call +00 70 123 45 67",
            "mail hr@x.example.test",
            "070-174 06 50", // fiction range 070-1740605..699
            "(555) 123-4567",
        ] {
            let mut a = first();
            a.checks[0].note = bad.into();
            assert!(validate_audit(&a).is_err(), "{bad}");
        }
        for ok in [
            "Registered 2019-04-12; raised 15000000 EUR",
            "~1,200 staff",
            "Series B 2024-01-01 to 2024-12-31",
        ] {
            let mut a = first();
            a.checks[0].note = ok.into();
            validate_audit(&a).unwrap_or_else(|e| panic!("{ok}: {e}"));
        }
    }

    #[test]
    fn reused_from_is_optional_and_checked() {
        let base = SAMPLE.lines().next().unwrap();
        let with = base.replacen("{", r#"{"reused_from":"2026-10-20T09:00:00+02:00","#, 1);
        let rows = parse_audits(&with).expect("row with reused_from validates");
        assert_eq!(
            rows[0].reused_from.as_deref(),
            Some("2026-10-20T09:00:00+02:00")
        );
        assert!(
            parse_audits(base).is_ok(),
            "row without reused_from still validates"
        );
        let bad = base.replacen("{", r#"{"reused_from":"yesterday","#, 1);
        assert!(
            parse_audits(&bad).is_err(),
            "reused_from must be a timestamp"
        );
        let extra = base.replacen("{", r#"{"reused_by":"x","#, 1);
        assert!(parse_audits(&extra).is_err(), "unknown fields stay refused");
    }

    fn reuse_of(a: &Audit, from: &str, at: &str) -> Audit {
        let mut r = a.clone();
        r.reused_from = Some(from.into());
        r.audited_at = at.into();
        r
    }

    #[test]
    fn reused_from_must_name_an_earlier_stored_audit_of_the_same_domain() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.jsonl");
        let rows = sample();
        let line = |a: &Audit| serde_json::to_string(a).unwrap();
        // nothing stored yet: the reference dangles
        let dangling = reuse_of(
            &rows[0],
            "2026-10-20T09:00:00+02:00",
            "2026-10-28T09:00:00+01:00",
        );
        assert!(append_audits(&p, &line(&dangling)).is_err());
        assert!(append_audit(&p, &dangling).is_err());
        assert!(!p.exists(), "nothing written");
        append_audits(&p, SAMPLE).unwrap();
        // contoso's stamp exists, but for another domain
        let other = reuse_of(
            &rows[0],
            "2026-10-21T10:00:00+02:00",
            "2026-10-28T09:00:00+01:00",
        );
        assert!(append_audits(&p, &line(&other)).is_err());
        // a reference to a later stamp is not an earlier audit
        let later = reuse_of(
            &rows[0],
            "2026-10-20T09:00:00+02:00",
            "2026-10-19T09:00:00+02:00",
        );
        assert!(append_audits(&p, &line(&later)).is_err());
        // same domain, earlier, stored: accepted (also within one batch)
        assert_eq!(append_audits(&p, &line(&dangling)).unwrap(), (1, 0));
        let again = reuse_of(
            &rows[0],
            "2026-10-28T09:00:00+01:00",
            "2026-10-29T09:00:00+01:00",
        );
        append_audit(&p, &again).unwrap();
        let stored = parse_audits(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert!(dangling_reuse(&stored).is_empty());
        assert_eq!(dangling_reuse(&[dangling]), vec![0]);
    }

    #[test]
    fn rejects_bad_domain_time_and_unknown_fields() {
        let mut a = first();
        a.domain = "https://Northwind.example.test/".into();
        assert!(
            validate_audit(&a).is_err(),
            "domain must be stored canonical"
        );
        let mut b = first();
        b.audited_at = "2026-10-20T09:00:00Z".into();
        assert!(validate_audit(&b).is_err());
        let line = SAMPLE
            .lines()
            .next()
            .unwrap()
            .replacen("{", r#"{"phone":"x","#, 1);
        assert!(parse_audits(&line).is_err());
        assert!(parse_audits("not json")
            .unwrap_err()
            .to_string()
            .contains("line 1"));
    }

    #[test]
    fn canonical_domain_normalises() {
        assert_eq!(
            canonical_domain(" HTTPS://www.Northwind.Example.test/jobs?x=1 "),
            "northwind.example.test"
        );
        assert_eq!(
            canonical_domain("northwind.example.test."),
            "northwind.example.test"
        );
    }

    #[test]
    fn status_for_bad_timestamp_is_stale_not_panic() {
        let mut rows = sample();
        rows[0].audited_at = "not-a-time".into();
        let t = now("2026-10-27T12:00:00+01:00");
        let (s, a) = status_for(&rows, "northwind.example.test", t);
        assert_eq!(s, GateStatus::Stale);
        assert!(a.is_some());
        // would-pass-on-empty guard: a valid row still gates normally
        assert_ne!(
            status_for(&rows, "fabrikam.example.test", t).0,
            GateStatus::Stale
        );
    }

    #[test]
    fn future_audited_at_reads_stale_not_fresh_forever() {
        let mut rows = sample();
        let t = now("2026-10-27T12:00:00+01:00");
        rows[0].audited_at = "2027-10-27T12:00:00+01:00".into();
        assert_eq!(
            status_for(&rows, "northwind.example.test", t).0,
            GateStatus::Stale
        );
        // a few seconds of clock skew between writers is still fresh
        rows[0].audited_at = "2026-10-27T12:04:00+01:00".into();
        assert_eq!(
            status_for(&rows, "northwind.example.test", t).0,
            GateStatus::Pass
        );
    }

    /// Shared with kanithanj autoapply audit.rs (keep the two lists identical).
    pub const DOMAIN_VECTORS: &[(&str, &str)] = &[
        ("https://x@acme.example/", "acme.example"),
        (
            "https://user:pw@www.Acme.Example:8443/jobs?a=1#f",
            "acme.example",
        ),
        (
            " HTTPS://www.Northwind.Example.test/jobs?x=1 ",
            "northwind.example.test",
        ),
        ("northwind.example.test.", "northwind.example.test"),
        ("jobs.acme.example/apply", "jobs.acme.example"),
    ];

    #[test]
    fn canonical_domain_shared_vectors() {
        assert!(!DOMAIN_VECTORS.is_empty());
        for (i, o) in DOMAIN_VECTORS {
            assert_eq!(canonical_domain(i), *o, "input {i:?}");
        }
    }

    #[test]
    fn last_row_per_domain_wins_and_status_gates() {
        let mut rows = sample();
        let mut later = rows[0].clone();
        later.checks[3].result = CheckResult::Flag;
        later.verdict = Verdict::Flag;
        later.audited_at = "2026-10-25T09:00:00+02:00".into();
        rows.push(later);
        let cur = current_audits(&rows);
        assert_eq!(cur.len(), 3);
        let t = now("2026-10-27T12:00:00+01:00");
        assert_eq!(
            status_for(&rows, "northwind.example.test", t).0,
            GateStatus::Flag
        );
        assert_eq!(
            status_for(&rows, "https://www.NORTHWIND.example.test/x", t).0,
            GateStatus::Flag
        );
        assert_eq!(
            status_for(&rows, "northwind fictional ab", t).0,
            GateStatus::Flag
        );
        assert_eq!(
            status_for(&rows, "fabrikam.example.test", t).0,
            GateStatus::Block
        );
        assert_eq!(
            status_for(&rows, "nobody.example.test", t).0,
            GateStatus::Unaudited
        );
        assert!(status_for(&rows, "nobody.example.test", t).1.is_none());
        // contoso audited 2026-10-21; 31 days later it is stale
        assert_eq!(
            status_for(&rows, "contoso.example.test", t).0,
            GateStatus::Flag
        );
        assert_eq!(
            status_for(
                &rows,
                "contoso.example.test",
                now("2026-11-21T12:00:00+01:00")
            )
            .0,
            GateStatus::Stale
        );
    }

    #[test]
    fn exit_code_contract() {
        assert_eq!(exit_code(GateStatus::Pass), 0);
        assert_eq!(exit_code(GateStatus::Flag), 3);
        assert_eq!(exit_code(GateStatus::Block), 4);
        assert_eq!(exit_code(GateStatus::Unaudited), 5);
        assert_eq!(exit_code(GateStatus::Stale), 5);
    }

    #[test]
    fn exports_are_nonempty_deterministic_and_idempotent() {
        let rows = sample();
        let csv = export_csv(&rows);
        assert!(csv.starts_with("domain,company,verdict,audited_at,audited_by,apply_url,apply_domain,domain_age,funding_ownership,headcount,reviews_news,duplicate_ghost,payment_id_ask,agency_repost\n"), "{csv}");
        assert_eq!(csv.lines().count(), 4);
        assert!(csv.contains("fabrikam.example.test,Fabrikam Example GmbH,block,"));
        let mut rev = rows.clone();
        rev.reverse();
        assert_eq!(csv, export_csv(&rev));
        let md = export_markdown(&rows);
        assert!(
            md.contains("| contoso.example.test | Contoso Sample Ltd | flag |"),
            "{md}"
        );
        let dir = tempfile::tempdir().unwrap();
        write_exports(&rows, dir.path()).unwrap();
        let a = std::fs::read(dir.path().join("company-audits.csv")).unwrap();
        write_exports(&rows, dir.path()).unwrap();
        assert_eq!(
            a,
            std::fs::read(dir.path().join("company-audits.csv")).unwrap()
        );
        assert!(!a.is_empty());
    }

    #[test]
    fn append_after_missing_final_newline_keeps_rows_apart() {
        let dir = tempfile::tempdir().unwrap();
        let mut lines = SAMPLE.lines();
        let first_line = lines.next().unwrap();
        let rest: Vec<&str> = lines.collect();
        let p = dir.path().join("a.jsonl");
        std::fs::write(&p, rest.join("\n")).unwrap();
        append_audit(&p, &serde_json::from_str(first_line).unwrap()).unwrap();
        let n = parse_audits(&std::fs::read_to_string(&p).unwrap())
            .unwrap()
            .len();
        assert_eq!(n, rest.len() + 1);
        let q = dir.path().join("b.jsonl");
        std::fs::write(&q, rest.join("\n")).unwrap();
        assert_eq!(append_audits(&q, first_line).unwrap(), (1, 0));
        let n = parse_audits(&std::fs::read_to_string(&q).unwrap())
            .unwrap()
            .len();
        assert_eq!(n, rest.len() + 1);
    }

    #[test]
    fn append_validates_and_refuses_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.jsonl");
        let a = first();
        append_audit(&p, &a).unwrap();
        assert!(append_audit(&p, &a).is_err());
        let mut bad = a.clone();
        bad.verdict = Verdict::Block;
        assert!(append_audit(&p, &bad).is_err());
        assert_eq!(
            parse_audits(&std::fs::read_to_string(&p).unwrap()).unwrap(),
            vec![a]
        );
    }

    #[test]
    fn batch_append_is_all_or_nothing_and_rerun_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.jsonl");
        assert_eq!(append_audits(&p, SAMPLE).unwrap(), (3, 0));
        assert_eq!(append_audits(&p, SAMPLE).unwrap(), (0, 3));
        let bad = format!("{}\nnot json\n", SAMPLE.lines().next().unwrap());
        let q = dir.path().join("b.jsonl");
        assert!(append_audits(&q, &bad).is_err());
        assert!(!q.exists(), "nothing written when any line is invalid");
        assert_eq!(std::fs::read_to_string(&p).unwrap().lines().count(), 3);
    }

    #[test]
    fn schema_matches_rust() {
        let s: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
        let ids: Vec<&str> = s["$defs"]["check"]["properties"]["id"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            [
                "apply_domain",
                "domain_age",
                "funding_ownership",
                "headcount",
                "reviews_news",
                "duplicate_ghost",
                "payment_id_ask",
                "agency_repost"
            ]
        );
        let mut keys: Vec<_> = s["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        keys.sort();
        let mut a = first();
        a.apply_url
            .get_or_insert_with(|| "https://jobs.example.test/x".into());
        a.reused_from = Some("2026-10-20T09:00:00+02:00".into());
        let full = serde_json::to_value(&a).unwrap();
        let mut rk: Vec<_> = full.as_object().unwrap().keys().cloned().collect();
        rk.sort();
        assert_eq!(keys, rk);
        let re =
            regex::Regex::new(s["properties"]["audited_at"]["pattern"].as_str().unwrap()).unwrap();
        for a in sample() {
            assert!(re.is_match(&a.audited_at));
        }
    }
}
