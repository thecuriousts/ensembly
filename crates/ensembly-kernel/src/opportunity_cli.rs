//! Opportunity-finder CLI (`apps` and `record apps`).

use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum RecordAppsCmd {
    /// Append one record event per current ledger row. Does not write the ledger.
    Import {
        #[arg(long)]
        ledger: PathBuf,
        #[arg(long)]
        log: PathBuf,
        /// Writer identity; must match the configured ENSEMBLY_WRITER value.
        #[arg(long)]
        writer: String,
        /// Local RFC 3339 time when a row has no submit time. Marked synthetic.
        #[arg(long)]
        at: Option<String>,
    },
    /// Replay the record log and diff it against the ledger. Writes nothing.
    /// Exit 1 when a link differs. Exit 2 when the ledger or the log cannot be read.
    Compare {
        #[arg(long)]
        ledger: PathBuf,
        #[arg(long)]
        log: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum AppsCmd {
    /// Company audit store (scam-risk / due-diligence gate before packs and applications)
    Audit {
        /// Audit store JSONL (default data/local/applications/company-audits.jsonl; local-only)
        #[arg(long)]
        store: Option<PathBuf>,
        #[command(subcommand)]
        sub: AuditCmd,
    },
    /// Validate every line; print row count and current-state count
    Validate,
    /// Append one validated row (identical row refused)
    Append {
        #[arg(long)]
        company: String,
        #[arg(long)]
        role: String,
        /// Canonical posting URL (not the form URL)
        #[arg(long)]
        link: String,
        /// Optional application-form URL (e.g. …/application)
        #[arg(long)]
        apply_url: Option<String>,
        /// prepared|submitted|interview|rejected|withdrawn|silent-closed
        #[arg(long)]
        status: String,
        /// RFC 3339 with numeric offset or Z (omit only for prepared)
        #[arg(long)]
        submit_time: Option<String>,
        /// Sent, but the time was never recorded (submit_time stays null)
        #[arg(long, default_value_t = false)]
        submit_time_unknown: bool,
        #[arg(long)]
        cv_version: String,
    },
    /// Write applications.csv + applications.md (byte-identical on re-run), or TRACKER.md with --tracker
    Export {
        /// Output dir (default data/local/applications/exports)
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Write only TRACKER.md (ledger state joined with a side file), atomically
        #[arg(long)]
        tracker: bool,
        /// Side file keyed by canonical link: Posted, Lane, Linear, Trello, Pack (.md hand tracker, .jsonl or .csv)
        #[arg(long, requires = "tracker")]
        join: Option<PathBuf>,
        /// Extra rows for the "Prepared, not submitted" section (.md, .jsonl or .csv)
        #[arg(long, requires = "tracker")]
        prepared: Option<PathBuf>,
        /// Header time "YYYY-MM-DD HH:MM" (default: now, local time)
        #[arg(long, requires = "tracker")]
        stamp: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum AuditCmd {
    /// Validate every line (or a FILE instead of the store); print counts
    Validate {
        /// Validate this JSONL file instead of the store
        file: Option<PathBuf>,
    },
    /// Append audits from a JSONL FILE ('-' = stdin): all lines validated first; rows already stored are skipped
    Append { file: PathBuf },
    /// Write company-audits.csv + .md (byte-identical on re-run)
    Export {
        /// Output dir (default data/local/applications/exports)
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// Print JSON status for a company name or domain; exit 0 pass, 3 = flag (not approvable), 4 block, 5 unaudited or stale
    Status {
        query: String,
        /// Evaluate staleness at this RFC 3339 time instead of now (tests/replays)
        #[arg(long)]
        now: Option<String>,
    },
}
fn read_side(p: Option<&std::path::Path>) -> Result<ensembly_kernel::tracker::Side> {
    use ensembly_kernel::tracker as tr;
    let Some(p) = p else {
        return Ok(tr::Side::default());
    };
    let text =
        std::fs::read_to_string(p).map_err(|e| anyhow::anyhow!("read {}: {e}", p.display()))?;
    tr::parse_side(&text, tr::SideFormat::from_path(p))
        .map_err(|e| anyhow::anyhow!("{}: {e}", p.display()))
}

/// `apps export --tracker`: TRACKER.md into `dir`, replaced atomically.
fn export_tracker(
    rows: &[ensembly_kernel::application_ledger::Row],
    dir: &std::path::Path,
    join: Option<&std::path::Path>,
    prepared: Option<&std::path::Path>,
    stamp: Option<String>,
) -> Result<()> {
    use ensembly_kernel::tracker as tr;
    let side = read_side(join)?;
    let extra = read_side(prepared)?;
    let t = tr::build(rows, &side, &extra.rows);
    let stamp = stamp.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d %H:%M").to_string());
    std::fs::create_dir_all(dir)?;
    tr::write_atomic(
        &dir.join(tr::TRACKER_FILE),
        tr::render(&t, &stamp).as_bytes(),
    )?;
    println!(
        "{}",
        tr::summary(&t, side.rows.len(), tr::current_rows(rows).len())
    );
    Ok(())
}

fn read_file(p: &PathBuf) -> Result<String> {
    std::fs::read_to_string(p).map_err(|e| anyhow::anyhow!("read {}: {e}", p.display()))
}

pub fn cmd_apps(ledger: Option<PathBuf>, sub: AppsCmd) -> Result<()> {
    use ensembly_kernel::application_ledger as al;
    let ledger = ledger.unwrap_or_else(|| PathBuf::from(al::DEFAULT_LEDGER_PATH));
    let read = |p: &PathBuf| -> Result<Vec<al::Row>> { al::parse_ledger(&read_file(p)?) };
    match sub {
        AppsCmd::Audit { store, sub } => cmd_audit(store, sub),
        AppsCmd::Validate => {
            let rows = read(&ledger)?;
            println!(
                "ok: {} rows, {} current applications",
                rows.len(),
                al::current_rows(&rows).len()
            );
            Ok(())
        }
        AppsCmd::Append {
            company,
            role,
            link,
            apply_url,
            status,
            submit_time,
            submit_time_unknown,
            cv_version,
        } => {
            let status: al::Status = serde_json::from_value(serde_json::Value::String(status))
                .map_err(|e| anyhow::anyhow!("status: {e}"))?;
            let row = al::Row {
                company,
                role,
                link,
                apply_url,
                status,
                submit_time,
                submit_time_unknown: submit_time_unknown.then_some(true),
                cv_version,
            };
            let writer = ensembly_kernel::KernelWriter::open_ledger(&ledger)?;
            if !ledger.exists() {
                writer.init_ledger()?;
            }
            let receipt = writer.append_ledger(&row)?;
            if receipt.appended {
                println!("appended: {} ({})", row.link, row.status.as_str());
            } else {
                println!("already present: {} ({})", row.link, row.status.as_str());
            }
            Ok(())
        }
        AppsCmd::Export {
            out_dir,
            tracker: true,
            join,
            prepared,
            stamp,
        } => {
            let rows = read(&ledger)?;
            let dir = out_dir.unwrap_or_else(|| PathBuf::from(al::DEFAULT_EXPORT_DIR));
            export_tracker(&rows, &dir, join.as_deref(), prepared.as_deref(), stamp)
        }
        AppsCmd::Export { out_dir, .. } => {
            let rows = read(&ledger)?;
            let dir = out_dir.unwrap_or_else(|| PathBuf::from(al::DEFAULT_EXPORT_DIR));
            al::write_exports(&rows, &dir)?;
            println!(
                "exported {} applications to {}",
                al::current_rows(&rows).len(),
                dir.display()
            );
            Ok(())
        }
    }
}

fn read_audit_store(store: &PathBuf) -> Result<Vec<ensembly_kernel::company_audit::Audit>> {
    use ensembly_kernel::company_audit as ca;
    match std::fs::read_to_string(store) {
        Ok(t) => ca::parse_audits(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(anyhow::anyhow!("read {}: {e}", store.display())),
    }
}

fn cmd_audit(store: Option<PathBuf>, sub: AuditCmd) -> Result<()> {
    use ensembly_kernel::application_ledger as al;
    use ensembly_kernel::company_audit as ca;
    let store = store.unwrap_or_else(|| PathBuf::from(ca::DEFAULT_AUDIT_PATH));
    match sub {
        AuditCmd::Validate { file } => audit_validate(&store, file),
        AuditCmd::Append { file } => {
            let text = if file.as_os_str() == "-" {
                let mut t = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut t)?;
                t
            } else {
                read_file(&file)?
            };
            let (n, skipped) = ca::append_audits(&store, &text)?;
            println!("appended {n}, skipped {skipped} already stored");
            Ok(())
        }
        AuditCmd::Export { out_dir } => {
            let rows = read_audit_store(&store)?;
            let dir = out_dir.unwrap_or_else(|| PathBuf::from(al::DEFAULT_EXPORT_DIR));
            ca::write_exports(&rows, &dir)?;
            println!(
                "exported {} companies to {}",
                ca::current_audits(&rows).len(),
                dir.display()
            );
            Ok(())
        }
        AuditCmd::Status { query, now } => audit_status(&store, &query, now),
    }
}

fn audit_validate(store: &PathBuf, file: Option<PathBuf>) -> Result<()> {
    use ensembly_kernel::company_audit as ca;
    let rows = match file {
        Some(f) => ca::parse_audits(&read_file(&f)?)?,
        None => read_audit_store(store)?,
    };
    println!(
        "ok: {} audits, {} companies",
        rows.len(),
        ca::current_audits(&rows).len()
    );
    for i in ca::dangling_reuse(&rows) {
        eprintln!("warning: audit {} ({}) has a reused_from that names no earlier stored audit of that domain", i + 1, rows[i].domain);
    }
    Ok(())
}

fn audit_status(store: &PathBuf, query: &str, now: Option<String>) -> Result<()> {
    use ensembly_kernel::company_audit as ca;
    let rows = read_audit_store(store)?;
    let now = match now {
        Some(t) => {
            chrono::DateTime::parse_from_rfc3339(&t).map_err(|e| anyhow::anyhow!("--now: {e}"))?
        }
        None => chrono::Local::now().fixed_offset(),
    };
    let (status, audit) = ca::status_for(&rows, query, now);
    let out = serde_json::json!({
        "query": query,
        "status": status,
        "company": audit.as_ref().map(|a| a.company.clone()),
        "domain": audit.as_ref().map(|a| a.domain.clone()),
        "verdict": audit.as_ref().map(|a| a.effective_verdict()),
        "audited_at": audit.as_ref().map(|a| a.audited_at.clone()),
        "stale_after_days": ca::STALE_AFTER_DAYS,
    });
    println!("{out}");
    std::process::exit(ca::audit_exit_code(status));
}

pub fn cmd_record_apps(cmd: RecordAppsCmd) -> Result<()> {
    match cmd {
        RecordAppsCmd::Import {
            ledger,
            log,
            writer,
            at,
        } => {
            let at = at.unwrap_or_else(ensembly_schema::record_now_local);
            let receipt = ensembly_kernel::import_ledger(&ledger, &log, &writer, &at)
                .map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string(&receipt)?);
            Ok(())
        }
        RecordAppsCmd::Compare { ledger, log } => {
            match ensembly_kernel::compare_files(&ledger, &log) {
                Ok(report) => finish_compare(&report),
                Err(err) => {
                    eprintln!("Error: {err}");
                    std::process::exit(2);
                }
            }
        }
    }
}

pub(crate) fn finish_compare(report: &ensembly_schema::CompareReport) -> Result<()> {
    println!("{}", serde_json::to_string(report)?);
    if !report.mismatches.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
