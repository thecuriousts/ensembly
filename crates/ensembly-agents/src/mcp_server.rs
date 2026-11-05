//! Model Context Protocol server for episodic memory and record Commit/View.
//! Primary consumer: Grok (via `grok mcp add` / `.grok/config.toml` or Cursor MCP config).
//! Record tools: `record_commit`, `record_view_snapshot`, `record_view_feed`.
//! Reconcile and Sync tools are deferred.
//! Memory tools are unchanged. `record_commit` writes only a sandbox temp path, `data/local` under `ENSEMBLY_HOME` or the server cwd, or `ENSEMBLY_RECORD_LOG`.
//! MCP commit refuses source `human` and `owner`, and stamps `record_time`.

use ensembly_memory::{coherence_report, propose_goals, EpisodicMemory, TrajectoryType};
use ensembly_schema::{commit_json, feed_json, snapshot_json, CommitFault};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

use crate::rpc::{rpc_error, serve_stdio, DispatchResult};

pub const PROTOCOL_VERSION: &str = "2024-11-05";

fn tool_defs() -> Value {
    json!([
        {
            "name": "memory_recent_trajectory",
            "description": "Recent episodic trajectory entries (observations, actions, reflections). Read-only.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "hours": {"type": "integer", "default": 24},
                    "limit": {"type": "integer", "default": 40}
                }
            }
        },
        {
            "name": "memory_get_report",
            "description": "Coherence report: current/average/trend, skill count, trajectory length.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "memory_list_skills",
            "description": "List synthesized skills (recurring action patterns).",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "memory_propose_goals",
            "description": "Goal proposals from current memory (deterministic).",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "memory_get_context",
            "description": "Read a context key from the CRDT document.",
            "inputSchema": {
                "type": "object",
                "properties": { "key": {"type": "string"} },
                "required": ["key"]
            }
        },
        {
            "name": "swarm_banner",
            "description": "ensembly-agents / ensembly-memory version banner and paths.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "record_commit",
            "description": "Append one record unit. The log must be ENSEMBLY_RECORD_LOG, data/local under ENSEMBLY_HOME or the server cwd, or the sandbox temp dir. Source human and owner are refused. record_time is stamped by the server.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "log": {"type": "string", "description": "Event log path. Defaults to ENSEMBLY_RECORD_LOG."},
                    "unit": {"description": "Unit JSON: an event array, an object with events, or a string of that JSON."}
                },
                "required": ["unit"]
            }
        },
        {
            "name": "record_view_snapshot",
            "description": "Read-only snapshot of a record log. view_append stays refused.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "log": {"type": "string"},
                    "at": {"type": "integer", "description": "Version. Omit for the newest version in the log."}
                }
            }
        },
        {
            "name": "record_view_feed",
            "description": "Read-only change feed of events after a version.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "log": {"type": "string"},
                    "since": {"type": "integer"}
                },
                "required": ["since"]
            }
        }
    ])
}

fn text_content(payload: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": payload.to_string() }]
    })
}

fn invalid_params(msg: &str) -> Value {
    rpc_error(-32602, &format!("invalid params: {msg}"))
}

/// Accept JSON integers or floats (hosts often send `5.0`).
fn json_i64(v: Option<&Value>, default: i64) -> i64 {
    let Some(v) = v else {
        return default;
    };
    v.as_i64()
        .or_else(|| v.as_u64().map(|n| n as i64))
        .or_else(|| v.as_f64().map(|n| n as i64))
        .unwrap_or(default)
}

fn json_usize(v: Option<&Value>, default: usize) -> usize {
    let Some(v) = v else {
        return default;
    };
    v.as_u64()
        .map(|n| n as usize)
        .or_else(|| v.as_i64().filter(|&n| n >= 0).map(|n| n as usize))
        .or_else(|| v.as_f64().filter(|&n| n >= 0.0).map(|n| n as usize))
        .unwrap_or(default)
}

pub struct McpServeConfig {
    pub memory_path: PathBuf,
    pub agent_id: String,
}

impl Default for McpServeConfig {
    fn default() -> Self {
        Self {
            memory_path: crate::env_alias::env_alias("ENSEMBLY_MEMORY", "PERAM_MEMORY")
                .map(PathBuf::from)
                .unwrap_or_else(|| crate::env_alias::resolve_memory_path(None)),
            agent_id: crate::env_alias::resolve_agent_id(None),
        }
    }
}

/// Run the MCP server until stdin EOF.
pub fn serve(config: McpServeConfig) -> anyhow::Result<()> {
    let memory_path = config.memory_path.clone();
    let agent_id = config.agent_id.clone();

    serve_stdio(move |method, params| -> DispatchResult {
        match method.as_str() {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "ensembly-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            })),
            "notifications/initialized" | "initialized" => Ok(Value::Null),
            "tools/list" => Ok(json!({ "tools": tool_defs() })),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid_params("missing name"))?;
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                call_tool(name, &args, &memory_path, &agent_id)
            }
            "ping" => Ok(json!({})),
            other => Err(rpc_error(-32601, &format!("method not found: {other}"))),
        }
    })?;
    Ok(())
}

fn open_mem(path: &PathBuf, agent_id: &str) -> Result<EpisodicMemory, Value> {
    // Fail closed: never create an empty doc for read-only MCP (wrong cwd footgun).
    EpisodicMemory::open_existing(path, agent_id)
        .map_err(|e| rpc_error(-32000, &format!("open memory {path:?}: {e}")))
}

fn record_log_from_args(args: &Value) -> Result<PathBuf, Value> {
    let explicit = args
        .get("log")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    let env = std::env::var("ENSEMBLY_RECORD_LOG")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    permit_record_log(
        explicit.as_deref(),
        env.as_deref(),
        &std::env::temp_dir(),
        &record_anchor(),
    )
    .map_err(|e| rpc_error(-32000, &e))
}

fn record_anchor() -> PathBuf {
    static ANCHOR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ANCHOR
        .get_or_init(|| {
            if let Some(home) = std::env::var_os("ENSEMBLY_HOME").filter(|s| !s.is_empty()) {
                return PathBuf::from(home);
            }
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        })
        .clone()
}

fn permit_record_log(
    explicit: Option<&Path>,
    env_log: Option<&Path>,
    temp_root: &Path,
    anchor: &Path,
) -> Result<PathBuf, String> {
    let chosen = match (explicit, env_log) {
        (Some(path), _) => path.to_path_buf(),
        (None, Some(path)) => path.to_path_buf(),
        (None, None) => {
            return Err("record log path required (ENSEMBLY_RECORD_LOG or log)".into());
        }
    };
    let allowed = env_log.is_some_and(|env| same_path(&chosen, env))
        || under_temp(&chosen, temp_root)
        || under_data_local(&chosen, anchor);
    if allowed {
        Ok(chosen)
    } else {
        Err(
            "record commit refuses paths outside a sandbox temp dir, data/local, or ENSEMBLY_RECORD_LOG"
                .into(),
        )
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path(left) == normalize_path(right)
}

fn under_temp(path: &Path, temp_root: &Path) -> bool {
    normalize_path(path).starts_with(normalize_path(temp_root))
}

fn under_data_local(path: &Path, anchor: &Path) -> bool {
    let root = normalize_path(anchor).join("data").join("local");
    normalize_path(path).starts_with(&root)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut acc = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                acc.pop();
            }
            Component::RootDir | Component::Prefix(_) => acc.push(component),
            Component::Normal(seg) => {
                acc.push(seg);
                if acc.exists() {
                    if let Ok(canon) = acc.canonicalize() {
                        acc = canon;
                    }
                }
            }
        }
    }
    acc
}

fn prepare_mcp_unit(raw: &str) -> Result<String, Value> {
    let mut value: Value = serde_json::from_str(raw).map_err(|e| invalid_params(&e.to_string()))?;
    let events = match &mut value {
        Value::Array(items) => items,
        Value::Object(map) => match map.get_mut("events") {
            Some(Value::Array(items)) => items,
            Some(_) | None => {
                return Err(invalid_params(
                    "unit must be an event array or an object with events",
                ))
            }
        },
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            return Err(invalid_params(
                "unit must be an event array or an object with events",
            ))
        }
    };
    let record_time = ensembly_schema::stockholm_record_now();
    for event in events.iter_mut() {
        let Some(obj) = event.as_object_mut() else {
            return Err(invalid_params("event must be an object"));
        };
        match obj.get("source").and_then(Value::as_str) {
            Some("human" | "owner") => {
                return Err(rpc_error(
                    -32000,
                    "record commit refuses source human or owner",
                ));
            }
            Some(_) => {}
            None => return Err(invalid_params("source required")),
        }
        obj.insert(
            "record_time".to_string(),
            Value::String(record_time.clone()),
        );
    }
    serde_json::to_string(&value).map_err(|e| rpc_error(-32000, &e.to_string()))
}

fn unit_json(args: &Value) -> Result<String, Value> {
    match args.get("unit") {
        None | Some(Value::Null) => Err(invalid_params("unit required")),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(other) => serde_json::to_string(other).map_err(|e| invalid_params(&e.to_string())),
    }
}

fn required_u64(args: &Value, key: &str) -> Result<u64, Value> {
    match json_u64_opt(args.get(key))? {
        Some(n) => Ok(n),
        None => Err(invalid_params(&format!("{key} required"))),
    }
}

fn json_u64_opt(v: Option<&Value>) -> Result<Option<u64>, Value> {
    let Some(v) = v else {
        return Ok(None);
    };
    if v.is_null() {
        return Ok(None);
    }
    v.as_u64()
        .or_else(|| v.as_i64().and_then(|n| u64::try_from(n).ok()))
        .or_else(|| {
            v.as_f64()
                .filter(|n| n.is_finite() && *n >= 0.0)
                .map(|n| n as u64)
        })
        .map(Some)
        .ok_or_else(|| invalid_params("version must be a non-negative integer"))
}

fn call_tool(name: &str, args: &Value, memory_path: &PathBuf, agent_id: &str) -> DispatchResult {
    match name {
        "swarm_banner" => Ok(text_content(json!({
            "ok": true,
            "agents": crate::agents_version(),
            "memory": ensembly_memory::memory_version(),
            "memoryPath": memory_path,
            "agentId": agent_id,
            "note": "Control SoT remains the kernel. Memory tools are read-only. record_commit writes only a sandbox temp path, data/local under ENSEMBLY_HOME or the server cwd, or ENSEMBLY_RECORD_LOG. Source human and owner are refused.",
            "grok": {
                "acp": "grok agent stdio",
                "headless": "grok -p \"…\" --output-format json --no-auto-update",
                "registerMcp": crate::grok::register_mcp_shell_hint("./target/debug/ensembly-mcp"),
                "projectToml": crate::grok::project_mcp_toml_snippet("./target/debug/ensembly-mcp", &[]),
                "docs": [
                    "https://docs.x.ai/build/cli/headless-scripting#acp",
                    "https://docs.x.ai/build/features/mcp-servers"
                ]
            }
        }))),
        "memory_get_report" => {
            let mem = open_mem(memory_path, agent_id)?;
            let report = coherence_report(mem.doc());
            Ok(text_content(
                serde_json::to_value(report).unwrap_or_default(),
            ))
        }
        "memory_list_skills" => {
            let mem = open_mem(memory_path, agent_id)?;
            let skills: Vec<_> = mem
                .doc()
                .skills
                .values()
                .map(|s| {
                    json!({
                        "name": s.name,
                        "pattern": s.pattern,
                        "confidence": s.confidence,
                        "usage_count": s.usage_count,
                        "version": s.version,
                    })
                })
                .collect();
            Ok(text_content(json!({ "skills": skills })))
        }
        "memory_propose_goals" => {
            let mem = open_mem(memory_path, agent_id)?;
            let proposals = propose_goals(mem.doc());
            Ok(text_content(json!({ "proposals": proposals })))
        }
        "memory_get_context" => {
            let key = args
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_params("key required"))?;
            let mem = open_mem(memory_path, agent_id)?;
            Ok(text_content(json!({
                "key": key,
                "value": mem.doc().get_context(key),
            })))
        }
        "record_commit" => {
            let log = record_log_from_args(args)?;
            let unit = prepare_mcp_unit(&unit_json(args)?)?;
            let result = commit_json(&log, &unit, &CommitFault::default())
                .map_err(|e| rpc_error(-32000, &e.to_string()))?;
            let body =
                serde_json::to_value(&result).map_err(|e| rpc_error(-32000, &e.to_string()))?;
            Ok(text_content(body))
        }
        "record_view_snapshot" => {
            let log = record_log_from_args(args)?;
            let at = json_u64_opt(args.get("at"))?;
            let snap = snapshot_json(&log, at).map_err(|e| rpc_error(-32000, &e.to_string()))?;
            let body =
                serde_json::to_value(&snap).map_err(|e| rpc_error(-32000, &e.to_string()))?;
            Ok(text_content(body))
        }
        "record_view_feed" => {
            let log = record_log_from_args(args)?;
            let since = required_u64(args, "since")?;
            let feed = feed_json(&log, since).map_err(|e| rpc_error(-32000, &e.to_string()))?;
            let body =
                serde_json::to_value(&feed).map_err(|e| rpc_error(-32000, &e.to_string()))?;
            Ok(text_content(body))
        }
        "memory_recent_trajectory" => {
            let hours = json_i64(args.get("hours"), 24).max(0);
            let limit = json_usize(args.get("limit"), 40);
            let mem = open_mem(memory_path, agent_id)?;
            let entries: Vec<_> = mem
                .doc()
                .get_recent_trajectory(hours)
                .into_iter()
                .rev()
                .take(limit)
                .map(|e| {
                    json!({
                        "id": e.id,
                        "type": match e.entry_type {
                            TrajectoryType::Observation => "observation",
                            TrajectoryType::Action => "action",
                            TrajectoryType::Reflection => "reflection",
                            TrajectoryType::SkillSynthesis => "skill_synthesis",
                            TrajectoryType::GoalUpdate => "goal_update",
                            TrajectoryType::ContextUpdate => "context_update",
                        },
                        "coherence": e.coherence,
                        "at": e.timestamp.to_rfc3339(),
                        "content": e.content,
                    })
                })
                .collect();
            Ok(text_content(json!({
                "count": entries.len(),
                "entries": entries,
            })))
        }
        other => Err(rpc_error(-32601, &format!("unknown tool: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ensembly_memory::TrajectoryType;
    use ensembly_schema::ViewContract;

    #[test]
    fn json_numeric_accepts_floats() {
        assert_eq!(json_i64(Some(&json!(24.0)), 0), 24);
        assert_eq!(json_usize(Some(&json!(5.0)), 40), 5);
        assert_eq!(json_usize(Some(&json!(7)), 40), 7);
        assert_eq!(json_i64(None, 24), 24);
    }

    #[test]
    fn open_existing_fails_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        match EpisodicMemory::open_existing(&path, "peram-swarm") {
            Ok(_) => panic!("expected missing file error"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("missing") || msg.contains("NotFound") || msg.contains("No such"),
                    "{msg}"
                );
            }
        }
    }

    #[test]
    fn recent_trajectory_respects_float_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mem.json");
        {
            let mut mem = EpisodicMemory::open(&path, "peram-swarm").unwrap();
            for i in 0..10 {
                mem.append(TrajectoryType::Observation, json!({"i": i}), 0.5);
            }
            mem.save().unwrap();
        }
        let result = call_tool(
            "memory_recent_trajectory",
            &json!({ "hours": 24.0, "limit": 5.0 }),
            &path,
            "peram-swarm",
        )
        .expect("tool ok");
        let text = result["content"][0]["text"].as_str().unwrap();
        let body: Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            body["count"], 5,
            "float limit must not fall back to default 40"
        );
    }

    #[test]
    fn record_commit_refuses_paths_outside_sandbox_or_local_data() {
        let err = call_tool(
            "record_commit",
            &json!({
                "log": "/etc/record-events.jsonl",
                "unit": "[]"
            }),
            &PathBuf::from("unused-memory.json"),
            "peram-swarm",
        )
        .unwrap_err();
        let msg = err["message"].as_str().unwrap_or("");
        assert!(
            msg.contains("data/local") || msg.contains("sandbox"),
            "{msg}"
        );
        assert!(!PathBuf::from("/etc/record-events.jsonl").exists());
    }

    #[test]
    fn record_log_allows_data_local_and_exact_env_path() {
        let anchor = tempfile::tempdir().unwrap();
        let temp = Path::new("/tmp/record-sandbox");
        let local = anchor
            .path()
            .join("data")
            .join("local")
            .join("events.jsonl");
        assert!(permit_record_log(Some(&local), None, temp, anchor.path()).is_ok());
        let custom = Path::new("/var/empty/record-events.jsonl");
        assert!(permit_record_log(None, Some(custom), temp, anchor.path()).is_ok());
        assert!(permit_record_log(Some(custom), Some(custom), temp, anchor.path()).is_ok());
        assert!(permit_record_log(
            Some(Path::new("/etc/record-events.jsonl")),
            None,
            temp,
            anchor.path()
        )
        .is_err());
        assert!(permit_record_log(None, None, temp, anchor.path()).is_err());
    }

    #[test]
    fn data_local_is_anchored_and_refuses_traversal_symlink_and_foreign_trees() {
        let anchor = tempfile::tempdir().unwrap();
        let temp = Path::new("/tmp/record-sandbox");
        let foreign = tempfile::tempdir().unwrap();
        let foreign_log = foreign
            .path()
            .join("data")
            .join("local")
            .join("events.jsonl");
        assert!(
            permit_record_log(Some(&foreign_log), None, temp, anchor.path()).is_err(),
            "foreign data/local"
        );

        let escaped = anchor
            .path()
            .join("data")
            .join("local")
            .join("..")
            .join("..")
            .join("outside.jsonl");
        assert!(
            permit_record_log(Some(&escaped), None, temp, anchor.path()).is_err(),
            "parent traversal"
        );

        let other = tempfile::tempdir().unwrap();
        let sibling = {
            let name = other.path().file_name().unwrap();
            anchor
                .path()
                .parent()
                .unwrap()
                .join(name)
                .join("data")
                .join("local")
                .join("events.jsonl")
        };
        assert!(
            permit_record_log(Some(&sibling), None, temp, anchor.path()).is_err(),
            "traversal into another tree"
        );

        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(outside.path().join("data").join("local")).unwrap();
        std::fs::create_dir_all(anchor.path().join("data")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("data").join("local"),
            anchor.path().join("data").join("local"),
        )
        .unwrap();
        let via_link = anchor
            .path()
            .join("data")
            .join("local")
            .join("events.jsonl");
        assert!(
            permit_record_log(Some(&via_link), None, temp, anchor.path()).is_err(),
            "symlinked data/local"
        );

        let outside_root = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .join(format!("foreign-record-{}", std::process::id()));
        let outside_log = outside_root.join("data").join("local").join("events.jsonl");
        let err = call_tool(
            "record_commit",
            &json!({
                "log": outside_log,
                "unit": "[]"
            }),
            &PathBuf::from("unused-memory.json"),
            "peram-swarm",
        )
        .unwrap_err();
        assert!(
            err["message"].as_str().unwrap_or("").contains("data/local")
                || err["message"].as_str().unwrap_or("").contains("sandbox"),
            "{err}"
        );
        assert!(
            !outside_root.exists(),
            "refused commit must not create a foreign tree"
        );
        let _ = std::fs::remove_dir_all(&outside_root);
    }

    #[test]
    fn mcp_refuses_human_or_owner_and_stamps_record_time() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("events.jsonl");
        let memory = PathBuf::from("unused-memory.json");
        let planted = "1999-01-01T00:00:00+01:00";
        for source in ["human", "owner"] {
            let unit = mcp_event(source, planted);
            let err = call_tool(
                "record_commit",
                &json!({ "log": &log, "unit": unit }),
                &memory,
                "peram-swarm",
            )
            .unwrap_err();
            assert!(
                err["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("human or owner"),
                "{source} {err}"
            );
        }
        assert!(!log.exists());
        let unit = mcp_event("document", planted);
        call_tool(
            "record_commit",
            &json!({ "log": &log, "unit": unit }),
            &memory,
            "peram-swarm",
        )
        .expect("document source");
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(!text.contains(planted), "{text}");
        assert!(text.contains("+01:00") || text.contains("+02:00"), "{text}");
    }

    fn mcp_event(source: &str, record_time: &str) -> String {
        format!(
            r#"[{{
                "id": "01JCV1Q8Z00000000000000000",
                "aggregate": "agg://example.test/acme-north/staff/1",
                "key": "mcp-source",
                "version": 1,
                "event_time": "2026-10-26T18:59:00+01:00",
                "record_time": "{record_time}",
                "source": "{source}",
                "schema_v": 1,
                "payload": {{"writer": "laptop-2", "note": "mcp"}}
            }}]"#
        )
    }

    #[test]
    fn scenario_a_mcp_commit_snapshot_equals_replay_and_rejects_same_key() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("events.jsonl");
        let raw = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../ensembly-schema/fixtures/scenarios/a-happy-events.json"),
        )
        .unwrap();
        let mut unit_value: Value = serde_json::from_str(&raw).unwrap();
        for event in unit_value.as_array_mut().unwrap() {
            event["source"] = json!("document");
        }
        let unit = unit_value.to_string();
        let memory = PathBuf::from("unused-memory.json");
        let committed = call_tool(
            "record_commit",
            &json!({ "log": log, "unit": unit }),
            &memory,
            "peram-swarm",
        )
        .expect("commit");
        let committed_body = tool_body(&committed);
        assert_eq!(committed_body["phase"], "done");

        let snap_body = tool_body(
            &call_tool(
                "record_view_snapshot",
                &json!({ "log": log }),
                &memory,
                "peram-swarm",
            )
            .expect("snapshot"),
        );
        let snap: ensembly_schema::Snapshot =
            serde_json::from_value(snap_body).expect("snapshot json");
        let text = std::fs::read_to_string(&log).unwrap();
        let events =
            ensembly_schema::parse_event_log::<ensembly_schema::JsonPayload>(&text).unwrap();
        let rows = ensembly_schema::replay(&events).unwrap();
        let via = ensembly_schema::RecordView::from_events(events)
            .snapshot_at(snap.at)
            .unwrap();
        assert_eq!(snap, via);
        assert_eq!(snap.facts.len(), rows.len());

        let again = call_tool(
            "record_commit",
            &json!({ "log": log, "unit": unit }),
            &memory,
            "peram-swarm",
        )
        .unwrap_err();
        assert!(
            again["message"]
                .as_str()
                .unwrap_or("")
                .contains("idempotency"),
            "{again}"
        );
        assert_eq!(
            std::fs::read_to_string(&log)
                .unwrap()
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count(),
            3
        );

        let before = std::fs::read(&log).unwrap();
        let feed = tool_body(
            &call_tool(
                "record_view_feed",
                &json!({ "log": log, "since": 1 }),
                &memory,
                "peram-swarm",
            )
            .expect("feed"),
        );
        assert_eq!(feed["since"], 1);
        assert!(feed["events"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["version"].as_u64().unwrap() > 1));
        assert_eq!(std::fs::read(&log).unwrap(), before);

        let tools = tool_defs();
        let names: Vec<_> = tools
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        for name in [
            "memory_recent_trajectory",
            "memory_get_report",
            "memory_list_skills",
            "memory_propose_goals",
            "memory_get_context",
            "swarm_banner",
            "record_commit",
            "record_view_snapshot",
            "record_view_feed",
        ] {
            assert!(names.iter().any(|n| n == name), "{names:?}");
        }
        assert!(names
            .iter()
            .all(|n| !n.contains("reconcile") && !n.contains("sync")));
    }

    fn tool_body(result: &Value) -> Value {
        let text = result["content"][0]["text"].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn missing_memory_errors_loudly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent.json");
        let err = call_tool("memory_get_report", &json!({}), &path, "peram-swarm").unwrap_err();
        assert_eq!(err["code"], -32000);
        assert!(
            err["message"].as_str().unwrap_or("").contains("missing")
                || err["message"].as_str().unwrap_or("").contains("NotFound"),
            "{}",
            err
        );
    }
}
