# Host decides done

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
stateDiagram-v2
  [*] --> Stop: agent says done
  Stop --> Checks: run named checks
  Checks --> Ready: all pass
  Ready --> Review: write marker with HEAD sha
  Review --> [*]: no-context reviewer on that sha
  Checks --> Continue: named check failed
  Checks --> Route: unclassified fail
  Route --> Continue: flowingstack routing
  Continue --> Stop: under 8
  Continue --> Failed: 8 reached
  Failed --> [*]
  [*] --> Guard: PreToolUse
  Guard --> Pass: new or under 3
  Guard --> Nudge: same call at 3
  Guard --> Blocked: same call at 5
  Blocked --> [*]
```

The agent saying done is a claim. These hooks run the checks and a repeat counter.

| Piece | Path | Role |
|---|---|---|
| checks | `hooks/done-gate` | run the configured list |
| stop | `hooks/stop` | Claude Code `decision` `block` plus Cursor `followup_message` |
| repeat | `hooks/repeat-guard` | hash tool name and input |
| list | `hooks/checks.tsv` | tests, privacy, host-alias by default |

| Event | Fail | Bound |
|---|---|---|
| named check fail | continue with the reason | 8 then `failed-gate` |
| unclassified fail | continue and route through flowingstack | same bound |
| all pass | ready marker with HEAD sha | no-context reviewer on that sha |
| same tool 3 times | nudge only, no permission decision | — |
| same tool 5 times | deny and stop | queue row `blocked` when `FS_QUEUE_FILE` and `FS_QUEUE_ROW` are set |

Unclassified means no check named, missing config, unknown tool, or unknown error. `route-rule` picks the `unknown` role. The shipped lead is `unknown`. A house overlay may set `eva-emptiness` with fallback `odysseus-navigator`.

## Turn on

| Harness | File | Hook |
|---|---|---|
| Claude Code | project `.claude/settings.json` | Stop and PreToolUse command hooks |
| Cursor | project `.cursor/hooks.json` | `stop` with `loop_limit` 8 and `preToolUse` |

```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "python3 ${CLAUDE_PLUGIN_ROOT}/hooks/stop",
            "timeout": 600
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "python3 ${CLAUDE_PLUGIN_ROOT}/hooks/repeat-guard"
          }
        ]
      }
    ]
  }
}
```

```json
{
  "version": 1,
  "hooks": {
    "stop": [
      {
        "command": "python3 ${FLOWINGSTACK_ROOT}/hooks/stop",
        "loop_limit": 8
      }
    ],
    "preToolUse": [
      {
        "command": "python3 ${FLOWINGSTACK_ROOT}/hooks/repeat-guard"
      }
    ]
  }
}
```

| Hook fact | Detail |
|---|---|
| Claude Stop | fires every turn |
| Hook timeout | fail-open; set it above the check timeouts |
| Claude path | `${CLAUDE_PLUGIN_ROOT}` or `${CLAUDE_PROJECT_DIR}` in the shell command |
| Cursor path | `${FLOWINGSTACK_ROOT}` from the workspace root |
| `FS_CHECK_TIMEOUT` | seconds per check, default 120 |
| `FS_MAX_CONTINUATIONS` | 1 or more, default 8 |
| `FS_DENY_AT` | 1 to 20, default 5 |

Set `FLOWINGSTACK_ROOT` to this plugin root when the project is another tree.
Set `FLOWINGSTACK_ROUTING` to `examples/house-routing.tsv` or a local `.flowingstack/routing.tsv` to load house rows.
Leave the user Claude Code and Cursor config files untouched unless you want the hooks on every project.

## Source

Grok Build's inner mechanism inspired the `done-gate` and `repeat-guard` hooks.

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#e5dbff", "primaryBorderColor": "#7048e8", "primaryTextColor": "#2b1a5e", "secondaryColor": "#d0ebff", "tertiaryColor": "#fff3bf", "lineColor": "#5f3dc4", "fontFamily": "Inter"}}}%%
stateDiagram-v2
  [*] --> Announce
  Announce --> RepeatHalt: same call hits the hard stop
  Announce --> RepeatNudge: same call crosses the warn mark
  RepeatNudge --> MaybeShrink
  Announce --> MaybeShrink
  MaybeShrink --> AwaitModel: shrink context when over the threshold
  AwaitModel --> Send
  Send --> Hold: rate-limit budget says wait
  Hold --> Send
  Send --> Reply
  Reply --> AllDone: no tools and nothing left to drain
  Reply --> Again: empty reply with a leftover nudge
  Again --> Announce
  Reply --> SoftLimit: tool batch hits the soft cap
  SoftLimit --> TurnCap: next tool round would exceed max turns
  TurnCap --> [*]
  SoftLimit --> Announce: record denies and skip the run
  Reply --> BeforeCall: tool batch under the cap
  BeforeCall --> Permit
  Permit --> SkipRun: policy deny
  Permit --> TurnReject: reject
  Permit --> TurnCancel: cancelled
  Permit --> RunCall: allow
  RunCall --> AfterCall
  SkipRun --> AfterCall
  AfterCall --> ShrinkMore: context now over the window
  ShrinkMore --> Announce
  AfterCall --> Announce
  TurnReject --> [*]
  TurnCancel --> [*]
  RepeatHalt --> [*]
  AllDone --> HostCheck
  HostCheck --> Announce: host asks to continue, under eight
  HostCheck --> [*]: host ends the turn
```

```mermaid
%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#fff3bf", "primaryBorderColor": "#f08c00", "primaryTextColor": "#4a3000", "secondaryColor": "#d0ebff", "tertiaryColor": "#d3f9d8", "lineColor": "#1971c2", "fontFamily": "Inter"}}}%%
flowchart TD
  subgraph gb["Grok Build"]
    done["host-owned completion and Stop hook"]
    ident["identical-call detector"]
    unk["unknown the host cannot classify"]
  end
  subgraph ours["our hooks"]
    stop["hooks/stop"]
    gate["hooks/done-gate"]
    guard["hooks/repeat-guard"]
  end
  subgraph hub["flowingstack"]
    route["route-rule"]
    eva["eva-emptiness"]
    odys["odysseus-navigator"]
  end
  done --> stop --> gate
  ident --> guard
  unk --> route
  route --> eva
  route --> odys
  classDef origin fill:#e5dbff,stroke:#7048e8,color:#2b1a5e
  classDef hook fill:#d0ebff,stroke:#1971c2,color:#0b2e4f
  classDef skill fill:#d3f9d8,stroke:#2f9e44,color:#0b3d17
  class done,ident,unk origin
  class stop,gate,guard,route hook
  class eva,odys skill
```
