# Network trace: install and first run

Date: Saturday 14 November 2026, 06:52–06:59 CET (Stockholm).
ensembly commit: `d62cf28d047c3d131debb672663e804f3cfb88c1` (master), crate `ensembly-kernel` 0.5.1.

**Result.** After install, `ensembly init`, `ensembly doctor --offline`, the runtime and pulse-pack commands, and `ensembly-mcp` made **no network connections**, on Linux or macOS. Online `ensembly doctor`'s only network call is `git ls-remote` to github.com:443, to look for a newer release tag. Install-time downloads came only from rustup (`sh.rustup.rs`, `static.rust-lang.org`), GitHub (`git clone`) and crates.io (`index.crates.io`, `static.crates.io`).

## Method

Two independent views on Linux, one kernel-enforced view on macOS.

| View | What it records | Catches traffic that ignores the proxy? |
|------|-----------------|------------------------------------------|
| mitmproxy (`mitmdump` + [flowlog.py](network-trace/flowlog.py)) | Host, port, method, path (query string dropped), status, sizes, and the local process that opened the proxy connection. Never headers or bodies. | No |
| `strace -f -Y -e trace=connect,sendto,sendmmsg` | Every `connect()` syscall by every process and child, with the process name | Yes (ground truth) |
| `strace -f -e trace=%network` (rerun of the ensembly-only phases) | Every network-class syscall: `socket`, `connect`, `bind`, `sendto`, `sendmsg`, … | Yes |
| macOS `sandbox-exec` with [no-net.sb](network-trace/no-net.sb) (`deny network*`) plus the kernel sandbox log | Every network attempt by the command and its children is blocked and logged by process name | Yes |

Linux setup: a fresh empty `HOME`, a minimal environment (`env -i`), `HTTPS_PROXY`/`HTTP_PROXY` pointing at the local mitmproxy, and the mitmproxy CA added to the CA bundle for curl, git and cargo. Rust was not installed in that `HOME`, so the trace includes a real rustup install.

macOS (mac-mini) setup: the mac-mini has about 300 MB free and a standing rule of no cargo builds there, so ensembly was not installed on it. Instead, the same commit was cross-built for `aarch64-apple-darwin` (cargo-zigbuild) and the two binaries were copied to a temporary folder with a fresh empty `HOME`. Each runtime command ran under the deny-all-network sandbox. A control command (`curl https://example.com/`) ran under the same sandbox to prove blocking and logging work. The temporary folder was removed afterwards.

### Tool versions

| | Linux box | mac-mini |
|--|--|--|
| OS | Debian 13.7, kernel 6.12, x86_64 | macOS 26.6.2, arm64 |
| Tracer | strace 6.13, mitmproxy 12.3.1 | sandbox-exec, unified log |
| Toolchain | rustup 1.29.3, Rust 1.99.1 (pinned by `rust-toolchain.toml`), git 2.47.3 | binaries cross-built on the Linux box with Rust 1.99.1, cargo-zigbuild 0.24.1, zig 0.13.0 |

## Findings per phase (Linux)

"Connects" counts non-local `connect()` calls seen by strace. Every one of them went to the local mitmproxy; none bypassed it.

| Phase | Command | Destinations (host:port) | Process | Purpose | Requests |
|-------|---------|--------------------------|---------|---------|----------|
| a0 | `curl https://sh.rustup.rs` | sh.rustup.rs:443 | curl | rustup installer script | 1 |
| a1 | `sh rustup-init.sh -y --default-toolchain none` | static.rust-lang.org:443 | curl | rustup-init binary (21 MB) | 1 |
| a2 | `git clone https://github.com/thecuriousts/ensembly` | github.com:443 | git-remote-http | clone the repo | 3 |
| a3 | `sh scripts/install.sh --harness cli` | static.rust-lang.org:443 | cargo (rustup proxy) | pinned toolchain 1.99.1 (135 MB) | 7 |
| | | index.crates.io:443 | cargo | crate index | 276 |
| | | static.crates.io:443 | cargo | crate downloads (12 MB) | 94 |
| a4 | `sh scripts/install.sh --harness all` (re-run) | index.crates.io:443 | cargo | index freshness check, nothing downloaded | 138 |
| b | `ensembly init` | **none** (0 connects) | — | — | 0 |
| c | `ensembly doctor --offline` | **none** (0 connects) | — | — | 0 |
| c2 | `ensembly doctor` (online) | github.com:443 | git-remote-http (child of `git ls-remote`) | latest release tag lookup | 2 |
| d1 | `runtime load/status/tick/approve/claim/complete/reflect`, `pulse-pack export/import`, `version` | **none** (0 connects) | — | pulse-pack export/import only; premflow not traced | 0 |
| d2 | `cargo build --release -p ensembly-agents --bin ensembly-mcp` | **none** (crates already cached) | — | — | 0 |
| d3 | `ensembly-mcp` over stdio: `initialize`, `tools/list`, `tools/call memory_get_report` | **none** (0 connects) | — | — | 0 |
| e | `ensembly export` | not run: the bundle commands (PR #81) are not merged yet | | | |

Rerun of b, c, d1 and d3 with `strace -e trace=%network`: **0 network-class syscalls of any kind** (no `socket`, `connect`, `bind`, `sendto` or `sendmsg`).

Static check of the Linux release binaries: `ensembly` and `ensembly-mcp` import none of `socket`, `connect`, `getaddrinfo`, `bind`, `listen` or `accept` from libc. `ensembly` imports `socketpair` and `sendmsg`, which come from Rust's standard-library process-spawning code (used by `doctor`); the `%network` rerun, which includes `doctor --offline`, shows neither was called.

## Findings (macOS, mac-mini)

| Phase | Command | Exit | Sandbox network denials |
|-------|---------|------|-------------------------|
| control | `curl -m 5 https://example.com/` | 6 (cannot resolve) | curl: `network-outbound` to the DNS resolver |
| b | `ensembly init` | 0 | none |
| c | `ensembly doctor --offline` | 0 | none |
| c2 | `ensembly doctor` | 0 (`latest offline`) | git-remote-http: `network-outbound` to the DNS resolver |
| d1 | runtime load/status/tick/approve/claim/complete/reflect, pulse-pack export/import only (premflow not traced) | 0 | none |
| d3 | `ensembly-mcp` stdio, same three calls | 0 | none |

Every runtime command worked with all networking blocked, and the sandbox log has no entry for `ensembly` or `ensembly-mcp`.

## Code audit

Search of non-test Rust code in `crates/` for `reqwest`, `ureq`, `hyper`, `TcpStream`, `UdpSocket`, `TcpListener`, `std::net`, `curl` and `Command::new`, plus `Cargo.lock` for HTTP, TLS and async-network crates (`reqwest`, `ureq`, `hyper`, `tokio`, `mio`, `socket2`, `rustls`, `native-tls`, `openssl`, `curl`).

- **No HTTP/TLS client crates**, socket API or TLS library are used anywhere in the workspace, and none is in `Cargo.lock`.
- URLs in source are strings only (documentation links, test fixtures, the repo homepage).

Every code path that can reach the network does it by running another program:

| Code | When it runs | What goes out |
|------|--------------|---------------|
| `crates/ensembly-kernel/src/doctor.rs:284-291` `latest_tag` runs `git ls-remote --tags --refs <remote> 'refs/tags/ensembly--v*'` (8 s timeout) | `ensembly doctor` **without** `--offline`. Skipped by `--offline` (`doctor.rs:113`) or `ENSEMBLY_OFFLINE=1` (`doctor.rs:87`). Remote defaults to `https://github.com/thecuriousts/ensembly` (`doctor.rs:10`), override with `ENSEMBLY_DOCTOR_REMOTE` (`doctor.rs:86`). | A standard git ref listing request to GitHub. No ensembly data. Seen in phase c2. |
| `crates/ensembly-kernel/src/doctor.rs:258` `tool_version` runs `git`, `python3`, `claude`, `cursor-agent` with `--version` | `ensembly doctor`, with or without `--offline` | Nothing from ensembly; whatever those tools do for `--version` (git and python3 made no connections here; claude and cursor-agent were not installed). |
| `crates/ensembly-kernel/src/pulse_core.rs:405-415` runs `premflow stats` if `premflow` is on `PATH` (`pulse_core.rs:112`) | `ensembly pulse` health check | Up to the premflow tool. Not exercised. |
| `crates/ensembly-agents/src/local_checks/runtime.rs:774-822` `invoke_cloud` runs a user-supplied command | Only in the separate `ensembly-local-check` binary (not installed by `install.sh`), and only when both `ENSEMBLY_CLOUD_CHECKS=1` and `ENSEMBLY_CLOUD_CHECK_CMD` are set (`runtime.rs:242-245`). It never auto-detects a cloud tool (`runtime.rs:255-266`) and refuses text that fails `assert_safe_for_cloud`. | The prompt, to whatever command the user configured. Not exercised. |
| `crates/ensembly-agents/src/local_checks/runtime.rs:699` (llama.cpp with `-m <local model file>`), `:754-759` (MLX), probes at `:371` (`python3 -c "import mlx_lm"`) and `support.rs:52-58` (`pagesize`, `vm_stat`) | Only in the separate `ensembly-local-check` binary (not installed by `install.sh`) when a local runtime is found or set with `ENSEMBLY_LLAMA_CLI` / `ENSEMBLY_MLX_BIN` | Nothing from ensembly; the model is a local file path. Not exercised. |
| `crates/evalbench-cli/src/lib.rs:105` `CommandClient` runs a configured harness wrapper | Only in the `evalbench` developer benchmark (not installed by `install.sh`) on a live run | Whatever the configured wrapper does. Not exercised. |
| `scripts/install.sh:126` `git pull --ff-only` | `install.sh --update` only | Git fetch from the checkout's own remote. |

The optional `opportunity-finder` feature has no network code (no HTTP client, socket or subprocess call in `crates/ensembly-opportunity-finder/src`).

## Conclusion

What the evidence supports:

1. ensembly has no telemetry: no HTTP client, socket or TLS code in the workspace, and zero network syscalls from `ensembly init`, `ensembly doctor --offline`, the runtime and pulse-pack commands, and `ensembly-mcp`, on Linux (strace) and macOS (sandbox).
2. The traced phases showed zero ensembly-originated connections, and the code has no HTTP/TLS client crates. This proof does not cover the `--version` subprocesses `doctor` runs on `git`, `python3`, `claude` and `cursor-agent`.
3. Install-time traffic comes only from the toolchain and the package registry: rustup, GitHub for the clone, and crates.io.
4. `ensembly doctor` without `--offline` contacts GitHub through `git ls-remote` to check for updates. It sends a git ref request, no ensembly data.

## Limits

- One run on each OS. Linux is the full install-and-run trace; macOS covers the runtime commands only, with binaries cross-built on Linux rather than installed by `install.sh` (disk and no-build rule on the mac-mini).
- The macOS static import check is not meaningful for the cross-built binaries (the zig linker kept standard-library symbols that Apple's linker strips), so it is not used as evidence.
- Not exercised: `ensembly export`/`backup`/`restore` from PR #81 (not merged); the `opportunity-finder` feature build; Cursor and Claude Code themselves (phase a4 only ran the installer's link steps for them); `ensembly pulse` with premflow present; `ensembly-local-check` with local models or the opt-in cloud command; `scripts/stranger-test.sh`.
- strace sees syscalls made by the traced process tree. It does not see network work done on its behalf by an already-running daemon; no such IPC (Unix socket connect) was seen in the ensembly phases either.
- This proves the behaviour of commit `d62cf28` only. A later change could add a network call; re-run the method below to check.

## Re-run it yourself

Linux (needs `strace` and `mitmproxy`; uses a throwaway `HOME`):

```sh
T=$(mktemp -d); mkdir -p "$T/h" "$T/conf"
FLOWLOG_OUT=$T/flows.jsonl FLOWLOG_PHASE=$T/phase \
  mitmdump --set confdir="$T/conf" --listen-host 127.0.0.1 -p 8899 -q -s docs/proof/network-trace/flowlog.py &
sleep 3; cat /etc/ssl/certs/ca-certificates.crt "$T/conf/mitmproxy-ca-cert.pem" > "$T/ca.pem"
run() { echo "$1" > "$T/phase"; n=$1; shift
  env -i HOME="$T/h" PATH="$T/h/.cargo/bin:/usr/bin:/bin" LANG=C.UTF-8 \
    HTTPS_PROXY=http://127.0.0.1:8899 HTTP_PROXY=http://127.0.0.1:8899 \
    SSL_CERT_FILE="$T/ca.pem" CARGO_HTTP_CAINFO="$T/ca.pem" GIT_SSL_CAINFO="$T/ca.pem" \
    strace -f -Y -qq -e trace=connect -o "$T/strace-$n.txt" "$@"; }
run clone git clone https://github.com/thecuriousts/ensembly "$T/h/ensembly"
cd "$T/h/ensembly"
run install sh scripts/install.sh --harness cli   # needs Rust; see the rustup steps in the table above
run init ensembly init
run doctor ensembly doctor --offline
grep -v AF_UNIX "$T"/strace-init.txt "$T"/strace-doctor.txt   # expect no output
```

macOS (run from the repo root; no install needed if `ensembly` is already on `PATH`):

```sh
D=$(mktemp -d); mkdir "$D/home"
start=$(date '+%Y-%m-%d %H:%M:%S')
for c in "init" "doctor --offline" "runtime status"; do
  env -i HOME="$D/home" PATH="$(dirname "$(command -v ensembly)"):/usr/bin:/bin" \
    sandbox-exec -f docs/proof/network-trace/no-net.sb ensembly $c; done
/usr/bin/log show --start "$start" --style compact --predicate 'eventMessage CONTAINS "Sandbox:" AND eventMessage CONTAINS "network"'
# expect no line naming ensembly
```

The raw traces from this run are kept privately, outside the repo.
