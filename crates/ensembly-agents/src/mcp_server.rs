//! Read-only Model Context Protocol server for episodic memory.
//! Primary consumer: Grok (via `grok mcp add` / `.grok/config.toml` or Cursor MCP config).
//! Mutating tools are out of scope for P4a — record path stays kernel CLI.

use ensembly_memory::{coherence_report, propose_goals, EpisodicMemory, TrajectoryType};
use serde_json::{json, Value};
use std::path::PathBuf;

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
            memory_path: PathBuf::from(
                std::env::var("PERAM_MEMORY")
                    .unwrap_or_else(|_| "data/local/peram-memory.json".into()),
            ),
            agent_id: std::env::var("PERAM_AGENT_ID")
                .unwrap_or_else(|_| "peram-swarm".into()),
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
    EpisodicMemory::open_existing(path, agent_id).map_err(|e| {
        rpc_error(
            -32000,
            &format!("open memory {path:?}: {e}"),
        )
    })
}

fn call_tool(
    name: &str,
    args: &Value,
    memory_path: &PathBuf,
    agent_id: &str,
) -> DispatchResult {
    match name {
        "swarm_banner" => Ok(text_content(json!({
            "ok": true,
            "agents": crate::agents_version(),
            "memory": ensembly_memory::memory_version(),
            "memoryPath": memory_path,
            "agentId": agent_id,
            "note": "Control SoT remains ensembly-kernel; this server is read-only aux. Prefer absolute PERAM_MEMORY.",
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
            Ok(text_content(serde_json::to_value(report).unwrap_or_default()))
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
                mem.append(
                    TrajectoryType::Observation,
                    json!({"i": i}),
                    0.5,
                );
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
        assert_eq!(body["count"], 5, "float limit must not fall back to default 40");
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
