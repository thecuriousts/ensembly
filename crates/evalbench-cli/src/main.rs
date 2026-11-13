//! `evalbench` — hash, check, score, record, run. Exit 0 ok, 1 finding, 2 error.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::{ArgGroup, Parser, Subcommand};
use evalbench_cli::{load_task_set, CommandClient};
use evalbench_core::check::{gaps, parse_pack, parse_posting};
use evalbench_core::runner::{
    run_set_with, trace_for, ClientError, ModelClient, ModelReply, PulseContext, ReplayClient,
};
use evalbench_core::score::{score, select_run};
use evalbench_core::trace::{parse_traces, valid_run_id, TraceRow};

#[derive(Parser)]
#[command(
    name = "evalbench",
    version,
    about = "evalbench E1: frozen task set, gap check, first-pass score"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print the frozen task-set hash; with --verify, exit 1 if it differs from the file.
    Hash {
        dir: PathBuf,
        #[arg(long)]
        verify: Option<PathBuf>,
    },
    /// Gap-check one posting against one pack. Exit 1 when there are gaps.
    Check { posting: PathBuf, pack: PathBuf },
    /// Score recorded traces against the task set (first attempts only).
    Score {
        #[arg(long)]
        tasks: PathBuf,
        #[arg(long)]
        run_id: Option<String>,
        traces: PathBuf,
    },
    /// Append one trace row. The frozen checker judges the answer; there is no --pass.
    Record {
        #[arg(long)]
        tasks: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        task_id: String,
        #[arg(long)]
        attempt: u32,
        #[arg(long)]
        model_id: String,
        #[arg(long)]
        tokens_in: u64,
        #[arg(long)]
        tokens_out: u64,
        #[arg(long)]
        cost_micro_usd: u64,
        /// File holding the model's raw answer text.
        #[arg(long)]
        answer: PathBuf,
    },
    /// Export pack (ensembly-export-v1): the record view of the traces (task, result,
    /// tokens, model, score, dataset hash, commit) and the canonical pulse pack, if any.
    /// The pulse store is opened read-only. Refuses an existing --out.
    #[command(group(ArgGroup::new("pulse").args(["pulse_pack", "pulse_db"])))]
    Export {
        #[arg(long)]
        tasks: PathBuf,
        /// Trace files (JSONL); repeat for more. Each run id becomes one run.
        #[arg(long = "traces", required = true)]
        traces: Vec<PathBuf>,
        /// Commit the traces were recorded at (40 hex).
        #[arg(long)]
        commit: String,
        #[arg(long)]
        pulse_pack: Option<PathBuf>,
        #[arg(long, requires_all = ["exported_at", "host"])]
        pulse_db: Option<PathBuf>,
        #[arg(long)]
        exported_at: Option<String>,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run every task once (attempt 1) and write traces. Refuses an existing --out.
    #[command(group(ArgGroup::new("client").required(true).args(["replay", "exec"])))]
    Run {
        #[arg(long)]
        tasks: PathBuf,
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        out: PathBuf,
        /// Recorded replies (JSONL of ModelReply).
        #[arg(long)]
        replay: Option<PathBuf>,
        /// Wrapper program: prompt on stdin, EVALBENCH_TASK_ID in env, one ModelReply on stdout.
        #[arg(long)]
        exec: Option<PathBuf>,
        /// Argument for the wrapper; repeat for more.
        #[arg(long = "arg", allow_hyphen_values = true, requires = "exec")]
        args: Vec<String>,
        #[arg(long, default_value_t = 120, requires = "exec")]
        timeout_secs: u64,
        /// Where to write the wrapper's replies (JSONL), for later replay.
        #[arg(long, requires = "exec")]
        replies_out: Option<PathBuf>,
        /// E5 treatment arm: append this pulse pack (canonical pulse-pack-v1 bytes) to
        /// every prompt. Traces get prompt id `gaps-v1+pulse`.
        #[arg(long)]
        pulse_pack: Option<PathBuf>,
    },
}

/// A finding (exit 1), as opposed to an error (exit 2).
struct Finding(String);

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli.cmd) {
        Ok(None) => ExitCode::SUCCESS,
        Ok(Some(Finding(msg))) => {
            if !msg.is_empty() {
                eprintln!("{msg}");
            }
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("evalbench: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn read_text(p: &Path) -> Result<String> {
    fs::read_to_string(p).with_context(|| format!("read {}", p.display()))
}

fn write_new(p: &Path, body: &str) -> Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(p)
        .with_context(|| format!("refusing to write {} (exists or not writable)", p.display()))?;
    f.write_all(body.as_bytes())
        .with_context(|| format!("write {}", p.display()))
}

fn dispatch(cmd: Cmd) -> Result<Option<Finding>> {
    match cmd {
        Cmd::Hash { dir, verify } => {
            let set = load_task_set(&dir).map_err(|e| anyhow!("{e}"))?;
            println!("{}", set.hash);
            if let Some(file) = verify {
                let want = read_text(&file)?;
                let want = want.strip_suffix('\n').unwrap_or(&want);
                if want != set.hash {
                    return Ok(Some(Finding(format!(
                        "hash mismatch: {} holds {want}, tasks give {}",
                        file.display(),
                        set.hash
                    ))));
                }
            }
            Ok(None)
        }
        Cmd::Check { posting, pack } => {
            let po = parse_posting(&read_text(&posting)?)
                .with_context(|| format!("parse {}", posting.display()))?;
            let pa = parse_pack(&read_text(&pack)?)
                .with_context(|| format!("parse {}", pack.display()))?;
            let g = gaps(&po, &pa);
            println!("{}", serde_json::json!({ "gaps": g }));
            Ok((!g.is_empty()).then(|| Finding(String::new())))
        }
        Cmd::Score {
            tasks,
            run_id,
            traces,
        } => {
            let set = load_task_set(&tasks).map_err(|e| anyhow!("{e}"))?;
            let mut rows = parse_traces(&read_text(&traces)?)?;
            if let Some(id) = run_id {
                rows = select_run(&rows, &id);
            }
            let s = score(&set.hash, &set.ids(), &rows)?;
            println!("{}", s.to_json());
            Ok(None)
        }
        Cmd::Record {
            tasks,
            out,
            run_id,
            task_id,
            attempt,
            model_id,
            tokens_in,
            tokens_out,
            cost_micro_usd,
            answer,
        } => {
            let set = load_task_set(&tasks).map_err(|e| anyhow!("{e}"))?;
            let task = set
                .tasks
                .iter()
                .find(|t| t.id == task_id)
                .ok_or_else(|| anyhow!("unknown task: {task_id}"))?;
            let reply = ModelReply {
                task_id: task_id.clone(),
                model_id,
                text: read_text(&answer)?,
                tokens_in,
                tokens_out,
                cost_micro_usd,
            };
            let row = trace_for(task, &set.hash, &run_id, attempt, &reply);
            row.validate()
                .map_err(|k| anyhow!("invalid trace row: {k:?}"))?;
            if out.exists() {
                let existing = parse_traces(&read_text(&out)?)?;
                if existing.iter().any(|r| {
                    r.run_id == row.run_id && r.task_id == row.task_id && r.attempt == row.attempt
                }) {
                    bail!(
                        "{} already holds run {} task {} attempt {}",
                        out.display(),
                        row.run_id,
                        row.task_id,
                        row.attempt
                    );
                }
            }
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&out)
                .with_context(|| format!("open {}", out.display()))?;
            f.write_all(row.to_line().as_bytes())
                .with_context(|| format!("append {}", out.display()))?;
            Ok(None)
        }
        Cmd::Export {
            tasks,
            traces,
            commit,
            pulse_pack,
            pulse_db,
            exported_at,
            host,
            out,
        } => {
            use evalbench_cli::export::{build, to_bytes, PulseSource};
            if out.exists() {
                bail!("refusing to overwrite {}", out.display());
            }
            let set = load_task_set(&tasks).map_err(|e| anyhow!("{e}"))?;
            let mut rows = Vec::new();
            for t in &traces {
                rows.extend(parse_traces(&read_text(t)?)?);
            }
            let pack_bytes = pulse_pack
                .as_ref()
                .map(|p| fs::read(p).with_context(|| format!("read {}", p.display())))
                .transpose()?;
            let src = match (&pack_bytes, &pulse_db) {
                (Some(b), _) => PulseSource::Pack(b),
                (None, Some(db)) => PulseSource::Store {
                    path: db,
                    exported_at: exported_at.as_deref().expect("clap requires it"),
                    host: host.as_deref().expect("clap requires it"),
                },
                (None, None) => PulseSource::None,
            };
            let doc = build(&set, &rows, &commit, src)?;
            let bytes = to_bytes(&doc);
            write_new(&out, std::str::from_utf8(&bytes).expect("canonical JSON is UTF-8"))?;
            Ok(None)
        }
        Cmd::Run {
            tasks,
            run_id,
            out,
            replay,
            exec,
            args,
            timeout_secs,
            replies_out,
            pulse_pack,
        } => {
            if !valid_run_id(&run_id) {
                bail!("bad run id: {run_id:?}");
            }
            for p in std::iter::once(&out).chain(replies_out.as_ref()) {
                if p.exists() {
                    bail!("refusing to overwrite {}", p.display());
                }
            }
            let set = load_task_set(&tasks).map_err(|e| anyhow!("{e}"))?;
            let pulse = pulse_pack
                .map(|p| read_text(&p).map(|text| PulseContext { text }))
                .transpose()?;
            let pulse = pulse.as_ref();
            let (rows, replies) = if let Some(file) = replay {
                let mut client = ReplayClient::from_jsonl(&read_text(&file)?)?;
                (run_set_with(&mut client, &set, &run_id, pulse)?, Vec::new())
            } else {
                let program = exec.expect("clap requires --replay or --exec");
                let mut client = Recording {
                    inner: CommandClient {
                        program,
                        args,
                        timeout: Duration::from_secs(timeout_secs),
                    },
                    replies: Vec::new(),
                };
                let rows = run_set_with(&mut client, &set, &run_id, pulse)?;
                (rows, client.replies)
            };
            let body: String = rows.iter().map(TraceRow::to_line).collect();
            write_new(&out, &body)?;
            if let Some(p) = replies_out {
                let mut text = String::new();
                for r in &replies {
                    text.push_str(&serde_json::to_string(r)?);
                    text.push('\n');
                }
                write_new(&p, &text)?;
            }
            Ok(None)
        }
    }
}

/// Keeps every reply so a live run can be replayed later.
struct Recording<C> {
    inner: C,
    replies: Vec<ModelReply>,
}

impl<C: ModelClient> ModelClient for Recording<C> {
    fn complete(&mut self, task_id: &str, prompt: &str) -> Result<ModelReply, ClientError> {
        let reply = self.inner.complete(task_id, prompt)?;
        self.replies.push(reply.clone());
        Ok(reply)
    }
}
