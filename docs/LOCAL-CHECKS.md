# Local fact and conflict checks

Replaces the Ollama adapter stub. Fact classification and conflict checks run
on the operator machine with llama.cpp (preferred). They are optional: the
default inference path stays deterministic Jaccard, and CI never needs a
model file.

## What it does

- **Fact classification** (4B): labels a statement `fact`, `opinion`,
  `instruction`, `uncertain`, or `other`, plus a confidence and a short
  rationale, as strict JSON.
- **Conflict check** (9B): compares two statements as `contradicts`,
  `compatible`, or `underspecified`.
- **One model at a time.** A process lock is taken around each call. The
  llama.cpp process exits (and unloads) before the other model can load.
  The lock waits up to about 2 seconds (40 × 50 ms). On Unix, a lock is
  stale if its PID is not alive. If a new process reused that PID, the
  waiter treats the lock as live, times out, and fails closed — it does
  not delete a lock that might still be held, and it does not send the
  text to a cloud fallback. On non-Unix platforms, a lock older than 30
  minutes is treated as stale.
- **Memory first.** A load is refused unless available RAM covers the model
  file plus 768 MiB headroom. The refusal is a local error; it does not
  escalate to the cloud.
- **Cloud fallback is off by default.** It runs only when the operator sets
  both `ENSEMBLY_CLOUD_CHECKS=1` and `ENSEMBLY_CLOUD_CHECK_CMD`, and only
  after a successful local result whose confidence is below `0.6`. Lock
  timeouts, memory refusals, a missing model or runtime, invalid JSON, and
  any other local error return that local error and never escalate.
  `grok` on `PATH` is never auto-detected. Each cloud call is logged with
  its reason and argv0 only — never the prompt or statement. A fail-closed
  redaction gate refuses the call entirely (it never partly redacts and
  sends) if any view of the text looks like a token, civic or coordination
  number, phone, email, ISO 13616 account number, payment card, home path, IP, MAC, hostname,
  or a name-like field.
  Views are the raw text; NFKC with zero-width/format characters stripped
  and confusables folded to ASCII; up to three rounds of percent and
  HTML-entity decoding; quoted-printable and MIME encoded-word decoding;
  JSON and slash-escape unescaping (`\u`, `\x`, `\n`); `at`/`dot` word
  and bracket forms (`[at]`, `(at)`, `[dot]`, `(dot)`); whitespace
  collapsed around `@./:-` (and removed entirely for token-prefix
  matching, including spaced `sk` prefixes); valid UTF-8 from base64 or
  base64url runs of 8 or more characters, kept only when the decoded
  bytes then hit a detector; and a reversed-text view used only for
  email and token detection. The base64 / base64url minimum is 8
  characters, not 16; a decoded run is kept only when it then hits a
  detector. International and Swedish phone numbers accept `.` and `/`
  separators. A dotted quad preceded by `v`, `version`, or `release` is
  treated as a version, not an IP, unless the quad is RFC1918, loopback,
  or link-local (`10/8`, `172.16/12`, `192.168/16`, `127/8`, `169.254/16`);
  those stay IP hits. Other quads stay fail-closed.
  Tests use synthetic fixtures only.

## Models and licences

Preferred weights, all Apache-2.0 from the official model cards:

| Role | Official model | Local file we use | Licence |
|---|---|---|---|
| Classify | [Qwen/Qwen3.5-4B](https://huggingface.co/Qwen/Qwen3.5-4B) | Community GGUF `unsloth/Qwen3.5-4B-GGUF` `Qwen3.5-4B-Q4_K_M.gguf` (inherits Apache-2.0) | Apache-2.0 |
| Classify alternative | [google/gemma-4-E4B](https://huggingface.co/google/gemma-4-E4B) | Official Google weights | Apache-2.0 |
| Conflict | [Qwen/Qwen3.5-9B](https://huggingface.co/Qwen/Qwen3.5-9B) | Community GGUF `unsloth/Qwen3.5-9B-GGUF` Q4_K_M when disk allows | Apache-2.0 |
| Runtime | Official [llama.cpp](https://github.com/ggml-org/llama.cpp) macOS arm64 build | MIT | |

MLX is not a production runtime yet: it has no `--json-schema` enforcement
to match llama.cpp. Set `ENSEMBLY_MLX_EXPERIMENTAL=1` only if you accept
post-hoc JSON parsing.

Gemma 4 E4B is the documented alternative if the Qwen 4B file cannot be used.
Community GGUFs are only accepted when they inherit the official Apache-2.0
licence. Do not use quantizations under a custom licence that bans product
use (for example Ora Computing's Qwen3.5-9B GGUF).

Installs and downloads on mac-mini are reversible. What was installed, where
it lives, and how to remove it is logged outside this repo in the operator
`local-models-install-log`. Default prefix: `$HOME/.local/share/ensembly/`
(remove that directory to undo).

## CI versus live

CI replays recorded synthetic JSON in `fixtures/local-checks/` and checks the
SHA-256 in `MANIFEST.sha256`. It does not download models.

Live model processes run only when `ENSEMBLY_LOCAL_MODELS=1` is set, on a
machine that already has the runtime and weights. Library callers of
`resolve_checker()` read the same environment (including the cloud flags)
and do not consult `ENSEMBLY_LOCAL_MODELS`; use `RecordedSet` when you want
fixtures.

A live classify run on mac-mini with Qwen3.5-4B Q4_K_M produced valid JSON
for all five synthetic fixtures once the model could load, with labels
matching the recorded fixtures on 3/5 (stock fact, cheerful opinion, move
instruction). The 9B conflict model was not downloaded on that machine
(disk), so conflict checks reported `model-missing`. Those live figures are
evidence, not a CI requirement.

```bash
# CI / default: replay recorded outputs
cargo test -p ensembly-agents
cargo run -p ensembly-agents --bin ensembly-local-check -- fixtures

# Live, after the runtime and models are installed (mac-mini):
ENSEMBLY_LOCAL_MODELS=1 \
ENSEMBLY_LLAMA_CLI="$HOME/.local/share/ensembly/bin/llama-cli" \
ENSEMBLY_CLASSIFY_MODEL="$HOME/.local/share/ensembly/models/qwen3.5-4b/Qwen3.5-4B-Q4_K_M.gguf" \
ENSEMBLY_CONFLICT_MODEL="$HOME/.local/share/ensembly/models/qwen3.5-9b/Qwen3.5-9B-Q4_K_M.gguf" \
cargo run -p ensembly-agents --bin ensembly-local-check -- fixtures

# Cloud (optional, off unless both are set):
# ENSEMBLY_CLOUD_CHECKS=1
# ENSEMBLY_CLOUD_CHECK_CMD='your-cmd -- {{prompt}}'
```

## Environment

| Variable | Meaning |
|---|---|
| `ENSEMBLY_LOCAL_MODELS` | Truthy (`1`, `true`, `yes`, or `on`, any case) to run real models. Unset or any other value is off. |
| `ENSEMBLY_LOCAL_RUNTIME` | `llama.cpp` (default) or `mlx` (requires `ENSEMBLY_MLX_EXPERIMENTAL=1`) |
| `ENSEMBLY_LLAMA_CLI` | Path to `llama-cli` or `llama-completion` |
| `ENSEMBLY_MLX_BIN` | Path to an MLX generate binary |
| `ENSEMBLY_MLX_EXPERIMENTAL` | Truthy (`1`, `true`, `yes`, or `on`) to allow MLX despite missing schema-constrained generation |
| `ENSEMBLY_CLASSIFY_MODEL` | 4B weights (GGUF or MLX dir) |
| `ENSEMBLY_CONFLICT_MODEL` | 9B weights |
| `ENSEMBLY_LOCAL_CONFIDENCE_MIN` | Cloud-eligible floor, default `0.6` (cloud still needs the opt-in below) |
| `ENSEMBLY_CLOUD_CHECKS` | Truthy (`1`, `true`, `yes`, or `on`, any case) to allow cloud fallback. Off if unset or any other value. |
| `ENSEMBLY_CLOUD_CHECK_CMD` | Command template; `{{prompt}}` is replaced. Required together with `ENSEMBLY_CLOUD_CHECKS=1`. Never inferred from `PATH`. |
| `ENSEMBLY_LOCAL_LOCK` | Override the one-model lock path |
| `ENSEMBLY_INFERENCE=local` | Selects the local backend (`ollama`, `llama.cpp`, and `mlx` are aliases) |

Local checks read only `ENSEMBLY_*` variables. There is no legacy alias.

The CLI `status` command reports `cloud_opted_in` (both the flag and a
non-empty command are set), `cloud_cmd_set` (a command template is set),
`cloud_checks` (the flag alone), and `cloud_status` (`off`,
`cmd-set-flag-off`, `flag-set-cmd-missing`, or `opted-in`) so a set
command is not mistaken for an active fallback.

CLI JSON does not include model `raw` output (which can echo user content)
unless you pass `--debug`. In the library, `CheckSuccess` and
`CheckFailure` omit `raw` from serialization; use `raw_for_debug()` when
you have explicitly opted into debug output.
