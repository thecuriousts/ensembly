//! `ensembly` CLI — dogfood entry for the operator kernel.
//! Compat alias: `peram` (one release; same binary).

#[cfg(feature = "opportunity-finder")]
mod decision_cli;
#[cfg(feature = "opportunity-finder")]
mod opportunity_cli;
use anyhow::{bail, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use ensembly_bundle::{
    export_bundle, import_bundle, load_backup_sources, manifest_file_path, merge_json_objects,
    merge_jsonl_records, merge_ops_preserve_existing, restore_bundle, verify_bundle,
    BundleManifest, ConfigSourcesError, EnsemblyStorePaths, ExportOpts, ImportOpts, OpsBundleMerge,
    RestoreOpts, MANIFEST_LEDGER_PATH, MANIFEST_MEMORY_PATH, MANIFEST_OPS_PATH,
};
use ensembly_kernel::approvals::{list_pending, Snapshot};
use ensembly_kernel::backup::{
    create_backup_pack, read_backup_pack, restore_apply, restore_dry_run, write_backup_pack,
};
use ensembly_kernel::channel_pulse::{
    project_wait_snapshot, reconcile_channel_pulse, seed_live_channel_actions,
    DEFAULT_CHANNEL_PULSE_PATH,
};
use ensembly_kernel::digital_flow::{run_cycle, DigitalFlow};
use ensembly_kernel::doctor;
use ensembly_kernel::memory_sink::MemorySink;
use ensembly_kernel::msg_bus::ManualCmd;
use ensembly_kernel::paths::{
    env_alias, migrate_local_paths_at, resolve_data_paths, DataPaths, DEFAULT_MEMORY_PATH,
};
use ensembly_kernel::pulse_core as pc;
use ensembly_kernel::pulse_pack::{
    export_pulse_pack, import_pulse_pack, local_pulse_status, read_pulse_pack, write_pulse_pack,
    PulseExportOpts, PulseImportOpts, DEFAULT_ARCHIVE_SIDECAR,
};
use ensembly_kernel::pulse_supersede as ps;
use ensembly_kernel::runtime::Runtime;
use ensembly_kernel::store::OpsBundle;
use ensembly_kernel::store::OpsStore;
use ensembly_kernel::turn::{
    actions_from_fixture_path, build_channel_ir, context_at, rank_now, Action,
};
use ensembly_kernel::{kernel_version, private_path_patterns};
use ensembly_paths::DEFAULT_OPS_PATH;
use ensembly_paths::{config_file_path, load_paths_data_dir, ConfigError};
use ensembly_schema::{commit_json, feed_json, recover_json, snapshot_json, CommitFault};
#[cfg(feature = "opportunity-finder")]
use opportunity_cli::{AppsCmd, RecordAppsCmd};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = env!("CARGO_BIN_NAME"), about = "ensembly operator kernel (Rust)")]
struct Cli {
    /// Ops SQLite path (T1). When omitted: existing `data/local/` or `private/state/` files, else `paths.data_dir` from config, else `data/local/ensembly-ops.sqlite`.
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    /// Episodic memory path. When omitted: paired with the resolved ops DB directory, or the same config / cwd rules as `--db`.
    #[arg(long, global = true)]
    memory: Option<PathBuf>,

    /// Disable episodic memory recording for this invocation.
    #[arg(long, global = true, default_value_t = false)]
    no_memory: bool,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum PulseCoreCmd {
    /// Health verdict. Exit 0 ok, 1 warn, 2 fail. Never writes.
    Check {
        #[arg(long)]
        json: bool,
    },
    /// Canonical pulse pack (pulse-pack-v1, canon 1) on stdout. Reads a Python or Rust
    /// pack, or with --from-db the store (mode=ro + query_only). Invalid: verdict on stderr, exit 2.
    Canon {
        /// Pack file (Python pulse-pack-v1, ensembly-pulse-pack-v1 or peram-pulse-pack-v1)
        pack: Option<PathBuf>,
        /// Build from --pulse-db instead (mode=ro + query_only; never writes)
        #[arg(long)]
        from_db: bool,
        #[arg(long)]
        exported_at: Option<String>,
        #[arg(long)]
        host: Option<String>,
    },
    /// Check a pack against the canonical spec. Exit 0 ok, 1 warn, 2 fail. Never writes.
    PackCheck {
        pack: PathBuf,
        /// Verdict as canonical JSON (the golden check.json shape)
        #[arg(long)]
        json: bool,
    },
    /// Read traces by key prefix or full-text search. Never writes.
    Read {
        /// nat_key prefix, e.g. `civic/`
        prefix: Option<String>,
        #[arg(long)]
        search: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Mission store (new store only; sources are read-only)
    Missions {
        #[command(subcommand)]
        sub: MissionsCmd,
    },
    /// Live memory traces: kind = memory and not superseded. Default limit 8.
    List {
        #[arg(long, default_value_t = 8)]
        limit: i64,
        #[arg(long)]
        json: bool,
    },
    /// Archive chain for one nat_key, including superseded rows.
    History {
        nat_key: String,
        #[arg(long)]
        json: bool,
    },
    /// Archive `old` under `new`. The old row stays; list hides it.
    Supersede {
        old: String,
        new: String,
        #[arg(long)]
        source: String,
        #[arg(long)]
        time: String,
        #[arg(long)]
        reason: String,
        /// Writer identity; must match the configured ENSEMBLY_WRITER value.
        #[arg(long)]
        writer: String,
        #[arg(long)]
        conflict: Option<i64>,
        #[arg(long)]
        json: bool,
    },
    /// Insert a trace. `--supersedes` archives that key instead of forking it.
    Admit {
        #[arg(long)]
        key: String,
        #[arg(long)]
        snippet: String,
        #[arg(long)]
        source: String,
        #[arg(long)]
        time: String,
        #[arg(long)]
        status: String,
        #[arg(long)]
        kind: String,
        #[arg(long, default_value = "open")]
        lock: String,
        #[arg(long)]
        supersedes: Option<String>,
        #[arg(long)]
        reason: Option<String>,
        /// Writer identity; must match the configured ENSEMBLY_WRITER value.
        #[arg(long)]
        writer: String,
        #[arg(long)]
        json: bool,
    },
    /// Turn an open or abstaining conflict into a supersession.
    Conflict {
        #[command(subcommand)]
        sub: ConflictCmd,
    },
}

#[derive(Subcommand)]
enum ConflictCmd {
    /// The winner stays live. The other row is archived and linked.
    Resolve {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        winner: String,
        #[arg(long)]
        source: String,
        #[arg(long)]
        time: String,
        #[arg(long)]
        reason: String,
        /// Writer identity; must match the configured ENSEMBLY_WRITER value.
        #[arg(long)]
        writer: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum MissionsCmd {
    /// Import the `| Mission |` table from a spec markdown into the mission store (exit 7: store exists, refused without --force)
    Import {
        #[arg(long)]
        spec: PathBuf,
        /// Replace an existing store (old one is kept as .bak-<utc>)
        #[arg(long)]
        force: bool,
    },
    /// List missions in the store
    List {
        #[arg(long)]
        json: bool,
    },
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Commands {
    /// Print kernel version + law banner
    Version,
    /// Versions, harness and plugin state. Exit 0 healthy or offline, 1 broken install or store, 2 usage.
    Doctor {
        /// Skip the latest-tag lookup.
        #[arg(long)]
        offline: bool,
        #[arg(long)]
        json: bool,
    },
    /// Write the default config to $XDG_CONFIG_HOME/ensembly/config.toml and print its path.
    /// Exit 7: config exists, refused without --force. Local only; no network.
    Init {
        /// Replace an existing config file
        #[arg(long)]
        force: bool,
    },
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
    Approve { id: String },
    /// Deny a pending authorization id
    Deny { id: String },
    /// Claim a physical action id
    Claim { id: String },
    /// Complete a physical action id
    Complete { id: String },
    /// Digital-flow bill_pay cycle (HITL dry-run)
    #[command(name = "digital-flow")]
    DigitalFlow {
        #[command(subcommand)]
        sub: DfCmd,
    },
    /// Create sealed backup pack of T1 ops (uses OpsBundle inside BackupPack)
    #[command(name = "sealed-backup", visible_alias = "ops-backup-pack")]
    SealedBackup {
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
    /// Export a portable plain-text bundle (kernel stores + configured backup sources)
    Export {
        #[arg(long)]
        out: PathBuf,
        /// When output exists and is non-empty, create a timestamped subdirectory instead of refusing
        #[arg(long)]
        force_new_subdir: bool,
        /// Skip individual files larger than this limit (MiB); recorded in MANIFEST
        #[arg(long, default_value_t = 200)]
        max_file_mb: u64,
    },
    /// Portable plain-text backup into timestamped dir under data_dir/backups or --out
    Backup {
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        force_new_subdir: bool,
        #[arg(long, default_value_t = 200)]
        max_file_mb: u64,
    },
    /// Verify MANIFEST checksums and plain-text parse rules for a portable bundle
    Verify { bundle: PathBuf },
    /// Verify then acknowledge a portable bundle (no default writes to live stores)
    Import {
        bundle: PathBuf,
        #[arg(long)]
        into: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        merge: bool,
    },
    /// Verify then restore into explicit destinations (`--into` sets ops + memory under one data dir)
    Restore {
        bundle: PathBuf,
        /// Data directory: writes ops sqlite + memory JSON using standard filenames
        #[arg(long)]
        into: Option<PathBuf>,
        #[arg(long)]
        into_ops: Option<PathBuf>,
        #[arg(long)]
        into_memory: Option<PathBuf>,
        #[arg(long)]
        sources_to: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        merge: bool,
    },
    /// Portable pulse + memory sync (laptop ↔ bot; no dual-writer ops)
    #[command(name = "pulse-pack")]
    PulsePack {
        #[command(subcommand)]
        sub: PulsePackCmd,
    },
    /// Pulse: read-only check and read, plus explicit supersession of a trace
    Pulse {
        /// pulse-memory SQLite. Check and read open it read-only. Supersede, admit, and conflict resolve write it. Default: ~/.local/share/pulse-memory/pulse.sqlite
        #[arg(long, global = true)]
        pulse_db: Option<PathBuf>,
        /// premflow binary. Default: first `premflow` on PATH, else ~/.local/bin/premflow
        #[arg(long, global = true)]
        premflow_bin: Option<PathBuf>,
        /// Mission store directory. Default: /workspace/ops/ensembly-pulse/missions
        #[arg(long, global = true)]
        missions_dir: Option<PathBuf>,
        /// Override "now" (RFC 3339 or YYYY-MM-DD), for tests and replays
        #[arg(long, global = true)]
        now: Option<String>,
        #[command(subcommand)]
        sub: PulseCoreCmd,
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
    /// Application ledger (SoT for job applications): validate, append, export
    #[cfg(feature = "opportunity-finder")]
    Apps {
        /// Ledger JSONL (default data/local/applications/ledger.jsonl; local-only, gitignored)
        #[arg(long)]
        ledger: Option<PathBuf>,
        #[command(subcommand)]
        sub: AppsCmd,
    },
    /// Append a record unit, or read a snapshot or change feed
    Record {
        #[command(subcommand)]
        sub: RecordCmd,
    },
    /// Decision model: eval, propose-and-wait, empty labelling template. The decider is not live.
    #[cfg(feature = "opportunity-finder")]
    Decision {
        #[command(subcommand)]
        sub: decision_cli::DecisionCmd,
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
enum RecordCmd {
    /// Append one unit
    Commit {
        /// Event log path
        #[arg(long)]
        log: PathBuf,
        /// Unit JSON: an event array, or an object with an events array
        #[arg(long)]
        unit: String,
    },
    /// Finish a commit that stopped after fsync
    Recover {
        /// Event log path
        #[arg(long)]
        log: PathBuf,
    },
    /// Read a snapshot or a change feed
    View {
        #[command(subcommand)]
        sub: RecordViewCmd,
    },
    /// Map the application ledger onto a record log, or compare the two
    #[cfg(feature = "opportunity-finder")]
    Apps {
        #[command(subcommand)]
        sub: RecordAppsCmd,
    },
}

#[derive(Subcommand)]
enum RecordViewCmd {
    /// Facts at one version
    Snapshot {
        /// Event log path
        #[arg(long)]
        log: PathBuf,
        /// Version to read. Omit for the newest version in the log.
        #[arg(long)]
        at: Option<u64>,
    },
    /// Events strictly after a version
    Feed {
        /// Event log path
        #[arg(long)]
        log: PathBuf,
        #[arg(long)]
        since: u64,
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
        /// CI dogfood only — never the live SoT path (prefer durable ops seed)
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
fn attach_memory(rt: &mut Runtime, flags: &MemoryFlags, paired: &DataPaths) {
    if flags.no_memory {
        return;
    }
    let explicit = flags.memory.is_some();
    let path = if explicit {
        flags.memory.clone().expect("explicit memory flag")
    } else {
        paired.memory.clone()
    };
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
    bail!(
        "unlock required: pass --unlock or set ENSEMBLY_UNLOCK (alias PERAM_UNLOCK; keyring later)"
    );
}

fn load_actions_from_fixture(path: &std::path::Path) -> Result<Vec<Action>> {
    actions_from_fixture_path(path).map_err(|e| anyhow::anyhow!("{e}"))
}

fn ensure_snap(store: &OpsStore, actions: &[Action]) -> Result<Snapshot> {
    let now = Utc::now();
    let existing = store.load_snapshot()?;
    let snap = project_wait_snapshot(actions, existing, now);
    store.save_snapshot(&snap)?;
    Ok(snap)
}

fn pulse_cfg(
    pulse_db: Option<PathBuf>,
    premflow_bin: Option<PathBuf>,
    missions_dir: Option<PathBuf>,
    now: Option<String>,
) -> Result<pc::PulseCfg> {
    let now = match now.as_deref() {
        None => Utc::now(),
        Some(s) => pc::parse_ts(s).ok_or_else(|| {
            anyhow::anyhow!("--now: cannot parse `{s}` (use RFC 3339 or YYYY-MM-DD)")
        })?,
    };
    let mut cfg = pc::PulseCfg::from_env(now);
    cfg.db = pulse_db.unwrap_or(cfg.db);
    cfg.premflow_bin = premflow_bin.unwrap_or(cfg.premflow_bin);
    cfg.missions_dir = missions_dir.unwrap_or(cfg.missions_dir);
    Ok(cfg)
}

fn pulse_read(
    cfg: &pc::PulseCfg,
    prefix: Option<String>,
    search: Option<String>,
    limit: usize,
    json: bool,
) -> Result<()> {
    let conn = pc::open_ro(&cfg.db)?;
    let rows = match (prefix, search) {
        (Some(p), None) => pc::read_prefix(&conn, &p, limit)?,
        (None, Some(q)) => pc::read_search(&conn, &q, limit)?,
        _ => bail!("give exactly one of <key-prefix> or --search <text>"),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        print!("{}", pc::render_traces(&rows));
    }
    Ok(())
}

fn pulse_import(cfg: &pc::PulseCfg, spec: &std::path::Path, force: bool) -> Result<()> {
    let st = match pc::import_missions(cfg, spec, force) {
        Ok(st) => st,
        Err(e) if e.downcast_ref::<pc::RefusedWrite>().is_some() => {
            eprintln!("{e}");
            std::process::exit(pc::REFUSED_WRITE_EXIT);
        }
        Err(e) => return Err(e),
    };
    let unresolved = st
        .missions
        .iter()
        .flat_map(|m| &m.g_files)
        .filter(|g| g.resolved.is_none())
        .count();
    println!(
        "imported {} missions into {} ({} owner(s) unconfirmed, {} G path(s) unresolved)",
        st.missions.len(),
        cfg.missions_dir.join(pc::MISSIONS_FILE).display(),
        st.missions.iter().filter(|m| !m.owner_confirmed).count(),
        unresolved
    );
    Ok(())
}

fn pulse_list(cfg: &pc::PulseCfg, json: bool) -> Result<()> {
    let Some(st) = pc::load_store(&cfg.missions_dir)? else {
        bail!("no mission store at {}", cfg.missions_dir.display());
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&st)?);
        return Ok(());
    }
    for m in &st.missions {
        let flag = if m.owner_confirmed {
            ""
        } else {
            " (unconfirmed)"
        };
        println!(
            "{:<40} {:<6} {:<8} {}{}",
            m.id, m.horizon, m.status, m.owner, flag
        );
    }
    Ok(())
}

fn pulse_canon(
    cfg: &pc::PulseCfg,
    pack: Option<PathBuf>,
    from_db: bool,
    exported_at: Option<String>,
    host: Option<String>,
) -> Result<()> {
    use ensembly_kernel::pulse_pack::canonical as canon;
    use std::io::Write;
    let (doc, verdict) = match (pack, from_db) {
        (Some(p), false) => canon::check_bytes(&std::fs::read(&p)?),
        (None, true) => {
            let (Some(at), Some(h)) = (exported_at, host) else {
                anyhow::bail!("canon --from-db needs --exported-at and --host");
            };
            match canon::canonical_from_db_path(&cfg.db, &at, &h) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("pulse store: {e:#}");
                    std::process::exit(2);
                }
            }
        }
        _ => anyhow::bail!("canon needs exactly one of PACK or --from-db"),
    };
    match doc {
        Some(d) => {
            let bytes = canon::canonical_bytes(&d).map_err(|i| anyhow::anyhow!("{i:?}"))?;
            std::io::stdout().write_all(&bytes)?;
            Ok(())
        }
        None => {
            std::io::stderr().write_all(&canon::verdict_bytes(&verdict))?;
            std::process::exit(2);
        }
    }
}

fn pulse_pack_check(pack: &std::path::Path, json: bool) -> Result<()> {
    use ensembly_kernel::pulse_pack::canonical as canon;
    use std::io::Write;
    let v = canon::check_pack(&std::fs::read(pack)?);
    if json {
        std::io::stdout().write_all(&canon::verdict_bytes(&v))?;
    } else {
        println!(
            "{} ({} traces, {} archive, source {})",
            v.verdict,
            v.traces,
            v.archive,
            v.source_format.as_deref().unwrap_or("unknown")
        );
        for i in v.errors.iter().chain(v.warnings.iter()) {
            println!(
                "  {} {}",
                i.code,
                if i.path.is_empty() { "/" } else { &i.path }
            );
        }
        if let Some(h) = &v.pack_sha256 {
            println!("  pack_sha256 {h}");
        }
    }
    std::process::exit(v.exit);
}

fn run_pulse(
    pulse_db: Option<PathBuf>,
    premflow_bin: Option<PathBuf>,
    missions_dir: Option<PathBuf>,
    now: Option<String>,
    sub: PulseCoreCmd,
) -> Result<()> {
    let cfg = pulse_cfg(pulse_db, premflow_bin, missions_dir, now)?;
    match sub {
        PulseCoreCmd::Check { json } => {
            let r = pc::run_check(&cfg);
            if json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                print!("{}", pc::render_check(&r));
            }
            std::process::exit(r.exit_code);
        }
        PulseCoreCmd::Read {
            prefix,
            search,
            limit,
            json,
        } => pulse_read(&cfg, prefix, search, limit, json),
        PulseCoreCmd::Canon {
            pack,
            from_db,
            exported_at,
            host,
        } => pulse_canon(&cfg, pack, from_db, exported_at, host),
        PulseCoreCmd::PackCheck { pack, json } => pulse_pack_check(&pack, json),
        PulseCoreCmd::Missions {
            sub: MissionsCmd::Import { spec, force },
        } => pulse_import(&cfg, &spec, force),
        PulseCoreCmd::Missions {
            sub: MissionsCmd::List { json },
        } => pulse_list(&cfg, json),
        PulseCoreCmd::List { limit, json } => pulse_live(&cfg, limit, json),
        PulseCoreCmd::History { nat_key, json } => pulse_history(&cfg, &nat_key, json),
        PulseCoreCmd::Supersede {
            old,
            new,
            source,
            time,
            reason,
            writer,
            conflict,
            json,
        } => pulse_supersede_cmd(
            &cfg,
            ps::SupersedeRequest {
                old_key: old,
                new_key: new,
                source,
                event_time: time,
                reason,
                writer,
                conflict_id: conflict,
            },
            json,
        ),
        PulseCoreCmd::Admit {
            key,
            snippet,
            source,
            time,
            status,
            kind,
            lock,
            supersedes,
            reason,
            writer,
            json,
        } => pulse_admit_cmd(
            &cfg,
            ps::AdmitRequest {
                key,
                snippet,
                source,
                time,
                status,
                kind,
                lock,
                supersedes,
                reason,
                writer,
            },
            json,
        ),
        PulseCoreCmd::Conflict {
            sub:
                ConflictCmd::Resolve {
                    id,
                    winner,
                    source,
                    time,
                    reason,
                    writer,
                    json,
                },
        } => pulse_resolve_cmd(
            &cfg,
            ps::ResolveRequest {
                conflict_id: id,
                winner,
                source,
                event_time: time,
                reason,
                writer,
            },
            json,
        ),
    }
}

fn refuse_writer<T>(result: Result<T, ensembly_kernel::WriterError>) -> Result<T> {
    use ensembly_kernel::WriterError;
    match result {
        Ok(value) => Ok(value),
        Err(err) => {
            let refused = matches!(
                err,
                WriterError::Refused(_) | WriterError::Pulse(ps::SupersedeError::Refused(_))
            );
            if refused {
                eprintln!("{err}");
                std::process::exit(pc::REFUSED_WRITE_EXIT);
            }
            Err(anyhow::anyhow!("{err}"))
        }
    }
}

fn pulse_writer(cfg: &pc::PulseCfg) -> Result<ensembly_kernel::KernelWriter> {
    ensembly_kernel::KernelWriter::open_pulse(&cfg.db).map_err(anyhow::Error::from)
}

fn print_supersede(receipt: &ps::SupersedeReceipt, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(receipt)?);
    } else {
        let replay = if receipt.idempotent {
            ", idempotent"
        } else {
            ""
        };
        println!(
            "superseded {} -> {} ({}{replay})",
            receipt.old_key, receipt.new_key, receipt.phase
        );
    }
    Ok(())
}

fn pulse_live(cfg: &pc::PulseCfg, limit: i64, json: bool) -> Result<()> {
    let conn = pc::open_ro(&cfg.db)?;
    let rows = ps::list_live(&conn, limit).map_err(|err| anyhow::anyhow!("{err}"))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        print!("{}", pc::render_traces(&rows));
    }
    Ok(())
}

fn pulse_history(cfg: &pc::PulseCfg, key: &str, json: bool) -> Result<()> {
    let conn = pc::open_ro(&cfg.db)?;
    match ps::read_history(&conn, key) {
        Ok(history) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&history)?);
            } else {
                print!("{}", pc::render_traces(&history.chain));
            }
            Ok(())
        }
        Err(err @ ps::SupersedeError::Refused(_)) => {
            eprintln!("{err}");
            std::process::exit(pc::REFUSED_WRITE_EXIT);
        }
        Err(err) => Err(anyhow::anyhow!("{err}")),
    }
}

fn pulse_supersede_cmd(cfg: &pc::PulseCfg, req: ps::SupersedeRequest, json: bool) -> Result<()> {
    let writer = pulse_writer(cfg)?;
    let receipt = refuse_writer(writer.supersede(&req, &ps::SupersedeFault::default()))?;
    print_supersede(&receipt, json)
}

fn pulse_admit_cmd(cfg: &pc::PulseCfg, req: ps::AdmitRequest, json: bool) -> Result<()> {
    let writer = pulse_writer(cfg)?;
    let receipt = refuse_writer(writer.admit(&req, &ps::SupersedeFault::default()))?;
    if json {
        println!("{}", serde_json::to_string(&receipt)?);
    } else {
        println!("admitted {} seen {}x", receipt.nat_key, receipt.seen_count);
        if let Some(link) = &receipt.superseded {
            print_supersede(link, false)?;
        }
    }
    Ok(())
}

fn pulse_resolve_cmd(cfg: &pc::PulseCfg, req: ps::ResolveRequest, json: bool) -> Result<()> {
    let writer = pulse_writer(cfg)?;
    let receipt = refuse_writer(writer.resolve_conflict(&req, &ps::SupersedeFault::default()))?;
    print_supersede(&receipt, json)
}

fn should_skip_eager_config(cli: &Cli) -> bool {
    matches!(cli.cmd, Commands::Version | Commands::Init { .. })
        || (cli.db.is_some() && cli.memory.is_some())
        || (cli.db.is_some() ^ cli.memory.is_some())
}

fn load_operator_config(cli: &Cli) -> (Option<PathBuf>, Option<ConfigError>) {
    if should_skip_eager_config(cli) {
        return (None, None);
    }
    match load_paths_data_dir() {
        Ok(v) => (v, None),
        Err(e) => {
            if matches!(cli.cmd, Commands::Doctor { .. }) {
                (None, Some(e))
            } else {
                eprintln!("{e}");
                std::process::exit(2);
            }
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (config_data_dir, config_problem) = load_operator_config(&cli);
    let data_paths = resolve_data_paths(
        Path::new(""),
        cli.db.clone(),
        cli.memory.clone(),
        config_data_dir.as_deref(),
    );
    let ctx = Ctx {
        db_path: data_paths.ops_db.clone(),
        data_paths,
        memory_flags: MemoryFlags {
            memory: cli.memory.clone(),
            no_memory: cli.no_memory,
        },
        memory_cli: cli.memory.clone(),
        config_problem,
    };
    dispatch(&ctx, cli.cmd)
}

/// Values every subcommand may need, lifted out of `Cli` before the match moves `cmd`.
struct Ctx {
    db_path: PathBuf,
    data_paths: DataPaths,
    memory_flags: MemoryFlags,
    memory_cli: Option<PathBuf>,
    config_problem: Option<ConfigError>,
}

impl Ctx {
    fn resolve_memory_path(&self) -> PathBuf {
        if let Some(ref p) = self.memory_cli {
            return p.clone();
        }
        self.data_paths.memory.clone()
    }
}

fn dispatch(ctx: &Ctx, cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Version => cmd_version(),
        Commands::Doctor { offline, json } => {
            cmd_doctor(&ctx.db_path, offline, json, ctx.config_problem.as_ref())
        }
        Commands::Init { force } => cmd_init(force),
        Commands::MigrateLocalPaths {
            dry_run,
            root,
            json,
        } => cmd_migrate(dry_run, root, json),
        Commands::Turn {
            fixture,
            json,
            channel,
            location,
        } => cmd_turn(&ctx.db_path, fixture, json, channel, location),
        Commands::Approve { id } => {
            manual_via_runtime(ctx, ManualCmd::Approve { id: id.clone() }, "approve", &id)
        }
        Commands::Deny { id } => {
            manual_via_runtime(ctx, ManualCmd::Deny { id: id.clone() }, "deny", &id)
        }
        Commands::Claim { id } => manual_via_runtime(
            ctx,
            ManualCmd::ClaimPhysical { id: id.clone() },
            "claim",
            &id,
        ),
        Commands::Complete { id } => manual_via_runtime(
            ctx,
            ManualCmd::CompletePhysical { id: id.clone() },
            "complete",
            &id,
        ),
        Commands::DigitalFlow { sub } => cmd_digital_flow(&ctx.db_path, sub),
        Commands::SealedBackup { out, unlock } => cmd_backup(&ctx.db_path, &out, &unlock),
        Commands::RestoreDryRun { pack, unlock } => cmd_restore_dry_run(&pack, &unlock),
        Commands::RestoreApply {
            pack,
            unlock,
            i_understand,
        } => cmd_restore_apply(&ctx.db_path, &pack, &unlock, i_understand),
        Commands::OpsBundle { sub } => cmd_ops_bundle(&ctx.db_path, sub),
        Commands::Export {
            out,
            force_new_subdir,
            max_file_mb,
        } => cmd_portable_export(ctx, &out, force_new_subdir, max_file_mb, false),
        Commands::Backup {
            out,
            force_new_subdir,
            max_file_mb,
        } => {
            let target = out.unwrap_or_else(default_portable_backup_out);
            cmd_portable_export(ctx, &target, force_new_subdir, max_file_mb, true)
        }
        Commands::Verify { bundle } => cmd_portable_verify(&bundle),
        Commands::Import {
            bundle,
            into,
            dry_run,
            merge,
        } => cmd_portable_import(ctx, &bundle, into, dry_run, merge),
        Commands::Restore {
            bundle,
            into,
            into_ops,
            into_memory,
            sources_to,
            dry_run,
            merge,
        } => cmd_portable_restore(
            ctx,
            &bundle,
            into,
            into_ops,
            into_memory,
            sources_to,
            dry_run,
            merge,
        ),
        Commands::Pulse {
            pulse_db,
            premflow_bin,
            missions_dir,
            now,
            sub,
        } => run_pulse(pulse_db, premflow_bin, missions_dir, now, sub),
        Commands::PulsePack { sub } => cmd_pulse_pack(ctx, sub),
        Commands::Runtime { sub } => cmd_runtime(ctx, sub),
        #[cfg(feature = "opportunity-finder")]
        Commands::Apps { ledger, sub } => opportunity_cli::cmd_apps(ledger, sub),
        Commands::ChannelPulse { sub } => cmd_channel_pulse(&ctx.db_path, sub),
        Commands::Record { sub } => cmd_record(sub),
        #[cfg(feature = "opportunity-finder")]
        Commands::Decision { sub } => decision_cli::run(sub),
    }
}

fn cmd_record(cmd: RecordCmd) -> Result<()> {
    match cmd {
        RecordCmd::Commit { log, unit } => {
            let result = commit_json(&log, &unit, &CommitFault::default())?;
            println!("{}", serde_json::to_string(&result)?);
            Ok(())
        }
        RecordCmd::Recover { log } => {
            let result = recover_json(&log, &CommitFault::default())?;
            println!("{}", serde_json::to_string(&result)?);
            Ok(())
        }
        RecordCmd::View { sub } => match sub {
            RecordViewCmd::Snapshot { log, at } => {
                let snap = snapshot_json(&log, at)?;
                println!("{}", serde_json::to_string(&snap)?);
                Ok(())
            }
            RecordViewCmd::Feed { log, since } => {
                let feed = feed_json(&log, since)?;
                println!("{}", serde_json::to_string(&feed)?);
                Ok(())
            }
        },
        #[cfg(feature = "opportunity-finder")]
        RecordCmd::Apps { sub } => opportunity_cli::cmd_record_apps(sub),
    }
}

fn print_json<T: serde::Serialize>(v: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

fn cmd_version() -> Result<()> {
    println!("{}", kernel_version());
    println!("private_paths: {:?}", private_path_patterns());
    println!("law: Node src/* legacy; ensembly-kernel is control SoT");
    Ok(())
}

/// Exit code when `init` finds a config and `--force` is not set.
const INIT_EXISTS_EXIT: i32 = 7;

/// XDG base dir: an absolute `$var`, else `$HOME/<fallback>`. Relative values are ignored (XDG spec).
fn xdg_dir(var: &str, fallback: &str) -> Result<PathBuf> {
    let set = std::env::var_os(var).map(PathBuf::from);
    if let Some(p) = set.filter(|p| p.is_absolute()) {
        return Ok(p);
    }
    match std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        Some(home) => Ok(PathBuf::from(home).join(fallback)),
        None => bail!("init: {var} and HOME are both unset"),
    }
}

/// TOML basic string. Refuses control characters rather than guessing an escape.
fn toml_str(p: &Path) -> Result<String> {
    let Some(s) = p.to_str() else {
        bail!("init: path is not UTF-8: {}", p.display());
    };
    if s.chars().any(char::is_control) {
        bail!("init: path has control characters: {}", p.display());
    }
    Ok(format!(
        "\"{}\"",
        s.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

fn default_config(data_dir: &Path) -> Result<String> {
    Ok(format!(
        "# ensembly config. Every key: docs/CONFIG.md. Local only: no telemetry, no network.\n\
         schema = 1\n\
         \n\
         [paths]\n\
         # Kernel data: ops SQLite and episodic memory.\n\
         data_dir = {}\n",
        toml_str(data_dir)?
    ))
}

/// True if the path is present (file, directory, or symlink, including dangling).
fn init_target_present(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

fn write_config_atomically(path: &Path, body: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("init: config path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let stem = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("config.toml");
    let tmp = parent.join(format!(".{stem}.init.tmp"));
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn cmd_init(force: bool) -> Result<()> {
    let path = xdg_dir("XDG_CONFIG_HOME", ".config")?.join("ensembly/config.toml");
    let data_dir = xdg_dir("XDG_DATA_HOME", ".local/share")?.join("ensembly");
    if init_target_present(&path) && !force {
        eprintln!("init: {} exists; refused without --force", path.display());
        std::process::exit(INIT_EXISTS_EXIT);
    }
    let body = default_config(&data_dir)?;
    std::fs::create_dir_all(&data_dir)?;
    write_config_atomically(&path, &body)?;
    println!("{}", path.display());
    Ok(())
}

fn cmd_doctor(
    db: &std::path::Path,
    offline: bool,
    json: bool,
    config_problem: Option<&ConfigError>,
) -> Result<()> {
    let config_msg = config_problem.map(|e| e.to_string());
    let report = doctor::run(
        &doctor::Opts::from_env(db.to_path_buf(), offline),
        config_msg.as_deref(),
    );
    if json {
        print_json(&report)?;
    } else {
        println!("{}", report.render());
    }
    std::process::exit(report.exit_code());
}

fn cmd_migrate(dry_run: bool, root: Option<PathBuf>, json: bool) -> Result<()> {
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let report = migrate_local_paths_at(&root, dry_run)?;
    if json {
        return print_json(&report);
    }
    if report.copied.is_empty() && report.skipped_dest_exists.is_empty() {
        println!(
            "MIGRATE_LOCAL no legacy peram-* files to copy under {}",
            root.display()
        );
    }
    for a in &report.copied {
        let verb = if report.dry_run {
            "would-copy"
        } else {
            "copied"
        };
        println!(
            "MIGRATE_LOCAL {verb} {} → {}",
            a.from.display(),
            a.to.display()
        );
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
    Ok(())
}

/// Drive FocusPlan from durable life-state when present, else from a valid
/// action graph (then persist the runtime pair).
fn turn_focus(
    store: &OpsStore,
    actions: &[Action],
    snap: &Snapshot,
    plan: ensembly_kernel::turn::FocusPlan,
) -> Result<ensembly_kernel::turn::FocusPlan> {
    // When durable life-state exists, drive FocusPlan from CP (read-only; no persist).
    if let Ok(Some(life)) = store.load_life_state() {
        let now = Utc::now();
        let mut rt = Runtime::new(now);
        rt.state = life;
        rt.snapshot = snap.clone();
        return Ok(rt.focus_plan(plan));
    }
    if ensembly_kernel::DepGraph::from_actions(actions, &Default::default()).is_err() {
        return Ok(plan);
    }
    let now = Utc::now();
    let mut rt = Runtime::new(now);
    if rt.load_actions(actions, now).is_err() {
        return Ok(plan);
    }
    let plan = rt.focus_plan(plan);
    // Fail loud — dual SoT must not partially persist.
    store.save_runtime_pair(&rt.state, &rt.snapshot)?;
    Ok(plan)
}

fn title_and_id(p: &Option<ensembly_kernel::turn::FocusItem>) -> String {
    p.as_ref()
        .map(|p| format!("{} ({})", p.title, p.id))
        .unwrap_or_else(|| "—".into())
}

fn id_or_dash(p: &Option<ensembly_kernel::turn::FocusItem>) -> &str {
    p.as_ref().map(|p| p.id.as_str()).unwrap_or("-")
}

fn print_turn_text(plan: &ensembly_kernel::turn::FocusPlan, store: &OpsStore) {
    println!("# FocusPlan — {}", plan.at.to_rfc3339());
    println!("biome: {}", plan.biome);
    println!("location: {:?}", plan.location_label);
    println!("next physical: {}", title_and_id(&plan.primary_physical));
    println!("next auth: {}", title_and_id(&plan.primary_auth));
    println!("places: {:?}", plan.places);
    println!("coach: {}", plan.coach_line);
    println!("db: {:?}", store.path());
}

fn manual_via_runtime(ctx: &Ctx, cmd: ManualCmd, decision: &str, id: &str) -> Result<()> {
    gate_via_runtime(
        &ctx.db_path,
        &ctx.memory_flags,
        &ctx.data_paths,
        cmd,
        decision,
        id,
    )
}

fn cmd_turn(
    db_path: &PathBuf,
    fixture: Option<PathBuf>,
    json: bool,
    channel: bool,
    location: Option<String>,
) -> Result<()> {
    let store = OpsStore::open(db_path)?;
    // Live seed from ops life-state / wait-snapshot; --fixture is CI dogfood only.
    // Fail-closed (CHANNEL_IR_FAIL) when empty — never silent grocery/rent SoT.
    let actions = match fixture {
        Some(f) => load_actions_from_fixture(&f)?,
        None => seed_live_channel_actions(&store)?,
    };
    let snap = ensure_snap(&store, &actions)?;
    let plan = rank_now(
        &context_at(Utc::now(), location.as_deref()),
        &actions,
        &[],
        &snap,
    );
    let plan = turn_focus(&store, &actions, &snap, plan)?;
    if channel {
        return print_json(&build_channel_ir(&plan, &snap, Utc::now()));
    }
    if json {
        print_json(&plan)?;
    } else {
        print_turn_text(&plan, &store);
    }
    eprintln!(
        "TURN_OK physical={} pending={} nextPhysical={} nextAuth={} biome={}",
        plan.physical_count,
        plan.pending_count,
        id_or_dash(&plan.primary_physical),
        id_or_dash(&plan.primary_auth),
        plan.biome
    );
    Ok(())
}

fn cmd_digital_flow(db_path: &PathBuf, sub: DfCmd) -> Result<()> {
    match sub {
        DfCmd::Cycle { payee, json } => df_cycle(db_path, &payee, json),
        DfCmd::Status { json } => {
            let store = OpsStore::open(db_path)?;
            let flow = store.load_flow("flow-bill_pay")?;
            if json {
                print_json(&flow)?;
            } else {
                println!("{:?}", flow.map(|f| (f.id, f.status, f.place)));
            }
            Ok(())
        }
    }
}

fn phase_of(status: &ensembly_kernel::SnapshotStatus) -> String {
    match status {
        ensembly_kernel::SnapshotStatus::IdleWaiting => "HITL_WAIT".into(),
        ensembly_kernel::SnapshotStatus::Clear => "CLEAR".into(),
        ensembly_kernel::SnapshotStatus::Partial => "PARTIAL".into(),
    }
}

fn df_cycle(db_path: &PathBuf, payee: &str, json: bool) -> Result<()> {
    let store = OpsStore::open(db_path)?;
    let now = Utc::now();
    let existing = store.load_flow("flow-bill_pay")?;
    let flow = existing.unwrap_or_else(|| DigitalFlow::bill_pay("flow-bill_pay", payee, now));
    let (flow, approval, executed) = run_cycle(&flow, "approve", "operator", now)?;
    store.save_flow(&flow)?;
    let mut snap = store
        .load_snapshot()?
        .unwrap_or_else(|| Snapshot::empty(now));
    snap.pending.retain(|p| p.id != approval.id);
    snap.pending.push(approval.clone());
    snap.status = ensembly_kernel::derive_status(&snap.pending);
    snap.phase = phase_of(&snap.status);
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
        return print_json(&body);
    }
    println!(
        "DIGITAL_FLOW ok place={} status={:?} approval={:?} executed={}",
        flow.place, flow.status, approval.status, executed
    );
    Ok(())
}

fn cmd_backup(db_path: &PathBuf, out: &PathBuf, unlock: &Option<String>) -> Result<()> {
    let store = OpsStore::open(db_path)?;
    let key = unlock_material(unlock)?;
    let pack = create_backup_pack(&store, &key)?;
    write_backup_pack(out, &pack)?;
    println!("BACKUP_OK path={out:?} suite={}", pack.sealed_ops.suite);
    Ok(())
}

fn cmd_restore_dry_run(pack: &PathBuf, unlock: &Option<String>) -> Result<()> {
    let key = unlock_material(unlock)?;
    let pack = read_backup_pack(pack)?;
    let report = restore_dry_run(&pack, &key)?;
    print_json(&report)?;
    if !report.ok {
        std::process::exit(2);
    }
    Ok(())
}

fn cmd_restore_apply(
    db_path: &PathBuf,
    pack: &PathBuf,
    unlock: &Option<String>,
    i_understand: bool,
) -> Result<()> {
    if !i_understand {
        bail!(
            "restore-apply is destructive — re-run with --i-understand. \
             Canonical host only; laptop must not dual-write ops SoT."
        );
    }
    let store = OpsStore::open(db_path)?;
    let key = unlock_material(unlock)?;
    let pack = read_backup_pack(pack)?;
    restore_apply(&store, &pack, &key)?;
    println!(
        "RESTORE_APPLY_OK db={:?} keys={}",
        store.path(),
        store.export_bundle()?.kv.len()
    );
    eprintln!("RESTORE_OK apply db={}", store.path().display());
    Ok(())
}

fn cmd_ops_bundle(db_path: &PathBuf, sub: OpsBundleCmd) -> Result<()> {
    match sub {
        OpsBundleCmd::Export { out, json } => bundle_export(db_path, &out, json),
        OpsBundleCmd::Import {
            bundle,
            dry_run: true,
            json,
        } => bundle_import_dry(&bundle, json),
        OpsBundleCmd::Import {
            bundle,
            dry_run: false,
            json,
        } => bundle_import(db_path, &bundle, json),
    }
}

fn default_portable_backup_out() -> PathBuf {
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    load_paths_data_dir()
        .ok()
        .flatten()
        .unwrap_or_else(|| PathBuf::from("data"))
        .join("backups")
        .join(format!("portable-{stamp}"))
}

fn load_portable_sources() -> Vec<ensembly_bundle::BackupSource> {
    let path = config_file_path().unwrap_or_default();
    match load_backup_sources(&path) {
        Ok(v) => v,
        Err(ConfigSourcesError::Io { .. }) if !path.is_file() => vec![],
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}

fn portable_ops_temp(ctx: &Ctx) -> Result<PathBuf> {
    let store = OpsStore::open(&ctx.db_path)?;
    let bundle = store.export_bundle()?;
    let tmp = std::env::temp_dir().join(format!(
        "ensembly-portable-ops-{}.json",
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    OpsStore::write_bundle_file(&tmp, &bundle)?;
    Ok(tmp)
}

#[cfg(feature = "opportunity-finder")]
fn default_ledger_path(ctx: &Ctx) -> Option<PathBuf> {
    let base = ctx
        .db_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("data/local"));
    let led = base.join("applications/ledger.jsonl");
    led.is_file().then_some(led)
}

#[cfg(not(feature = "opportunity-finder"))]
fn default_ledger_path(_ctx: &Ctx) -> Option<PathBuf> {
    None
}

fn cmd_portable_export(
    ctx: &Ctx,
    out: &Path,
    force_new_subdir: bool,
    max_file_mb: u64,
    timestamp_subdir: bool,
) -> Result<()> {
    let ops_tmp = portable_ops_temp(ctx)?;
    let mut out = out.to_path_buf();
    if timestamp_subdir {
        out = out.join(Utc::now().format("%Y%m%dT%H%M%SZ").to_string());
    }
    let sources = load_portable_sources();
    let mem = ctx.resolve_memory_path();
    let report = export_bundle(ExportOpts {
        out,
        force_new_subdir,
        max_file_mb,
        ensembly_version: kernel_version().to_string(),
        created_at: Utc::now(),
        stores: EnsemblyStorePaths {
            ops_json: Some(ops_tmp.clone()),
            memory_json: mem.is_file().then_some(mem),
            ledger_jsonl: default_ledger_path(ctx),
        },
        user_sources: sources,
    })
    .map_err(|e| anyhow::anyhow!(e))?;
    let _ = std::fs::remove_file(ops_tmp);
    println!(
        "PORTABLE_EXPORT_OK root={} files={} skipped_oversize={} skipped_secret={}",
        report.bundle_root.display(),
        report.files,
        report.skipped_oversize,
        report.skipped_secret
    );
    Ok(())
}

fn cmd_portable_verify(bundle: &Path) -> Result<()> {
    let report = verify_bundle(bundle);
    if report.ok {
        println!("PORTABLE_VERIFY_OK bundle={}", bundle.display());
        Ok(())
    } else {
        for e in &report.errors {
            eprintln!("PORTABLE_VERIFY_FAIL: {e}");
        }
        std::process::exit(1);
    }
}

fn resolve_into_data_dir(
    into: Option<PathBuf>,
    into_ops: Option<PathBuf>,
    into_memory: Option<PathBuf>,
) -> (Option<PathBuf>, Option<PathBuf>) {
    if let Some(dir) = into {
        let ops = dir.join(
            Path::new(DEFAULT_OPS_PATH)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("ensembly-ops.sqlite"),
        );
        let mem = dir.join(
            Path::new(DEFAULT_MEMORY_PATH)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("ensembly-memory.json"),
        );
        return (into_ops.or(Some(ops)), into_memory.or(Some(mem)));
    }
    (into_ops, into_memory)
}

fn merge_bundle_structs(
    a: &OpsBundle,
    b: &OpsBundle,
) -> Result<(OpsBundle, Vec<ensembly_bundle::KvRow>)> {
    let ea: OpsBundleMerge = serde_json::from_value(serde_json::to_value(a)?)?;
    let eb: OpsBundleMerge = serde_json::from_value(serde_json::to_value(b)?)?;
    let (merged, conflicts) = merge_ops_preserve_existing(&ea, &eb);
    let out: OpsBundle = serde_json::from_value(serde_json::to_value(merged)?)?;
    Ok((out, conflicts))
}

fn write_ops_conflicts_sidecar(ops_db: &Path, conflicts: &[ensembly_bundle::KvRow]) -> Result<()> {
    if conflicts.is_empty() {
        return Ok(());
    }
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let side = ops_db.with_extension(format!("incoming-{stamp}.jsonl"));
    let mut body = String::new();
    for row in conflicts {
        let line = serde_json::json!({
            "key": row.key,
            "value": row.value,
            "updated_at": row.updated_at,
        });
        body.push_str(&serde_json::to_string(&line)?);
        body.push('\n');
    }
    std::fs::write(&side, body)?;
    eprintln!(
        "warn: ops kv conflicts; kept existing DB {}; incoming rows → {}",
        ops_db.display(),
        side.display()
    );
    Ok(())
}

fn portable_verify_or_exit(bundle: &Path) {
    let report = verify_bundle(bundle);
    if !report.ok {
        for e in &report.errors {
            eprintln!("PORTABLE_VERIFY_FAIL: {e}");
        }
        std::process::exit(1);
    }
}

fn restore_ops_from_bundle(
    bundle: &Path,
    manifest: &BundleManifest,
    ops_db: &Path,
    merge: bool,
    dry_run: bool,
) -> Result<()> {
    let ops_json =
        manifest_file_path(bundle, manifest, MANIFEST_OPS_PATH).map_err(|e| anyhow::anyhow!(e))?;
    let incoming = OpsStore::read_bundle_file(&ops_json)?;
    if dry_run {
        return Ok(());
    }
    if ops_db.exists() && !merge {
        bail!("refusing overwrite {}; pass --merge", ops_db.display());
    }
    let store = OpsStore::open(ops_db)?;
    if merge && ops_db.exists() {
        let existing = store.export_bundle()?;
        let (merged, conflicts) = merge_bundle_structs(&existing, &incoming)?;
        store.import_bundle(&merged)?;
        write_ops_conflicts_sidecar(ops_db, &conflicts)?;
    } else {
        store.import_bundle(&incoming)?;
    }
    Ok(())
}

fn restore_memory_from_bundle(
    bundle: &Path,
    manifest: &BundleManifest,
    mem_path: &Path,
    merge: bool,
    dry_run: bool,
) -> Result<()> {
    let src = match manifest_file_path(bundle, manifest, MANIFEST_MEMORY_PATH) {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };
    if !src.is_file() {
        return Ok(());
    }
    if dry_run {
        return Ok(());
    }
    if mem_path.exists() && !merge {
        bail!("refusing overwrite {}; pass --merge", mem_path.display());
    }
    if let Some(parent) = mem_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if merge && mem_path.is_file() {
        merge_json_objects(&src, mem_path).map_err(|e| anyhow::anyhow!(e))?;
    } else {
        std::fs::copy(&src, mem_path)?;
    }
    Ok(())
}

fn restore_ledger_from_bundle(
    bundle: &Path,
    manifest: &BundleManifest,
    ledger_path: &Path,
    merge: bool,
    dry_run: bool,
) -> Result<()> {
    let src = match manifest_file_path(bundle, manifest, MANIFEST_LEDGER_PATH) {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };
    if !src.is_file() {
        return Ok(());
    }
    let incoming = std::fs::read_to_string(&src)?;
    if dry_run {
        return Ok(());
    }
    if let Some(parent) = ledger_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if merge && ledger_path.is_file() {
        let tmp = std::env::temp_dir().join(format!(
            "ensembly-ledger-in-{}.jsonl",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::write(&tmp, &incoming)?;
        merge_jsonl_records(&tmp, ledger_path).map_err(|e| anyhow::anyhow!(e))?;
        let _ = std::fs::remove_file(tmp);
    } else {
        if ledger_path.exists() && !merge {
            bail!("refusing overwrite {}; pass --merge", ledger_path.display());
        }
        std::fs::write(ledger_path, incoming)?;
    }
    Ok(())
}

fn cmd_portable_import(
    ctx: &Ctx,
    bundle: &Path,
    into: Option<PathBuf>,
    dry_run: bool,
    merge: bool,
) -> Result<()> {
    portable_verify_or_exit(bundle);
    let manifest = BundleManifest::read(bundle).map_err(|e| anyhow::anyhow!(e))?;
    if !dry_run {
        import_bundle(bundle, &ImportOpts { merge, dry_run }).map_err(|e| anyhow::anyhow!(e))?;
    }
    if let Some(dir) = into {
        let (ops, mem) = resolve_into_data_dir(Some(dir), None, None);
        if let Some(ops) = ops {
            restore_ops_from_bundle(bundle, &manifest, &ops, merge, dry_run)?;
        }
        if let Some(mem) = mem {
            restore_memory_from_bundle(bundle, &manifest, &mem, merge, dry_run)?;
        }
        let ledger = ctx
            .db_path
            .parent()
            .map(|p| p.join("applications/ledger.jsonl"));
        if let Some(led) = ledger {
            restore_ledger_from_bundle(bundle, &manifest, &led, merge, dry_run)?;
        }
    }
    println!(
        "PORTABLE_IMPORT_OK bundle={} dry_run={dry_run} merge={merge}",
        bundle.display()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_portable_restore(
    _ctx: &Ctx,
    bundle: &Path,
    into: Option<PathBuf>,
    into_ops: Option<PathBuf>,
    into_memory: Option<PathBuf>,
    sources_to: Option<PathBuf>,
    dry_run: bool,
    merge: bool,
) -> Result<()> {
    let (into_ops, into_memory) = resolve_into_data_dir(into, into_ops, into_memory);
    portable_verify_or_exit(bundle);
    let manifest = BundleManifest::read(bundle).map_err(|e| anyhow::anyhow!(e))?;
    if into_ops.is_none() && into_memory.is_none() && sources_to.is_none() {
        bail!("restore needs --into, --into-ops, --into-memory, or --sources-to");
    }
    if let Some(ref ops) = into_ops {
        restore_ops_from_bundle(bundle, &manifest, ops, merge, dry_run)?;
    }
    if let Some(ref mem) = into_memory {
        restore_memory_from_bundle(bundle, &manifest, mem, merge, dry_run)?;
    }
    let mut copies = 0usize;
    if let Some(base) = sources_to {
        let plan = restore_bundle(
            bundle,
            &RestoreOpts {
                into_ops: None,
                into_memory: None,
                sources_to: Some(base),
                merge,
                dry_run,
            },
        )
        .map_err(|e| anyhow::anyhow!(e))?;
        copies = plan.copies.len();
    }
    println!(
        "PORTABLE_RESTORE_OK bundle={} source_copies={} dry_run={dry_run}",
        bundle.display(),
        copies
    );
    Ok(())
}

fn bundle_export(db_path: &PathBuf, out: &PathBuf, json: bool) -> Result<()> {
    let store = OpsStore::open(db_path)?;
    let bundle = store.export_bundle()?;
    OpsStore::write_bundle_file(out, &bundle)?;
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
        return print_json(&body);
    }
    println!(
        "OPS_BUNDLE_EXPORT_OK path={out:?} keys={} schema={}",
        bundle.kv.len(),
        bundle.schema_version
    );
    Ok(())
}

fn bundle_import_dry(bundle: &PathBuf, json: bool) -> Result<()> {
    let read = OpsStore::read_bundle_file(bundle)?;
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
        return print_json(&body);
    }
    println!(
        "OPS_BUNDLE_IMPORT_DRY_RUN_OK keys={} (primary not modified)",
        read.kv.len()
    );
    Ok(())
}

fn bundle_import(db_path: &PathBuf, bundle: &PathBuf, json: bool) -> Result<()> {
    let read = OpsStore::read_bundle_file(bundle)?;
    let store = OpsStore::open(db_path)?;
    store.import_bundle(&read)?;
    let body = serde_json::json!({
        "ok": true,
        "cmd": "ops-bundle.import",
        "dry_run": false,
        "kv_keys": read.kv.len(),
        "db": store.path(),
    });
    if json {
        print_json(&body)?;
    } else {
        println!(
            "OPS_BUNDLE_IMPORT_OK db={:?} keys={}",
            store.path(),
            read.kv.len()
        );
    }
    eprintln!("OPS_BUNDLE_OK import keys={}", read.kv.len());
    Ok(())
}

fn rfc3339_or_dash(t: Option<chrono::DateTime<Utc>>) -> String {
    t.map(|t| t.to_rfc3339()).unwrap_or_else(|| "-".into())
}

fn cmd_pulse_pack(ctx: &Ctx, sub: PulsePackCmd) -> Result<()> {
    let memory_path = ctx.resolve_memory_path();
    let archive_sidecar = PathBuf::from(DEFAULT_ARCHIVE_SIDECAR);
    match sub {
        PulsePackCmd::Export {
            out,
            include_archive,
            archive_limit,
            json,
        } => {
            let ops_db = include_archive.then(|| ctx.db_path.clone());
            pp_export(
                memory_path,
                archive_sidecar,
                ops_db,
                archive_limit,
                &out,
                json,
            )
        }
        PulsePackCmd::Import {
            pack,
            dry_run,
            archive_sidecar: archive_path,
            json,
        } => {
            let opts = PulseImportOpts {
                memory_path,
                archive_sidecar: archive_path.unwrap_or(archive_sidecar),
                dry_run,
            };
            pp_import(&pack, &opts, json)
        }
        PulsePackCmd::Status {
            pack,
            archive_sidecar: archive_path,
            json,
        } => pp_status(
            pack,
            &memory_path,
            &archive_path.unwrap_or(archive_sidecar),
            json,
        ),
    }
}

fn pp_export(
    memory_path: PathBuf,
    archive_sidecar: PathBuf,
    ops_db: Option<PathBuf>,
    archive_limit: usize,
    out: &PathBuf,
    json: bool,
) -> Result<()> {
    let pack = export_pulse_pack(&PulseExportOpts {
        memory_path,
        archive_sidecar,
        ops_db,
        archive_limit,
        source_host: std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "unknown".into()),
    })?;
    write_pulse_pack(out, &pack)?;
    let status = pack.status();
    if json {
        print_json(&status)?;
    } else {
        println!(
            "PULSE_EXPORT_OK path={out:?} traces={} archive={} hash={} last_seen={}",
            status.memory_trace_count,
            status.archive_event_count,
            status.pack_hash,
            rfc3339_or_dash(status.last_seen)
        );
    }
    eprintln!(
        "PULSE_OK export traces={} archive={}",
        status.memory_trace_count, status.archive_event_count
    );
    Ok(())
}

fn pp_import(pack: &PathBuf, opts: &PulseImportOpts, json: bool) -> Result<()> {
    let read = read_pulse_pack(pack)?;
    let report = import_pulse_pack(&read, opts)?;
    if json {
        print_json(&report)?;
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
        report.dry_run, report.memory_trajectory_after
    );
    Ok(())
}

fn pp_status(
    pack: Option<PathBuf>,
    memory_path: &std::path::Path,
    sidecar: &std::path::Path,
    json: bool,
) -> Result<()> {
    let from_pack = pack.is_some();
    let status = match pack {
        Some(p) => read_pulse_pack(&p)?.status(),
        None => local_pulse_status(memory_path, sidecar)?,
    };
    if json {
        return print_json(&status);
    }
    println!(
        "# Pulse status — {}",
        if from_pack { "pack" } else { "local" }
    );
    println!("format: {}", status.format);
    println!("traces: {}", status.memory_trace_count);
    println!("archive: {}", status.archive_event_count);
    println!("last_seen: {}", rfc3339_or_dash(status.last_seen));
    println!("hash: {}", status.pack_hash);
    if let Some(h) = &status.memory_hash {
        println!("memory_hash: {}", h);
    }
    Ok(())
}

fn cmd_runtime(ctx: &Ctx, sub: RuntimeCmd) -> Result<()> {
    let store = OpsStore::open(&ctx.db_path)?;
    let now = Utc::now();
    let mut rt = Runtime::new(now);
    if let Some(life) = store.load_life_state()? {
        rt.state = life;
    }
    if let Some(snap) = store.load_snapshot()? {
        rt.snapshot = snap;
    }
    attach_memory(&mut rt, &ctx.memory_flags, &ctx.data_paths);
    let mut r = Rt { store, rt, now };
    match sub {
        RuntimeCmd::Load { fixture, json } => runtime_load(&mut r, &fixture, json),
        RuntimeCmd::Status { json } => runtime_status(&r, json),
        RuntimeCmd::Tick { agent, json } => runtime_tick(&mut r, agent, json),
        RuntimeCmd::Approve { id, json } => runtime_approve(&mut r, &id, json),
        RuntimeCmd::Deny { id, json } => runtime_deny(&mut r, &id, json),
        RuntimeCmd::Claim { id, json } => runtime_claim(&mut r, id, json),
        RuntimeCmd::Complete { id, json } => runtime_complete(&mut r, id, json),
        RuntimeCmd::Reflect { json } => runtime_reflect(&mut r.rt, json),
        RuntimeCmd::Dive { probe_budget, json } => runtime_dive(&r.rt, probe_budget, json),
    }
}

/// An opened runtime with its store, for the `runtime` subcommands.
struct Rt {
    store: OpsStore,
    rt: Runtime,
    now: chrono::DateTime<Utc>,
}

impl Rt {
    /// Persist the runtime pair, then memory (same order as every mutating subcommand).
    fn persist(&mut self) -> Result<()> {
        self.store
            .save_runtime_pair(&self.rt.state, &self.rt.snapshot)?;
        save_memory(&mut self.rt);
        Ok(())
    }

    /// Enqueue one manual command, tick without the agent, persist.
    fn manual(&mut self, cmd: ManualCmd) -> Result<ensembly_kernel::runtime::TickReport> {
        self.rt.enqueue_manual(cmd, self.now);
        let report = self.rt.tick(false, self.now)?;
        self.persist()?;
        Ok(report)
    }
}

fn runtime_load(r: &mut Rt, fixture: &std::path::Path, json: bool) -> Result<()> {
    let actions = load_actions_from_fixture(fixture)?;
    r.rt.load_actions(&actions, r.now)?;
    r.persist()?;
    let rt = &r.rt;
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
        "db": r.store.path(),
    });
    if json {
        print_json(&body)?;
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
    Ok(())
}

fn runtime_status(r: &Rt, json: bool) -> Result<()> {
    let rt = &r.rt;
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
        "db": r.store.path(),
    });
    if json {
        return print_json(&body);
    }
    println!("# LifeState v{}", rt.state.version);
    println!("regime: {:?}", rt.state.regime);
    println!(
        "CP: {}",
        cp.map(|c| c.explain.as_str())
            .unwrap_or("(none — run runtime load)")
    );
    println!(
        "metrics: hootl_done={} hitl_surfaces={} agent_failures={}",
        rt.state.metrics.hootl_completed,
        rt.state.metrics.hitl_surfaces,
        rt.state.metrics.agent_failures
    );
    Ok(())
}

fn runtime_tick(r: &mut Rt, agent: bool, json: bool) -> Result<()> {
    let report = r.rt.tick(agent, r.now)?;
    r.persist()?;
    if json {
        print_json(&report)?;
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
    Ok(())
}

fn runtime_approve(r: &mut Rt, id: &str, json: bool) -> Result<()> {
    // Normalize: approve uses action id (pay-rent); auth- prefix accepted then stripped.
    let action_id = ensembly_kernel::runtime::action_id_of(id).to_string();
    let report = r.manual(ManualCmd::Approve {
        id: action_id.clone(),
    })?;
    if json {
        return print_json(
            &serde_json::json!({ "ok": true, "decision": "approve", "id": action_id, "tick": report }),
        );
    }
    println!("RUNTIME_APPROVE id={action_id} regime={:?}", report.regime);
    Ok(())
}

fn runtime_deny(r: &mut Rt, id: &str, json: bool) -> Result<()> {
    let action_id = ensembly_kernel::runtime::action_id_of(id).to_string();
    let report = r.manual(ManualCmd::Deny {
        id: action_id.clone(),
    })?;
    if json {
        return print_json(
            &serde_json::json!({ "ok": true, "decision": "deny", "id": action_id, "tick": report }),
        );
    }
    println!("RUNTIME_DENY id={action_id}");
    Ok(())
}

fn runtime_claim(r: &mut Rt, id: String, json: bool) -> Result<()> {
    let report = r.manual(ManualCmd::ClaimPhysical { id: id.clone() })?;
    if json {
        return print_json(
            &serde_json::json!({ "ok": true, "decision": "claim", "id": id, "tick": report }),
        );
    }
    println!("RUNTIME_CLAIM id={id}");
    Ok(())
}

fn runtime_complete(r: &mut Rt, id: String, json: bool) -> Result<()> {
    let report = r.manual(ManualCmd::CompletePhysical { id: id.clone() })?;
    if json {
        return print_json(
            &serde_json::json!({ "ok": true, "decision": "complete", "id": id, "tick": report }),
        );
    }
    println!("RUNTIME_COMPLETE id={id} regime={:?}", report.regime);
    Ok(())
}

fn runtime_reflect(rt: &mut Runtime, json: bool) -> Result<()> {
    let Some(sink) = rt.memory.as_mut() else {
        bail!("reflect needs episodic memory — remove --no-memory (default path: {DEFAULT_MEMORY_PATH})");
    };
    let entries = sink.memory.doc().trajectory.len();
    let Some(r) = sink.reflect() else {
        let msg = format!(
            "not enough trajectory to reflect (have {entries}, need >= 5) — run runtime load/tick first"
        );
        if json {
            return print_json(&serde_json::json!({ "ok": false, "reason": msg }));
        }
        println!("REFLECT_SKIP {msg}");
        return Ok(());
    };
    save_memory(rt);
    if json {
        print_json(&r)?;
    } else {
        println!(
            "REFLECT coherence={:.1}% entries={} skills={} new_skills={:?}",
            r.coherence * 100.0,
            r.trajectory_length,
            r.known_skills,
            r.new_skills
        );
        for p in &r.goal_proposals {
            println!(
                "  proposal [{}] {} (priority {:.1})",
                p.goal_type, p.description, p.priority
            );
        }
    }
    eprintln!(
        "REFLECT_OK coherence={:.2} skills={} proposals={}",
        r.coherence,
        r.known_skills,
        r.goal_proposals.len()
    );
    Ok(())
}

fn print_dive_text(dive: &ensembly_kernel::DiveReport) {
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
    match &dive.next_probe {
        Some(p) => println!(
            "next probe: {} (score={:.1}) — {}",
            p.id, p.uncertainty_score, p.reason
        ),
        None => println!("next probe: (none)"),
    }
    println!(
        "guards: budget={} refuse_auto_auth={} claim_via_cp={}",
        dive.trauma_guards.probe_budget,
        dive.trauma_guards.refuse_auto_auth,
        dive.trauma_guards.claim_via_cp_only
    );
}

fn runtime_dive(rt: &Runtime, probe_budget: u32, json: bool) -> Result<()> {
    use ensembly_kernel::critical_path::compute_critical_path;
    use ensembly_kernel::plan_dive;
    if rt.state.graph.nodes.is_empty() {
        bail!("runtime dive needs a loaded graph — run `runtime load --fixture …` first");
    }
    let cp = match rt.state.critical_path.clone() {
        Some(c) => c,
        None => compute_critical_path(&rt.state.graph, 0).map_err(|e| anyhow::anyhow!(e))?,
    };
    let dive = plan_dive(&rt.state.graph, &cp, probe_budget);
    if json {
        print_json(&dive)?;
    } else {
        print_dive_text(&dive);
    }
    eprintln!(
        "RUNTIME_OK dive candidates={} probe={}",
        dive.candidates.len(),
        dive.next_probe
            .as_ref()
            .map(|p| p.id.as_str())
            .unwrap_or("-")
    );
    Ok(())
}

fn cmd_channel_pulse(db_path: &PathBuf, sub: ChannelPulseCmd) -> Result<()> {
    let ChannelPulseCmd::Reconcile {
        out,
        fixture,
        location,
        json,
        verbose,
    } = sub;
    let store = OpsStore::open(db_path)?;
    // Prefer durable live seed; --fixture only for isolated CI dogfood.
    // Empty ops → CHANNEL_IR_FAIL (non-zero), never silent grocery projection.
    let actions = match fixture {
        Some(f) => load_actions_from_fixture(&f)?,
        None => seed_live_channel_actions(&store)?,
    };
    let pulse_path = out.unwrap_or_else(|| PathBuf::from(DEFAULT_CHANNEL_PULSE_PATH));
    let report = reconcile_channel_pulse(&store, &actions, location.as_deref(), &pulse_path)?;
    if json {
        print_json(&report)?;
    } else if verbose && report.wrote {
        eprintln!(
            "CHANNEL_PULSE_OK wrote={} path={:?} hash={}",
            report.wrote, report.pulse_path, report.content_hash
        );
    }
    Ok(())
}

/// Top-level approve/deny/claim/complete always go through Runtime (G + Snapshot).
/// Requires durable life_state from `ensembly runtime load` — no snapshot-only legacy path.
fn gate_via_runtime(
    db_path: &PathBuf,
    memory_flags: &MemoryFlags,
    data_paths: &DataPaths,
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
    attach_memory(&mut rt, memory_flags, data_paths);
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
