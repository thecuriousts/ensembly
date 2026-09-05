//! `ensembly` CLI — dogfood entry for the operator kernel.
//! Compat alias: `peram` (one release; same binary).

use anyhow::{bail, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use ensembly_kernel::approvals::{list_pending, Snapshot};
use ensembly_kernel::backup::{
    create_backup_pack, read_backup_pack, restore_apply, restore_dry_run, write_backup_pack,
};
use ensembly_kernel::digital_flow::{run_cycle, DigitalFlow};
use ensembly_kernel::memory_sink::MemorySink;
use ensembly_kernel::paths::{
    env_alias, migrate_local_paths_at, resolve_memory_path, resolve_ops_db, DEFAULT_MEMORY_PATH,
};
use ensembly_kernel::pulse_pack::{
    export_pulse_pack, import_pulse_pack, local_pulse_status, read_pulse_pack, write_pulse_pack,
    PulseExportOpts, PulseImportOpts, DEFAULT_ARCHIVE_SIDECAR,
};
use ensembly_kernel::msg_bus::ManualCmd;
use ensembly_kernel::runtime::Runtime;
use ensembly_kernel::store::OpsStore;
use ensembly_kernel::channel_pulse::{
    project_wait_snapshot, reconcile_channel_pulse, DEFAULT_CHANNEL_PULSE_PATH,
};
use ensembly_kernel::turn::{
    actions_from_fixture_path, build_channel_ir, context_at, rank_now, Action,
};
use ensembly_kernel::{kernel_version, private_path_patterns};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = env!("CARGO_BIN_NAME"), about = "ensembly operator kernel (Rust)")]
struct Cli {
    /// Ops SQLite path (T1). Fresh default: data/local/ensembly-ops.sqlite (legacy peram-ops.sqlite discovered if present).
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    /// Episodic memory path. Fresh default: data/local/ensembly-memory.json (legacy peram-memory.json discovered if present).
    #[arg(long, global = true)]
    memory: Option<PathBuf>,

    /// Disable episodic memory recording for this invocation.
    #[arg(long, global = true, default_value_t = false)]
    no_memory: bool,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Print kernel version + law banner
    Version,
    /// Build FocusPlan (next body + next auth) from fixture actions + durable snapshot
    Turn {
        /// JSON fixture path with extra_candidates (state-sample shape)
        #[arg(long)]
        fixture: Option<PathBuf>,
        #[arg(long)]
        json: bool,
        /// Emit versioned channel IR (one body + one gate); no stderr banner
        #[arg(long)]
        channel: bool,
        /// location_label: home|travel|office
        #[arg(long)]
        location: Option<String>,
    },
    /// Approve a pending authorization id
    Approve {
        id: String,
    },
    /// Deny a pending authorization id
    Deny {
        id: String,
    },
    /// Claim a physical action id
    Claim {
        id: String,
    },
    /// Complete a physical action id
    Complete {
        id: String,
    },
    /// Digital-flow bill_pay cycle (HITL dry-run)
    #[command(name = "digital-flow")]
    DigitalFlow {
        #[command(subcommand)]
        sub: DfCmd,
    },
    /// Create sealed backup pack of T1 ops (uses OpsBundle inside BackupPack)
    Backup {
        #[arg(long)]
        out: PathBuf,
        /// Unlock material (demo CLI; production → keyring). Env ENSEMBLY_UNLOCK / PERAM_UNLOCK overrides.
        #[arg(long)]
        unlock: Option<String>,
    },
    /// Restore dry-run (does not write primary DB)
    #[command(name = "restore-dry-run")]
    RestoreDryRun {
        #[arg(long)]
        pack: PathBuf,
        #[arg(long)]
        unlock: Option<String>,
    },
    /// Apply sealed backup pack into primary ops DB (canonical host only)
    #[command(name = "restore-apply")]
    RestoreApply {
        #[arg(long)]
        pack: PathBuf,
        #[arg(long)]
        unlock: Option<String>,
        /// Required — destructive write to the ops sqlite
        #[arg(long)]
        i_understand: bool,
    },
    /// Unsealed T1 ops bundle export/import (ensembly-ops-bundle-v1 — not pulse)
    #[command(name = "ops-bundle")]
    OpsBundle {
        #[command(subcommand)]
        sub: OpsBundleCmd,
    },
    /// Portable pulse + memory sync (laptop ↔ bot; no dual-writer ops)
    #[command(name = "pulse-pack")]
    PulsePack {
        #[command(subcommand)]
        sub: PulsePackCmd,
    },
    /// Issue #1 control plane: S+G+CP, MsgBus, HITL/HOOTL tick
    Runtime {
        #[command(subcommand)]
        sub: RuntimeCmd,
    },
    /// Redacted channel pulse (Issue #8) — observation only, never writes G
    #[command(name = "channel-pulse")]
    ChannelPulse {
        #[command(subcommand)]
        sub: ChannelPulseCmd,
    },
    /// One-shot copy-if-missing: peram-* local files → ensembly-* (never overwrite, never delete)
    #[command(name = "migrate-local-paths")]
    MigrateLocalPaths {
        /// Print planned copies without writing
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Root for relative data/local paths (default: cwd)
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum OpsBundleCmd {
    /// Export unsealed ops bundle JSON from the ops sqlite
    Export {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Import ops bundle (canonical host / disaster recovery — not laptop sync)
    Import {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum PulsePackCmd {
    /// Export episodic memory + optional audit archive slice to a portable pack
    Export {
        #[arg(long)]
        out: PathBuf,
        /// Include audit archive slice from ops DB (read-only export)
        #[arg(long, default_value_t = false)]
        include_archive: bool,
        /// Max audit rows when --include-archive (default 200)
        #[arg(long, default_value_t = 200)]
        archive_limit: usize,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Import/merge a pack into local memory (CRDT merge; archive → sidecar JSONL)
    Import {
        #[arg(long)]
        pack: PathBuf,
        /// Validate only — do not write local stores
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Archive sidecar path (default data/local/pulse-archive.jsonl)
        #[arg(long)]
        archive_sidecar: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Print pack or local pulse status (counts, last_seen, hash)
    Status {
        /// Inspect a pack file instead of local stores
        #[arg(long)]
        pack: Option<PathBuf>,
        #[arg(long)]
        archive_sidecar: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum RuntimeCmd {
    /// Load fixture actions into life-state graph, compute CP, persist
    Load {
        #[arg(long)]
        fixture: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Show life-state + critical path (+ optional Monte Carlo already in state)
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Drain MsgBus + optional HOOTL agent claim/complete on CP
    Tick {
        /// Run one HOOTL digital claim+complete if work available
        #[arg(long, default_value_t = true)]
        agent: bool,
        #[arg(long)]
        json: bool,
    },
    /// Enqueue approve (auth gate) via MsgBus then tick
    Approve {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Enqueue deny via MsgBus then tick
    Deny {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Claim physical beacon via MsgBus
    Claim {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Complete physical beacon via MsgBus
    Complete {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Reflection pass over episodic memory: coherence, skill synthesis, goal proposals
    Reflect {
        #[arg(long)]
        json: bool,
    },
    /// UncertaintyDive — Prior→Probe→Simulate→Score→ActOrAsk (inspect only; no mutate)
    Dive {
        #[arg(long, default_value_t = ensembly_kernel::DEFAULT_PROBE_BUDGET)]
        probe_budget: u32,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum DfCmd {
    /// activate → approve → dry-run execute
    Cycle {
        #[arg(long, default_value = "monthly bill")]
        payee: String,
        #[arg(long)]
        json: bool,
    },
    Status {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ChannelPulseCmd {
    /// Weekday reconcile: diff wait-snapshot vs last pulse; write when changed (quiet when not)
    #[command(after_help = "\
Examples:
  cargo run -p ensembly-kernel -- --db /tmp/peram-ops-smoke.sqlite channel-pulse reconcile \\
    --fixture fixtures/issue-1-runtime.json --out /tmp/channel-pulse.json --json
  cargo run -p ensembly-kernel -- --db /tmp/peram-ops-smoke.sqlite channel-pulse reconcile \\
    --fixture fixtures/issue-1-runtime.json --out /tmp/channel-pulse.json
    # unchanged → exit 0, silent
")]
    Reconcile {
        /// Redacted pulse output path (default data/local/channel-pulse.json)
        #[arg(long)]
        out: Option<PathBuf>,
        /// JSON fixture for actions when no wait-snapshot is in the ops DB
        #[arg(long)]
        fixture: Option<PathBuf>,
        /// location_label: home|travel|office
        #[arg(long)]
        location: Option<String>,
        /// Print reconcile report JSON even when unchanged
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Print a one-line status when pulse was updated
        #[arg(long, default_value_t = false)]
        verbose: bool,
    },
}

/// Memory flags lifted out of Cli before `match cli.cmd` moves the subcommand.
struct MemoryFlags {
    memory: Option<PathBuf>,
    no_memory: bool,
}

/// Attach episodic memory to a runtime. Explicit `--memory <path>` open
/// failure is fatal (operator asked for it); default-path failure warns on
/// stderr and continues — memory is aux, never a control dependency.
fn attach_memory(rt: &mut Runtime, flags: &MemoryFlags) {
    if flags.no_memory {
        return;
    }
    let explicit = flags.memory.is_some();
    let path = resolve_memory_path(flags.memory.clone());
    match MemorySink::open(&path) {
        Ok(sink) => rt.memory = Some(sink),
        Err(e) => {
            if explicit {
                eprintln!("MEMORY_FAIL open {path:?}: {e}");
                std::process::exit(2);
            }
            eprintln!("MEMORY_WARN open {path:?} failed: {e} — continuing without memory");
        }
    }
}

/// Persist memory after a mutating command. The control op already committed
/// to T1; a memory failure warns loudly but does not rewrite that truth.
fn save_memory(rt: &mut Runtime) {
    let Some(sink) = rt.memory.as_mut() else {
        return;
    };
    if let Err(e) = sink.sync_and_save() {
        eprintln!("MEMORY_WARN save failed: {e} — trajectory not persisted");
    }
}

fn unlock_material(cli: &Option<String>) -> Result<Vec<u8>> {
    if let Some(v) = env_alias("ENSEMBLY_UNLOCK", "PERAM_UNLOCK") {
        return Ok(v.into_bytes());
    }
    if let Some(u) = cli {
        return Ok(u.clone().into_bytes());
    }
    bail!("unlock required: pass --unlock or set ENSEMBLY_UNLOCK (alias PERAM_UNLOCK; keyring later)");
}

fn load_actions_from_fixture(path: &PathBuf) -> Result<Vec<Action>> {
    actions_from_fixture_path(path).map_err(|e| anyhow::anyhow!("{e}"))
}

fn ensure_snap(store: &OpsStore, actions: &[Action]) -> Result<Snapshot> {
    let now = Utc::now();
    let existing = store.load_snapshot()?;
    let snap = project_wait_snapshot(actions, existing, now);
    store.save_snapshot(&snap)?;
    Ok(snap)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = resolve_ops_db(cli.db.clone());
    let memory_flags = MemoryFlags {
        memory: cli.memory.clone(),
        no_memory: cli.no_memory,
    };

    match cli.cmd {
        Commands::Version => {
            println!("{}", kernel_version());
            println!("private_paths: {:?}", private_path_patterns());
            println!("law: Node src/* legacy; ensembly-kernel is control SoT");
        }
        Commands::MigrateLocalPaths {
            dry_run,
            root,
            json,
        } => {
            let root = root.unwrap_or_else(|| PathBuf::from("."));
            let report = migrate_local_paths_at(&root, dry_run)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                if report.copied.is_empty() && report.skipped_dest_exists.is_empty() {
                    println!("MIGRATE_LOCAL no legacy peram-* files to copy under {}", root.display());
                }
                for a in &report.copied {
                    let verb = if report.dry_run { "would-copy" } else { "copied" };
                    println!("MIGRATE_LOCAL {verb} {} → {}", a.from.display(), a.to.display());
                }
                for a in &report.skipped_dest_exists {
                    println!(
                        "MIGRATE_LOCAL skip-dest-exists {} (left {})",
                        a.to.display(),
                        a.from.display()
                    );
                }
                println!(
                    "MIGRATE_LOCAL_OK copied={} skipped_dest={} skipped_missing={} dry_run={}",
                    report.copied.len(),
                    report.skipped_dest_exists.len(),
                    report.skipped_src_missing.len(),
                    report.dry_run
                );
                println!(
                    "next (canonical host only): smoke runtime status on the new ops path, then\n  cargo run -p ensembly-kernel -- pulse-pack export --out ~/sync/pulse/bot.pulse.json --include-archive\n  # laptop: pulse-pack import that pack (defaults prefer ensembly-memory.json once present)\nKeep legacy copies until smoke passes. Do not dual-write ops. After verify, you may delete peram-* files."
                );
            }
        }
        Commands::Turn {
            fixture,
            json,
            channel,
            location,
        } => {
            let store = OpsStore::open(&db_path)?;
            let actions = if let Some(f) = fixture {
                load_actions_from_fixture(&f)?
            } else {
                // minimal defaults if no fixture
                vec![
                    Action {
                        id: "healthy-self-energy".into(),
                        title: "Healthy Self Energy foundation".into(),
                        area: Some("Health".into()),
                        kind: Some("health_body".into()),
                        realm: Some("physical".into()),
                        urgency: 3,
                        importance: 4,
                        tags: vec!["physical".into()],
                        public: Some(false),
                        depends_on: None,
                        deadline_at: None,
                    },
                ]
            };
            let snap = ensure_snap(&store, &actions)?;
            let mut plan = rank_now(
                &context_at(Utc::now(), location.as_deref()),
                &actions,
                &[],
                &snap,
            );
            // When durable life-state exists, drive FocusPlan from CP (read-only; no persist).
            if let Ok(Some(life)) = store.load_life_state() {
                let now = Utc::now();
                let mut rt = Runtime::new(now);
                rt.state = life;
                rt.snapshot = snap.clone();
                plan = rt.focus_plan(plan);
            } else if ensembly_kernel::DepGraph::from_actions(&actions, &Default::default()).is_ok() {
                let now = Utc::now();
                let mut rt = Runtime::new(now);
                if rt.load_actions(&actions, now).is_ok() {
                    plan = rt.focus_plan(plan);
                    // Fail loud — dual SoT must not partially persist.
                    store.save_runtime_pair(&rt.state, &rt.snapshot)?;
                }
            }
            if channel {
                let ir = build_channel_ir(&plan, &snap, Utc::now());
                println!("{}", serde_json::to_string_pretty(&ir)?);
            } else if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!("# FocusPlan — {}", plan.at.to_rfc3339());
                println!("biome: {}", plan.biome);
                println!("location: {:?}", plan.location_label);
                println!(
                    "next physical: {}",
                    plan.primary_physical
                        .as_ref()
                        .map(|p| format!("{} ({})", p.title, p.id))
                        .unwrap_or_else(|| "—".into())
                );
                println!(
                    "next auth: {}",
                    plan.primary_auth
                        .as_ref()
                        .map(|p| format!("{} ({})", p.title, p.id))
                        .unwrap_or_else(|| "—".into())
                );
                println!("places: {:?}", plan.places);
                println!("coach: {}", plan.coach_line);
                println!("db: {:?}", store.path());
            }
            if !channel {
                eprintln!(
                    "TURN_OK physical={} pending={} nextPhysical={} nextAuth={} biome={}",
                    plan.physical_count,
                    plan.pending_count,
                    plan.primary_physical
                        .as_ref()
                        .map(|p| p.id.as_str())
                        .unwrap_or("-"),
                    plan.primary_auth
                        .as_ref()
                        .map(|p| p.id.as_str())
                        .unwrap_or("-"),
                    plan.biome
                );
            }
        }
        Commands::Approve { id } => {
            gate_via_runtime(
                &db_path,
                &memory_flags,
                ManualCmd::Approve { id: id.clone() },
                "approve",
                &id,
            )?;
        }
        Commands::Deny { id } => {
            gate_via_runtime(
                &db_path,
                &memory_flags,
                ManualCmd::Deny { id: id.clone() },
                "deny",
                &id,
            )?;
        }
        Commands::Claim { id } => {
            gate_via_runtime(
                &db_path,
                &memory_flags,
                ManualCmd::ClaimPhysical { id: id.clone() },
                "claim",
                &id,
            )?;
        }
        Commands::Complete { id } => {
            gate_via_runtime(
                &db_path,
                &memory_flags,
                ManualCmd::CompletePhysical { id: id.clone() },
                "complete",
                &id,
            )?;
        }
        Commands::DigitalFlow { sub } => match sub {
            DfCmd::Cycle { payee, json } => {
                let store = OpsStore::open(&db_path)?;
                let now = Utc::now();
                let existing = store.load_flow("flow-bill_pay")?;
                let flow = existing.unwrap_or_else(|| DigitalFlow::bill_pay("flow-bill_pay", &payee, now));
                let (flow, approval, executed) = run_cycle(&flow, "approve", "operator", now)?;
                store.save_flow(&flow)?;
                let mut snap = store
                    .load_snapshot()?
                    .unwrap_or_else(|| Snapshot::empty(now));
                snap.pending.retain(|p| p.id != approval.id);
                snap.pending.push(approval.clone());
                snap.status = ensembly_kernel::derive_status(&snap.pending);
                snap.phase = match snap.status {
                    ensembly_kernel::SnapshotStatus::IdleWaiting => "HITL_WAIT".into(),
                    ensembly_kernel::SnapshotStatus::Clear => "CLEAR".into(),
                    ensembly_kernel::SnapshotStatus::Partial => "PARTIAL".into(),
                };
                snap.updated_at = now;
                store.save_snapshot(&snap)?;
                let body = serde_json::json!({
                    "ok": true,
                    "cmd": "cycle",
                    "flow": {
                        "id": flow.id,
                        "place": flow.place,
                        "status": format!("{:?}", flow.status),
                        "title": flow.title,
                        "lastResult": flow.last_result,
                    },
                    "approvalStatus": format!("{:?}", approval.status),
                    "executed": executed,
                    "mutated": false,
                    "db": store.path(),
                });
                if json {
                    println!("{}", serde_json::to_string_pretty(&body)?);
                } else {
                    println!(
                        "DIGITAL_FLOW ok place={} status={:?} approval={:?} executed={}",
                        flow.place, flow.status, approval.status, executed
                    );
                }
            }
            DfCmd::Status { json } => {
                let store = OpsStore::open(&db_path)?;
                let flow = store.load_flow("flow-bill_pay")?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&flow)?);
                } else {
                    println!("{:?}", flow.map(|f| (f.id, f.status, f.place)));
                }
            }
        },
        Commands::Backup { out, unlock } => {
            let store = OpsStore::open(&db_path)?;
            let key = unlock_material(&unlock)?;
            let pack = create_backup_pack(&store, &key)?;
            write_backup_pack(&out, &pack)?;
            println!("BACKUP_OK path={out:?} suite={}", pack.sealed_ops.suite);
        }
        Commands::RestoreDryRun { pack, unlock } => {
            let key = unlock_material(&unlock)?;
            let pack = read_backup_pack(&pack)?;
            let report = restore_dry_run(&pack, &key)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.ok {
                std::process::exit(2);
            }
        }
        Commands::RestoreApply {
            pack,
            unlock,
            i_understand,
        } => {
            if !i_understand {
                bail!(
                    "restore-apply is destructive — re-run with --i-understand. \
                     Canonical host only; laptop must not dual-write ops SoT."
                );
            }
            let store = OpsStore::open(&db_path)?;
            let key = unlock_material(&unlock)?;
            let pack = read_backup_pack(&pack)?;
            restore_apply(&store, &pack, &key)?;
            println!(
                "RESTORE_APPLY_OK db={:?} keys={}",
                store.path(),
                store.export_bundle()?.kv.len()
            );
            eprintln!("RESTORE_OK apply db={}", store.path().display());
        }
        Commands::OpsBundle { sub } => match sub {
            OpsBundleCmd::Export { out, json } => {
                let store = OpsStore::open(&db_path)?;
                let bundle = store.export_bundle()?;
                OpsStore::write_bundle_file(&out, &bundle)?;
                let body = serde_json::json!({
                    "ok": true,
                    "cmd": "ops-bundle.export",
                    "format": bundle.format,
                    "schema_version": bundle.schema_version,
                    "kv_keys": bundle.kv.len(),
                    "path": out,
                    "db": store.path(),
                });
                if json {
                    println!("{}", serde_json::to_string_pretty(&body)?);
                } else {
                    println!(
                        "OPS_BUNDLE_EXPORT_OK path={out:?} keys={} schema={}",
                        bundle.kv.len(),
                        bundle.schema_version
                    );
                }
            }
            OpsBundleCmd::Import { bundle, dry_run, json } => {
                let read = OpsStore::read_bundle_file(&bundle)?;
                if dry_run {
                    let temp = OpsStore::open_in_memory()?;
                    temp.import_bundle(&read)?;
                    let body = serde_json::json!({
                        "ok": true,
                        "cmd": "ops-bundle.import",
                        "dry_run": true,
                        "kv_keys": read.kv.len(),
                        "message": "dry-run OK — primary store not modified",
                    });
                    if json {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    } else {
                        println!(
                            "OPS_BUNDLE_IMPORT_DRY_RUN_OK keys={} (primary not modified)",
                            read.kv.len()
                        );
                    }
                } else {
                    let store = OpsStore::open(&db_path)?;
                    store.import_bundle(&read)?;
                    let body = serde_json::json!({
                        "ok": true,
                        "cmd": "ops-bundle.import",
                        "dry_run": false,
                        "kv_keys": read.kv.len(),
                        "db": store.path(),
                    });
                    if json {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    } else {
                        println!(
                            "OPS_BUNDLE_IMPORT_OK db={:?} keys={}",
                            store.path(),
                            read.kv.len()
                        );
                    }
                    eprintln!("OPS_BUNDLE_OK import keys={}", read.kv.len());
                }
            }
        },
        Commands::PulsePack { sub } => {
            let memory_path = resolve_memory_path(cli.memory.clone());
            let archive_sidecar = PathBuf::from(DEFAULT_ARCHIVE_SIDECAR);
            match sub {
                PulsePackCmd::Export {
                    out,
                    include_archive,
                    archive_limit,
                    json,
                } => {
                    let ops_db = if include_archive {
                        Some(db_path.clone())
                    } else {
                        None
                    };
                    let pack = export_pulse_pack(&PulseExportOpts {
                        memory_path: memory_path.clone(),
                        archive_sidecar: archive_sidecar.clone(),
                        ops_db,
                        archive_limit,
                        source_host: std::env::var("HOSTNAME")
                            .or_else(|_| std::env::var("COMPUTERNAME"))
                            .unwrap_or_else(|_| "unknown".into()),
                    })?;
                    write_pulse_pack(&out, &pack)?;
                    let status = pack.status();
                    if json {
                        println!("{}", serde_json::to_string_pretty(&status)?);
                    } else {
                        println!(
                            "PULSE_EXPORT_OK path={out:?} traces={} archive={} hash={} last_seen={}",
                            status.memory_trace_count,
                            status.archive_event_count,
                            status.pack_hash,
                            status
                                .last_seen
                                .map(|t| t.to_rfc3339())
                                .unwrap_or_else(|| "-".into())
                        );
                    }
                    eprintln!(
                        "PULSE_OK export traces={} archive={}",
                        status.memory_trace_count,
                        status.archive_event_count
                    );
                }
                PulsePackCmd::Import {
                    pack,
                    dry_run,
                    archive_sidecar: archive_path,
                    json,
                } => {
                    let read = read_pulse_pack(&pack)?;
                    let report = import_pulse_pack(
                        &read,
                        &PulseImportOpts {
                            memory_path: memory_path.clone(),
                            archive_sidecar: archive_path
                                .clone()
                                .unwrap_or_else(|| archive_sidecar.clone()),
                            dry_run,
                        },
                    )?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    } else {
                        println!(
                            "PULSE_IMPORT_OK dry_run={} traces_before={} traces_after={} archive_appended={} hash={}",
                            report.dry_run,
                            report.memory_trajectory_before,
                            report.memory_trajectory_after,
                            report.archive_appended,
                            report.pack_hash
                        );
                    }
                    eprintln!(
                        "PULSE_OK import dry_run={} after={}",
                        report.dry_run,
                        report.memory_trajectory_after
                    );
                }
                PulsePackCmd::Status {
                    pack,
                    archive_sidecar: archive_path,
                    json,
                } => {
                    let sidecar = archive_path.unwrap_or(archive_sidecar);
                    let from_pack = pack.is_some();
                    let status = if let Some(p) = pack {
                        read_pulse_pack(&p)?.status()
                    } else {
                        local_pulse_status(&memory_path, &sidecar)?
                    };
                    if json {
                        println!("{}", serde_json::to_string_pretty(&status)?);
                    } else {
                        println!(
                            "# Pulse status — {}",
                            if from_pack { "pack" } else { "local" }
                        );
                        println!("format: {}", status.format);
                        println!("traces: {}", status.memory_trace_count);
                        println!("archive: {}", status.archive_event_count);
                        println!(
                            "last_seen: {}",
                            status
                                .last_seen
                                .map(|t| t.to_rfc3339())
                                .unwrap_or_else(|| "-".into())
                        );
                        println!("hash: {}", status.pack_hash);
                        if let Some(h) = &status.memory_hash {
                            println!("memory_hash: {}", h);
                        }
                    }
                }
            }
        }
        Commands::Runtime { sub } => {
            let store = OpsStore::open(&db_path)?;
            let now = Utc::now();
            let mut rt = Runtime::new(now);
            if let Some(life) = store.load_life_state()? {
                rt.state = life;
            }
            if let Some(snap) = store.load_snapshot()? {
                rt.snapshot = snap;
            }
            attach_memory(&mut rt, &memory_flags);

            match sub {
                RuntimeCmd::Load { fixture, json } => {
                    let actions = load_actions_from_fixture(&fixture)?;
                    rt.load_actions(&actions, now)?;
                    store.save_runtime_pair(&rt.state, &rt.snapshot)?;
                    save_memory(&mut rt);
                    let cp = rt.state.critical_path.as_ref();
                    let body = serde_json::json!({
                        "ok": true,
                        "cmd": "runtime.load",
                        "version": rt.state.version,
                        "regime": format!("{:?}", rt.state.regime),
                        "nodes": rt.state.graph.nodes.len(),
                        "edges": rt.state.graph.edges.len(),
                        "cp": cp.map(|c| &c.path),
                        "explain": cp.map(|c| &c.explain),
                        "monteCarlo": cp.and_then(|c| c.monte_carlo.as_ref()),
                        "db": store.path(),
                    });
                    if json {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    } else {
                        println!(
                            "RUNTIME_LOAD ok nodes={} edges={} regime={:?} {}",
                            rt.state.graph.nodes.len(),
                            rt.state.graph.edges.len(),
                            rt.state.regime,
                            cp.map(|c| c.explain.as_str()).unwrap_or("-")
                        );
                    }
                    eprintln!(
                        "RUNTIME_OK load nodes={} cp_len={}",
                        rt.state.graph.nodes.len(),
                        cp.map(|c| c.path.len()).unwrap_or(0)
                    );
                }
                RuntimeCmd::Status { json } => {
                    let cp = rt.state.critical_path.as_ref();
                    let body = serde_json::json!({
                        "ok": true,
                        "cmd": "runtime.status",
                        "version": rt.state.version,
                        "regime": format!("{:?}", rt.state.regime).to_ascii_lowercase(),
                        "fingerprint": rt.state.fingerprint,
                        "metrics": rt.state.metrics,
                        "cp": cp,
                        "pendingAuth": list_pending(&rt.snapshot).iter().map(|p| &p.id).collect::<Vec<_>>(),
                        "db": store.path(),
                    });
                    if json {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    } else {
                        println!("# LifeState v{}", rt.state.version);
                        println!("regime: {:?}", rt.state.regime);
                        println!(
                            "CP: {}",
                            cp.map(|c| c.explain.as_str()).unwrap_or("(none — run runtime load)")
                        );
                        println!(
                            "metrics: hootl_done={} hitl_surfaces={} agent_failures={}",
                            rt.state.metrics.hootl_completed,
                            rt.state.metrics.hitl_surfaces,
                            rt.state.metrics.agent_failures
                        );
                    }
                }
                RuntimeCmd::Tick { agent, json } => {
                    let report = rt.tick(agent, now)?;
                    store.save_runtime_pair(&rt.state, &rt.snapshot)?;
                    save_memory(&mut rt);
                    if json {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    } else {
                        println!(
                            "RUNTIME_TICK regime={:?} drained={} claim={:?} complete={:?} auth={:?} physical={:?} {}",
                            report.regime,
                            report.messages_drained,
                            report.hootl_claim,
                            report.hootl_complete,
                            report.next_auth,
                            report.next_physical,
                            report.cp_explain
                        );
                    }
                    eprintln!(
                        "RUNTIME_OK tick regime={:?} claim={} complete={} auth={}",
                        report.regime,
                        report.hootl_claim.as_deref().unwrap_or("-"),
                        report.hootl_complete.as_deref().unwrap_or("-"),
                        report.next_auth.as_deref().unwrap_or("-")
                    );
                }
                RuntimeCmd::Approve { id, json } => {
                    // Normalize: approve uses action id (pay-rent); auth- prefix accepted then stripped.
                    let action_id = ensembly_kernel::runtime::action_id_of(&id).to_string();
                    rt.enqueue_manual(ManualCmd::Approve { id: action_id.clone() }, now);
                    let report = rt.tick(false, now)?;
                    store.save_runtime_pair(&rt.state, &rt.snapshot)?;
                    save_memory(&mut rt);
                    let body = serde_json::json!({ "ok": true, "decision": "approve", "id": action_id, "tick": report });
                    if json {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    } else {
                        println!("RUNTIME_APPROVE id={action_id} regime={:?}", report.regime);
                    }
                }
                RuntimeCmd::Deny { id, json } => {
                    let action_id = ensembly_kernel::runtime::action_id_of(&id).to_string();
                    rt.enqueue_manual(ManualCmd::Deny { id: action_id.clone() }, now);
                    let report = rt.tick(false, now)?;
                    store.save_runtime_pair(&rt.state, &rt.snapshot)?;
                    save_memory(&mut rt);
                    let body = serde_json::json!({ "ok": true, "decision": "deny", "id": action_id, "tick": report });
                    if json {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    } else {
                        println!("RUNTIME_DENY id={action_id}");
                    }
                }
                RuntimeCmd::Claim { id, json } => {
                    rt.enqueue_manual(ManualCmd::ClaimPhysical { id: id.clone() }, now);
                    let report = rt.tick(false, now)?;
                    store.save_runtime_pair(&rt.state, &rt.snapshot)?;
                    save_memory(&mut rt);
                    let body = serde_json::json!({ "ok": true, "decision": "claim", "id": id, "tick": report });
                    if json {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    } else {
                        println!("RUNTIME_CLAIM id={id}");
                    }
                }
                RuntimeCmd::Complete { id, json } => {
                    rt.enqueue_manual(ManualCmd::CompletePhysical { id: id.clone() }, now);
                    let report = rt.tick(false, now)?;
                    store.save_runtime_pair(&rt.state, &rt.snapshot)?;
                    save_memory(&mut rt);
                    let body = serde_json::json!({ "ok": true, "decision": "complete", "id": id, "tick": report });
                    if json {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    } else {
                        println!("RUNTIME_COMPLETE id={id} regime={:?}", report.regime);
                    }
                }
                RuntimeCmd::Reflect { json } => {
                    let Some(sink) = rt.memory.as_mut() else {
                        bail!("reflect needs episodic memory — remove --no-memory (default path: {DEFAULT_MEMORY_PATH})");
                    };
                    let entries = sink.memory.doc().trajectory.len();
                    match sink.reflect() {
                        None => {
                            let msg = format!(
                                "not enough trajectory to reflect (have {entries}, need >= 5) — run runtime load/tick first"
                            );
                            if json {
                                println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "ok": false, "reason": msg }))?);
                            } else {
                                println!("REFLECT_SKIP {msg}");
                            }
                        }
                        Some(r) => {
                            save_memory(&mut rt);
                            if json {
                                println!("{}", serde_json::to_string_pretty(&r)?);
                            } else {
                                println!("REFLECT coherence={:.1}% entries={} skills={} new_skills={:?}", r.coherence * 100.0, r.trajectory_length, r.known_skills, r.new_skills);
                                for p in &r.goal_proposals {
                                    println!("  proposal [{}] {} (priority {:.1})", p.goal_type, p.description, p.priority);
                                }
                            }
                            eprintln!(
                                "REFLECT_OK coherence={:.2} skills={} proposals={}",
                                r.coherence,
                                r.known_skills,
                                r.goal_proposals.len()
                            );
                        }
                    }
                }
                RuntimeCmd::Dive { probe_budget, json } => {
                    use ensembly_kernel::critical_path::compute_critical_path;
                    use ensembly_kernel::plan_dive;
                    if rt.state.graph.nodes.is_empty() {
                        bail!("runtime dive needs a loaded graph — run `runtime load --fixture …` first");
                    }
                    let cp = match rt.state.critical_path.clone() {
                        Some(c) => c,
                        None => compute_critical_path(&rt.state.graph, 0)
                            .map_err(|e| anyhow::anyhow!(e))?,
                    };
                    let dive = plan_dive(&rt.state.graph, &cp, probe_budget);
                    if json {
                        println!("{}", serde_json::to_string_pretty(&dive)?);
                    } else {
                        println!("# UncertaintyDive v{}", dive.version);
                        println!("coach: {}", dive.coach_line);
                        println!(
                            "simulate: σ={:.2} E={:.1} cp=[{}]",
                            dive.simulate.pert_sigma,
                            dive.simulate.length_expected,
                            dive.simulate.cp_path.join(" → ")
                        );
                        if let Some(a) = &dive.next_auth {
                            println!("auth black hole: {} — {}", a.id, a.reason);
                        }
                        if let Some(p) = &dive.next_physical {
                            println!("physical beacon: {} — {}", p.id, p.reason);
                        }
                        if let Some(p) = &dive.next_probe {
                            println!(
                                "next probe: {} (score={:.1}) — {}",
                                p.id, p.uncertainty_score, p.reason
                            );
                        } else {
                            println!("next probe: (none)");
                        }
                        println!(
                            "guards: budget={} refuse_auto_auth={} claim_via_cp={}",
                            dive.trauma_guards.probe_budget,
                            dive.trauma_guards.refuse_auto_auth,
                            dive.trauma_guards.claim_via_cp_only
                        );
                    }
                    eprintln!(
                        "RUNTIME_OK dive candidates={} probe={}",
                        dive.candidates.len(),
                        dive.next_probe
                            .as_ref()
                            .map(|p| p.id.as_str())
                            .unwrap_or("-")
                    );
                }
            }
        }
        Commands::ChannelPulse { sub } => match sub {
            ChannelPulseCmd::Reconcile {
                out,
                fixture,
                location,
                json,
                verbose,
            } => {
                let store = OpsStore::open(&db_path)?;
                let actions = if let Some(f) = fixture {
                    load_actions_from_fixture(&f)?
                } else {
                    vec![]
                };
                let pulse_path = out.unwrap_or_else(|| PathBuf::from(DEFAULT_CHANNEL_PULSE_PATH));
                let report = reconcile_channel_pulse(
                    &store,
                    &actions,
                    location.as_deref(),
                    &pulse_path,
                )?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else if verbose && report.wrote {
                    eprintln!(
                        "CHANNEL_PULSE_OK wrote={} path={:?} hash={}",
                        report.wrote, report.pulse_path, report.content_hash
                    );
                }
            }
        },
    }
    Ok(())
}

/// Top-level approve/deny/claim/complete always go through Runtime (G + Snapshot).
/// Requires durable life_state from `ensembly runtime load` — no snapshot-only legacy path.
fn gate_via_runtime(
    db_path: &PathBuf,
    memory_flags: &MemoryFlags,
    cmd: ManualCmd,
    decision: &str,
    id: &str,
) -> Result<()> {
    let store = OpsStore::open(db_path)?;
    let now = Utc::now();
    let Some(life) = store.load_life_state()? else {
        bail!(
            "no life_state in DB — refuse snapshot-only {decision}. \
             Run: cargo run -p ensembly-kernel -- runtime load --fixture <path> \
             then: peram {decision} {id}  (or peram runtime {decision} {id})"
        );
    };
    let mut rt = Runtime::new(now);
    rt.state = life;
    if let Some(snap) = store.load_snapshot()? {
        rt.snapshot = snap;
    }
    attach_memory(&mut rt, memory_flags);
    let cmd = match cmd {
        ManualCmd::Approve { id } => ManualCmd::Approve {
            id: ensembly_kernel::runtime::action_id_of(&id).to_string(),
        },
        ManualCmd::Deny { id } => ManualCmd::Deny {
            id: ensembly_kernel::runtime::action_id_of(&id).to_string(),
        },
        other => other,
    };
    let resolved_id = match &cmd {
        ManualCmd::Approve { id }
        | ManualCmd::Deny { id }
        | ManualCmd::ClaimPhysical { id }
        | ManualCmd::CompletePhysical { id } => id.clone(),
        _ => id.to_string(),
    };
    rt.enqueue_manual(cmd, now);
    let report = rt.tick(false, now)?;
    store.save_runtime_pair(&rt.state, &rt.snapshot)?;
    save_memory(&mut rt);
    println!(
        "{}",
        serde_json::json!({
            "ok": true,
            "decision": decision,
            "id": resolved_id,
            "via": "runtime",
            "regime": format!("{:?}", report.regime),
            "pendingRemaining": list_pending(&rt.snapshot).iter().map(|p| &p.id).collect::<Vec<_>>(),
            "tick": report,
        })
    );
    eprintln!("GATE_OK via=runtime decision={decision} id={resolved_id}");
    Ok(())
}
