//! Decision-model CLI (`ensembly decision …`).

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Subcommand;
use ensembly_kernel::KernelWriter;

#[derive(Subcommand)]
pub enum DecisionCmd {
    /// Fit on train and report split metrics. Held-out scores wait for labels.
    Eval {
        #[arg(long)]
        labels: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Write an empty labelling CSV from a posting list. Does not invent labels.
    Template {
        #[arg(long)]
        from: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Append a proposal (or a gate skip) to a sealed decision log.
    Propose {
        #[arg(long)]
        labels: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long)]
        events: PathBuf,
        /// Writer identity; must match the configured ENSEMBLY_WRITER value.
        #[arg(long)]
        writer: String,
        #[arg(long)]
        at: String,
        #[arg(long)]
        deadline: Option<String>,
    },
    Confirm {
        #[arg(long)]
        rid: String,
        #[arg(long)]
        events: PathBuf,
        /// Writer identity; must match the configured ENSEMBLY_WRITER value.
        #[arg(long)]
        writer: String,
        #[arg(long)]
        at: String,
    },
    Override {
        #[arg(long)]
        rid: String,
        #[arg(long)]
        events: PathBuf,
        /// apply or skip
        #[arg(long)]
        label: String,
        /// Writer identity; must match the configured ENSEMBLY_WRITER value.
        #[arg(long)]
        writer: String,
        #[arg(long)]
        at: String,
    },
    /// Record timeout with no action. Silence never becomes apply.
    Expire {
        #[arg(long)]
        rid: String,
        #[arg(long)]
        events: PathBuf,
        /// Writer identity; must match the configured ENSEMBLY_WRITER value.
        #[arg(long)]
        writer: String,
        #[arg(long)]
        at: String,
    },
}

pub fn run(cmd: DecisionCmd) -> Result<()> {
    match cmd {
        DecisionCmd::Eval { labels, json } => cmd_eval(&labels, json),
        DecisionCmd::Template { from, out } => cmd_template(&from, &out),
        DecisionCmd::Propose {
            labels,
            id,
            events,
            writer,
            at,
            deadline,
        } => cmd_propose(&labels, &id, &events, &writer, &at, deadline.as_deref()),
        DecisionCmd::Confirm {
            rid,
            events,
            writer,
            at,
        } => {
            let w = open_decision_log(&events)?;
            let receipt = w.confirm_decision(&rid, &writer, &at)?;
            println!(
                "rid={} type={} appended={}",
                receipt.rid,
                receipt.event_type.as_str(),
                receipt.appended
            );
            Ok(())
        }
        DecisionCmd::Override {
            rid,
            events,
            label,
            writer,
            at,
        } => {
            let label = parse_override(&label)?;
            let w = open_decision_log(&events)?;
            let receipt = w.override_decision(&rid, &writer, &at, label)?;
            println!(
                "rid={} type={} appended={}",
                receipt.rid,
                receipt.event_type.as_str(),
                receipt.appended
            );
            Ok(())
        }
        DecisionCmd::Expire {
            rid,
            events,
            writer,
            at,
        } => {
            let w = open_decision_log(&events)?;
            let receipt = w.expire_decision(&rid, &writer, &at)?;
            println!(
                "rid={} type={} appended={}",
                receipt.rid,
                receipt.event_type.as_str(),
                receipt.appended
            );
            Ok(())
        }
    }
}

fn open_decision_log(path: &Path) -> Result<KernelWriter> {
    let w = KernelWriter::open_decisions(path)?;
    if !path.exists() {
        w.init_decisions()?;
    }
    Ok(w)
}

fn parse_override(label: &str) -> Result<ensembly_schema::ModelDecision> {
    match label {
        "apply" => Ok(ensembly_schema::ModelDecision::Apply),
        "skip" => Ok(ensembly_schema::ModelDecision::Skip),
        _ => anyhow::bail!("override label must be apply or skip"),
    }
}

fn cmd_eval(labels: &Path, json: bool) -> Result<()> {
    use ensembly_kernel::decision::{load_labels, run_eval, FlagList};
    let rows = load_labels(labels)?;
    let report = run_eval(&rows, &FlagList::default(), 0.90)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!("{}", report.claim);
    println!("triage: {}", report.triage_question);
    println!(
        "trained_on={} t_skip={:.3} t_apply={:.3}",
        report.trained_on, report.band.t_skip, report.band.t_apply
    );
    for split in &report.splits {
        print!(
            "{} n={} labelled={}",
            split.bucket, split.n, split.n_labelled
        );
        if let Some(m) = &split.model {
            if let (Some(p), Some(a)) = (m.apply_precision, m.accuracy_answered) {
                print!(" precision={p:.3} accuracy={a:.3}");
                if let Some(w) = &m.apply_precision_wilson {
                    print!(" precision_wilson=[{:.3},{:.3}] n={}", w.low, w.high, w.n);
                }
            }
        }
        if let Some(note) = &split.note {
            print!(" ({note})");
        }
        println!();
    }
    Ok(())
}

fn cmd_template(from: &Path, out: &Path) -> Result<()> {
    use ensembly_kernel::decision::load_labels;
    let rows = load_labels(from)?;
    let mut body = String::from(
        "id,link_key,company,role,location,url,posting_text,split,decision,reason_tag,outcome,provenance\n",
    );
    for row in rows {
        let loc = row.location.clone().unwrap_or_default();
        let fields = [
            row.id.as_str(),
            row.link_key.as_str(),
            row.company.as_str(),
            row.role.as_str(),
            loc.as_str(),
            row.url.as_str(),
            row.posting_text.as_str(),
            row.split().as_str(),
            "",
            "",
            "",
            "operator-label",
        ];
        body.push_str(
            &fields
                .into_iter()
                .map(csv_field)
                .collect::<Vec<_>>()
                .join(","),
        );
        body.push('\n');
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(out, body)?;
    println!("wrote empty labelling sheet to {}", out.display());
    Ok(())
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn cmd_propose(
    labels: &Path,
    id: &str,
    events: &Path,
    writer: &str,
    at: &str,
    deadline: Option<&str>,
) -> Result<()> {
    use ensembly_kernel::decision::{fit, load_labels, score, FlagList};
    use ensembly_schema::SplitBucket;
    let rows = load_labels(labels)?;
    let row = rows
        .iter()
        .find(|r| r.id == id)
        .ok_or_else(|| anyhow::anyhow!("no posting with id {id}"))?;
    let train: Vec<&_> = rows
        .iter()
        .filter(|r| r.split() == SplitBucket::Train && r.decision.is_some())
        .collect();
    let model = fit(&train, &FlagList::default(), 0.90).map_err(|e| anyhow::anyhow!(e))?;
    let scored = score(&model, row, &FlagList::default());
    let w = open_decision_log(events)?;
    let receipt = w.propose_decision(row, &scored, writer, at, deadline)?;
    println!(
        "rid={} type={} appended={} decision={}",
        receipt.rid,
        receipt.event_type.as_str(),
        receipt.appended,
        scored.decision.as_str()
    );
    Ok(())
}
