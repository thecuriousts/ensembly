//! `peram` CLI — dogfood entry for the Rust life kernel.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use peram_kernel::approvals::{
    apply_decision, apply_physical_decision, list_pending, upsert_pending_from_actions,
    upsert_physical, Snapshot,
};
use peram_kernel::backup::{
    create_backup_pack, read_backup_pack, restore_dry_run, write_backup_pack,
};
use peram_kernel::digital_flow::{run_cycle, DigitalFlow};
use peram_kernel::msg_bus::ManualCmd;
use peram_kernel::runtime::Runtime;
use peram_kernel::store::OpsStore;
use peram_kernel::turn::{context_at, rank_now, Action};
use peram_kernel::{kernel_version, private_path_patterns};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "peram", about = "Game of Peram life kernel (Rust)")]
struct Cli {
    /// Ops SQLite path (T1). Default: data/local/peram-ops.sqlite under cwd/repo.
    #[arg(long, global = true)]
    db: Option<PathBuf>,

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
    /// Create sealed backup pack of T1 ops
    Backup {
        #[arg(long)]
        out: PathBuf,
        /// Unlock material (demo CLI; production → keyring). Env PERAM_UNLOCK overrides.
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
    /// Issue #1 control plane: S+G+CP, MsgBus, HITL/HOOTL tick
    Runtime {
        #[command(subcommand)]
        sub: RuntimeCmd,
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

fn default_db() -> PathBuf {
    // Prefer repo data/local when present
    let candidates = [
        PathBuf::from("data/local/peram-ops.sqlite"),
        PathBuf::from("private/state/peram-ops.sqlite"),
    ];
    for c in candidates {
        if c.parent().map(|p| p.exists()).unwrap_or(false) || c.exists() {
            return c;
        }
    }
    PathBuf::from("data/local/peram-ops.sqlite")
}

fn unlock_material(cli: &Option<String>) -> Result<Vec<u8>> {
    if let Ok(v) = std::env::var("PERAM_UNLOCK") {
        if !v.is_empty() {
            return Ok(v.into_bytes());
        }
    }
    if let Some(u) = cli {
        return Ok(u.clone().into_bytes());
    }
    bail!("unlock required: pass --unlock or set PERAM_UNLOCK (keyring later)");
}

fn load_actions_from_fixture(path: &PathBuf) -> Result<Vec<Action>> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {path:?}"))?;
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    let mut actions = vec![];
    if let Some(arr) = v.get("extra_candidates").and_then(|x| x.as_array()) {
        for item in arr {
            actions.push(Action {
                id: item
                    .get("id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("unknown")
                    .into(),
                title: item
                    .get("title")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .into(),
                area: item
                    .get("area")
                    .and_then(|x| x.as_str())
                    .map(str::to_string),
                kind: item
                    .get("kind")
                    .and_then(|x| x.as_str())
                    .map(str::to_string),
                realm: item
                    .get("realm")
                    .and_then(|x| x.as_str())
                    .map(str::to_string),
                urgency: item.get("urgency").and_then(|x| x.as_i64()).unwrap_or(2) as i32,
                importance: item
                    .get("importance")
                    .and_then(|x| x.as_i64())
                    .unwrap_or(2) as i32,
                tags: item
                    .get("tags")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|t| t.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                public: item.get("public").and_then(|x| x.as_bool()),
                depends_on: item.get("depends_on").and_then(|x| x.as_array()).map(|a| {
                    a.iter()
                        .filter_map(|t| t.as_str().map(str::to_string))
                        .collect()
                }),
                deadline_at: item
                    .get("deadline_at")
                    .and_then(|x| x.as_str())
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.with_timezone(&Utc)),
            });
        }
    }
    Ok(actions)
}

fn ensure_snap(store: &OpsStore, actions: &[Action]) -> Result<Snapshot> {
    let now = Utc::now();
    let existing = store.load_snapshot()?;
    let hitl: Vec<_> = actions
        .iter()
        .map(|a| {
            let hitl = matches!(
                a.kind.as_deref(),
                Some("job_application_submit")
                    | Some("finance_transfer")
                    | Some("external_email_send")
                    | Some("calendar_mutate")
                    | Some("git_push_shared")
                    | Some("publish_private_data")
            ) || a.area.as_deref() == Some("Finance");
            (
                a.id.clone(),
                a.title.clone(),
                a.kind.clone().unwrap_or_else(|| "hitl".into()),
                hitl,
            )
        })
        .collect();
    let mut snap = upsert_pending_from_actions(&hitl, existing, now);
    let physical: Vec<_> = actions
        .iter()
        .filter(|a| a.realm.as_deref() == Some("physical") || a.kind.as_deref() == Some("physical_errand") || a.kind.as_deref() == Some("outdoor"))
        .map(|a| (a.id.clone(), a.title.clone()))
        .collect();
    snap = upsert_physical(&physical, Some(snap), now);
    store.save_snapshot(&snap)?;
    Ok(snap)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = cli.db.clone().unwrap_or_else(default_db);

    match cli.cmd {
        Commands::Version => {
            println!("{}", kernel_version());
            println!("private_paths: {:?}", private_path_patterns());
            println!("law: Node src/* legacy; peram-kernel is control SoT");
        }
        Commands::Turn {
            fixture,
            json,
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
            // When durable life-state exists, drive FocusPlan from CP.
            if let Ok(Some(life)) = store.load_life_state() {
                let now = Utc::now();
                let mut rt = Runtime::new(now);
                rt.state = life;
                rt.snapshot = snap.clone();
                plan = rt.focus_plan(plan);
            } else if peram_kernel::DepGraph::from_actions(&actions, &Default::default()).is_ok() {
                let now = Utc::now();
                let mut rt = Runtime::new(now);
                if rt.load_actions(&actions, now).is_ok() {
                    plan = rt.focus_plan(plan);
                    let _ = store.save_life_state(&rt.state);
                    let _ = store.save_snapshot(&rt.snapshot);
                }
            }
            if json {
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
        Commands::Approve { id } => {
            let store = OpsStore::open(&db_path)?;
            let snap = store
                .load_snapshot()?
                .unwrap_or_else(|| Snapshot::empty(Utc::now()));
            let next = apply_decision(&snap, &id, "approve", "operator", Utc::now())?;
            store.save_snapshot(&next)?;
            println!(
                "{}",
                serde_json::json!({
                    "ok": true,
                    "decision": "approve",
                    "id": id,
                    "status": format!("{:?}", next.status),
                    "pendingRemaining": list_pending(&next).iter().map(|p| &p.id).collect::<Vec<_>>(),
                })
            );
        }
        Commands::Deny { id } => {
            let store = OpsStore::open(&db_path)?;
            let snap = store
                .load_snapshot()?
                .unwrap_or_else(|| Snapshot::empty(Utc::now()));
            let next = apply_decision(&snap, &id, "deny", "operator", Utc::now())?;
            store.save_snapshot(&next)?;
            println!(
                "{}",
                serde_json::json!({ "ok": true, "decision": "deny", "id": id })
            );
        }
        Commands::Claim { id } => {
            let store = OpsStore::open(&db_path)?;
            let snap = store
                .load_snapshot()?
                .unwrap_or_else(|| Snapshot::empty(Utc::now()));
            let next = apply_physical_decision(&snap, &id, "claim", Utc::now())?;
            store.save_snapshot(&next)?;
            println!("{}", serde_json::json!({ "ok": true, "decision": "claim", "id": id }));
        }
        Commands::Complete { id } => {
            let store = OpsStore::open(&db_path)?;
            let snap = store
                .load_snapshot()?
                .unwrap_or_else(|| Snapshot::empty(Utc::now()));
            let next = apply_physical_decision(&snap, &id, "complete", Utc::now())?;
            store.save_snapshot(&next)?;
            println!(
                "{}",
                serde_json::json!({ "ok": true, "decision": "complete", "id": id })
            );
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
                snap.status = peram_kernel::derive_status(&snap.pending);
                snap.phase = match snap.status {
                    peram_kernel::SnapshotStatus::IdleWaiting => "HITL_WAIT".into(),
                    peram_kernel::SnapshotStatus::Clear => "CLEAR".into(),
                    peram_kernel::SnapshotStatus::Partial => "PARTIAL".into(),
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

            match sub {
                RuntimeCmd::Load { fixture, json } => {
                    let actions = load_actions_from_fixture(&fixture)?;
                    rt.load_actions(&actions, now)?;
                    store.save_life_state(&rt.state)?;
                    store.save_snapshot(&rt.snapshot)?;
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
                            "metrics: correctness={} effectiveness={} efficiency={} hootl_done={}",
                            rt.state.metrics.correctness_events,
                            rt.state.metrics.effectiveness_events,
                            rt.state.metrics.efficiency_events,
                            rt.state.metrics.hootl_completed
                        );
                    }
                }
                RuntimeCmd::Tick { agent, json } => {
                    let report = rt.tick(agent, now)?;
                    store.save_life_state(&rt.state)?;
                    store.save_snapshot(&rt.snapshot)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    } else {
                        println!(
                            "RUNTIME_TICK regime={:?} drained={} hootl={:?} auth={:?} physical={:?} {}",
                            report.regime,
                            report.messages_drained,
                            report.hootl_claim,
                            report.next_auth,
                            report.next_physical,
                            report.cp_explain
                        );
                    }
                    eprintln!(
                        "RUNTIME_OK tick regime={:?} hootl={} auth={}",
                        report.regime,
                        report.hootl_claim.as_deref().unwrap_or("-"),
                        report.next_auth.as_deref().unwrap_or("-")
                    );
                }
                RuntimeCmd::Approve { id, json } => {
                    rt.enqueue_manual(ManualCmd::Approve { id: id.clone() }, now);
                    let report = rt.tick(false, now)?;
                    store.save_life_state(&rt.state)?;
                    store.save_snapshot(&rt.snapshot)?;
                    let body = serde_json::json!({ "ok": true, "decision": "approve", "id": id, "tick": report });
                    if json {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    } else {
                        println!("RUNTIME_APPROVE id={id} regime={:?}", report.regime);
                    }
                }
                RuntimeCmd::Deny { id, json } => {
                    rt.enqueue_manual(ManualCmd::Deny { id: id.clone() }, now);
                    let report = rt.tick(false, now)?;
                    store.save_life_state(&rt.state)?;
                    store.save_snapshot(&rt.snapshot)?;
                    let body = serde_json::json!({ "ok": true, "decision": "deny", "id": id, "tick": report });
                    if json {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    } else {
                        println!("RUNTIME_DENY id={id}");
                    }
                }
                RuntimeCmd::Claim { id, json } => {
                    rt.enqueue_manual(ManualCmd::ClaimPhysical { id: id.clone() }, now);
                    let report = rt.tick(false, now)?;
                    store.save_life_state(&rt.state)?;
                    store.save_snapshot(&rt.snapshot)?;
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
                    store.save_life_state(&rt.state)?;
                    store.save_snapshot(&rt.snapshot)?;
                    let body = serde_json::json!({ "ok": true, "decision": "complete", "id": id, "tick": report });
                    if json {
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    } else {
                        println!("RUNTIME_COMPLETE id={id} regime={:?}", report.regime);
                    }
                }
            }
        }
    }
    Ok(())
}
