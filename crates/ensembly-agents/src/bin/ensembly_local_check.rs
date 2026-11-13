//! Opt-in local fact/conflict checks (llama.cpp or MLX) plus fixture replay.
//!
//! Default (no `ENSEMBLY_LOCAL_MODELS=1`): replay recorded synthetic JSON.
//! Live model processes run only when the flag is set. Cloud fallback never
//! receives personal data; the redaction gate refuses those texts.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use ensembly_agents::local_checks::{
    fixture_root_from, live_models_enabled, replay_checker, CheckFailure, CheckSuccess,
    FallbackReason, LocalChecker, RecordedSet, RuntimeKind, FIXTURE_ROOT,
};
use serde_json::{json, Value};
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(
    name = "ensembly-local-check",
    about = "Fact classification and conflict checks (recorded fixtures or opt-in local models)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Classify one statement (stdin or --text).
    Classify {
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        id: Option<String>,
    },
    /// Compare two statements.
    Conflict {
        #[arg(long)]
        left: Option<String>,
        #[arg(long)]
        right: Option<String>,
        #[arg(long)]
        id: Option<String>,
    },
    /// Replay or live-run every recorded fixture and print a summary.
    Fixtures {
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Show runtime, memory, and configured model paths (home path redacted).
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Classify { text, id } => cmd_classify(text, id),
        Cmd::Conflict { left, right, id } => cmd_conflict(left, right, id),
        Cmd::Fixtures { dir } => cmd_fixtures(dir),
        Cmd::Status => cmd_status(),
    }
}

fn cmd_classify(text: Option<String>, id: Option<String>) -> Result<()> {
    if live_models_enabled() {
        let statement = required_text(text)?;
        let checker = LocalChecker::from_env();
        print_result(
            checker
                .classify(&statement)
                .map(serialize_ok)
                .map_err(serialize_err),
        )
    } else {
        let set = load_set(None)?;
        let result = if let Some(id) = id {
            set.classify_by_id(&id)
        } else {
            set.classify(&required_text(text)?)
        };
        print_result(result.map(serialize_ok).map_err(serialize_err))
    }
}

fn cmd_conflict(left: Option<String>, right: Option<String>, id: Option<String>) -> Result<()> {
    if live_models_enabled() {
        let left = left.context("--left is required for a live conflict check")?;
        let right = right.context("--right is required for a live conflict check")?;
        let checker = LocalChecker::from_env();
        print_result(
            checker
                .conflict(&left, &right)
                .map(serialize_ok)
                .map_err(serialize_err),
        )
    } else {
        let set = load_set(None)?;
        let result = if let Some(id) = id {
            set.conflict_by_id(&id)
        } else {
            let left = left.context("--left or --id is required")?;
            let right = right.context("--right is required")?;
            set.conflict(&left, &right)
        };
        print_result(result.map(serialize_ok).map_err(serialize_err))
    }
}

fn cmd_fixtures(dir: Option<PathBuf>) -> Result<()> {
    let root = match dir {
        Some(d) => d,
        None => fixture_root_from(&std::env::current_dir()?),
    };
    let set = RecordedSet::load(&root).map_err(anyhow::Error::msg)?;
    let live = live_models_enabled();
    let checker = if live {
        Some(LocalChecker::from_env())
    } else {
        None
    };
    let started = Instant::now();
    let mut rows = Vec::new();
    let mut json_ok = 0u32;
    let mut total = 0u32;
    for fixture in &set.classify {
        total += 1;
        let result = if let Some(checker) = &checker {
            checker
                .classify(&fixture.statement)
                .map(serialize_ok)
                .map_err(serialize_err)
        } else {
            set.classify_by_id(&fixture.id)
                .map(serialize_ok)
                .map_err(serialize_err)
        };
        let value = flatten(result);
        if value.get("ok") == Some(&json!(true)) {
            json_ok += 1;
        }
        rows.push(json!({ "id": fixture.id, "kind": "classify", "result": value }));
    }
    for fixture in &set.conflict {
        total += 1;
        let result = if let Some(checker) = &checker {
            checker
                .conflict(&fixture.left, &fixture.right)
                .map(serialize_ok)
                .map_err(serialize_err)
        } else {
            set.conflict_by_id(&fixture.id)
                .map(serialize_ok)
                .map_err(serialize_err)
        };
        let value = flatten(result);
        if value.get("ok") == Some(&json!(true)) {
            json_ok += 1;
        }
        rows.push(json!({ "id": fixture.id, "kind": "conflict", "result": value }));
    }
    let report = json!({
        "ok": json_ok == total,
        "mode": if live { "live" } else { "replay" },
        "fixture_root": root.display().to_string(),
        "digest": set.digest().ok(),
        "total": total,
        "json_valid": json_ok,
        "json_validity_rate": if total == 0 { 0.0 } else { json_ok as f64 / total as f64 },
        "elapsed_ms": started.elapsed().as_millis(),
        "rows": rows,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    if json_ok != total {
        bail!("some fixtures failed JSON validation");
    }
    Ok(())
}

fn cmd_status() -> Result<()> {
    let cfg = ensembly_agents::local_checks::LocalCheckConfig::from_env();
    let available = ensembly_agents::local_checks::support::available_bytes_now();
    let report = json!({
        "live_models_enabled": live_models_enabled(),
        "preferred_runtime": cfg.prefer.as_str(),
        "llama_bin": cfg.llama_bin.as_ref().map(|p| ensembly_agents::local_checks::runtime::display_model(p)),
        "mlx_bin": cfg.mlx_bin.as_ref().map(|p| ensembly_agents::local_checks::runtime::display_model(p)),
        "classify_model": cfg.classify_model.as_ref().map(|p| ensembly_agents::local_checks::runtime::display_model(p)),
        "conflict_model": cfg.conflict_model.as_ref().map(|p| ensembly_agents::local_checks::runtime::display_model(p)),
        "cloud_configured": cfg.cloud_cmd.is_some(),
        "confidence_floor": cfg.confidence_floor,
        "available_bytes": available,
        "fixture_root": FIXTURE_ROOT,
        "host_alias": "mac-mini",
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn load_set(dir: Option<PathBuf>) -> Result<RecordedSet> {
    let root = dir.unwrap_or_else(|| fixture_root_from(&std::env::current_dir().unwrap()));
    replay_checker(&root).map_err(anyhow::Error::msg)
}

fn required_text(text: Option<String>) -> Result<String> {
    if let Some(t) = text {
        return Ok(t);
    }
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    if buf.trim().is_empty() {
        bail!("pass --text or pipe a statement on stdin");
    }
    Ok(buf)
}

fn serialize_ok<T: serde::Serialize>(ok: CheckSuccess<T>) -> Value {
    json!({
        "ok": true,
        "value": ok.value,
        "runtime": ok.source.runtime.as_str(),
        "model": ok.source.model,
        "fallback": ok.source.fallback.map(FallbackReason::as_str),
        "latency_ms": ok.latency.as_millis(),
        "raw": ok.raw,
    })
}

fn serialize_err(err: CheckFailure) -> Value {
    json!({
        "ok": false,
        "reason": err.reason.as_str(),
        "detail": err.detail,
        "latency_ms": err.latency.as_millis(),
        "raw": err.raw,
    })
}

fn flatten(result: Result<Value, Value>) -> Value {
    match result {
        Ok(value) | Err(value) => value,
    }
}

fn print_result(result: Result<Value, Value>) -> Result<()> {
    let value = flatten(result);
    println!("{}", serde_json::to_string_pretty(&value)?);
    if value.get("ok") == Some(&json!(false)) {
        bail!("local check failed");
    }
    Ok(())
}

// Silence unused import in some rustc versions when Replay is only in docs.
#[allow(dead_code)]
fn _runtime_kinds() -> [RuntimeKind; 4] {
    [
        RuntimeKind::LlamaCpp,
        RuntimeKind::Mlx,
        RuntimeKind::Cloud,
        RuntimeKind::Replay,
    ]
}
