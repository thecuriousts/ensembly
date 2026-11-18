//! `retrieval-bench` — slice 1a retrieval baseline harness (not shipped in default release).

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use retrieval_bench::harness::run_size;
use retrieval_bench::report::write_outputs;
use retrieval_bench::DEFAULT_SEED;

#[derive(Parser)]
#[command(
    name = "retrieval-bench",
    about = "Slice 1a retrieval benchmark (measure only)"
)]
struct Args {
    /// Output directory for results.json, summary.md, and explain plans (embedded in JSON).
    #[arg(long, default_value = "target/retrieval-bench")]
    out: PathBuf,

    /// Comma-separated corpus sizes (rows in pulse + events in record log).
    #[arg(long, default_value = "1000,10000")]
    sizes: String,

    /// Deterministic RNG seed.
    #[arg(long, default_value_t = DEFAULT_SEED)]
    seed: u64,

    /// Optional git SHA recorded in the report.
    #[arg(long, env = "GITHUB_SHA")]
    git_sha: Option<String>,

    /// Work subdirectory under `out` for generated fixtures (removed only when `--clean`).
    #[arg(long)]
    clean: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.clean && args.out.exists() {
        std::fs::remove_dir_all(&args.out)?;
    }
    std::fs::create_dir_all(&args.out)?;
    let sizes: Vec<usize> = args
        .sizes
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<usize>())
        .collect::<std::result::Result<_, _>>()?;
    let mut reports = Vec::new();
    for size in sizes {
        let work = args.out.join(format!("work-{size}"));
        let rep = run_size(&work, size, args.seed)?;
        eprintln!(
            "size={} unique@10={:.3} nl@10={:.3} fresh-first={:.3}",
            size,
            rep.quality.unique.recall_at_10,
            rep.quality.natural_phrase.recall_at_10,
            rep.quality.fresh_first_accuracy
        );
        reports.push(rep);
    }
    write_outputs(&args.out, &reports, args.git_sha)?;
    eprintln!("wrote {}", args.out.join("results.json").display());
    Ok(())
}
