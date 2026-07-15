//! `peram` CLI — dogfood entry for the Rust life kernel.

use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use peram_kernel::approvals::{
    apply_decision, apply_physical_decision, list_pending, upsert_pending_from_actions,
    upsert_physical, Snapshot,
};
use peram_kernel::backup::{
    create_backup_pack, read_backup_pack, restore_dry_run, write_backup_pack,
};
use peram_kernel::digital_flow::{run_cycle, DigitalFlow};
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
                    },
                ]
            };
            let snap = ensure_snap(&store, &actions)?;
            let plan = rank_now(
                &context_at(Utc::now(), location.as_deref()),
                &actions,
                &[],
                &snap,
            );
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
    }
    Ok(())
}
