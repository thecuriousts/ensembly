# ensembly-agents

Protocol and inference **satellite** for the ensembly operator kernel (`ensembly-kernel` stays control SoT).

Official Grok (xAI) docs:
- [Headless & ACP](https://docs.x.ai/build/cli/headless-scripting#acp) — `grok agent stdio`, `grok -p`
- [MCP servers](https://docs.x.ai/build/features/mcp-servers) — `grok mcp add` / `.grok/config.toml`

| Module | Role |
|--------|------|
| `inference` | `InferenceProvider` trait; `PERAM_INFERENCE=deterministic\|grok-mcp\|…` |
| `delegation` | `DelegationBackend`; `PERAM_DELEGATE=none\|grok-acp\|…` |
| `mcp_server` / `ensembly-mcp` | Read-only MCP tools (sync NDJSON stdio) |
| `grok` | Official CLI argv helpers + project MCP TOML snippet |
| `rpc` | Sync JSON-RPC 2.0 NDJSON |

## Register memory tools into Grok

```bash
# Build the server
cargo build -p ensembly-agents --bin ensembly-mcp

# Project scope (writes .grok/config.toml — gitignored; use absolute PERAM_MEMORY)
grok mcp add --scope project ensembly -- ./target/debug/ensembly-mcp

# Or paste into .grok/config.toml with an absolute memory path:
# [mcp_servers.ensembly]
# command = "./target/debug/ensembly-mcp"
# env = { PERAM_MEMORY = "/ABS/PATH/to/ensembly/data/local/peram-memory.json" }

grok mcp list --json
grok mcp doctor ensembly
```

Grok also loads Cursor `.mcp.json` / `.cursor/mcp.json` (compat). Tools are namespaced `ensembly__<tool>` when the server key is `ensembly`.

## ACP / headless (operator machine)

```bash
# ACP agent for IDE/tool hosts
grok --no-auto-update agent stdio

# Fast reflect enrichment (InferenceProvider path)
grok --no-auto-update -p "Summarize this trajectory…" --output-format json
```

Auth: `grok login` or `XAI_API_KEY`. Scripts: always `--no-auto-update`.

## Tests

```bash
cargo test -p ensembly-agents
```
