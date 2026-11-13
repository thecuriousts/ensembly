//! Local runtimes (llama.cpp, then MLX) and the command-based cloud fallback.
//!
//! No compile-time dependency on those tools. CI never needs the models:
//! tests inject a mock command or replay recorded synthetic JSON.
//!
//! Cloud fallback is off unless the operator sets both
//! `ENSEMBLY_CLOUD_CHECKS=1` and `ENSEMBLY_CLOUD_CHECK_CMD`. `grok` on
//! `PATH` is never auto-detected. Local errors (lock, memory, missing
//! model or runtime, invalid JSON, timeouts) return a local failure and
//! never escalate. Only a successful local result below the confidence
//! floor may call the cloud, after a fail-closed redaction gate, and each
//! such call is logged without its content.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::local_checks::schema::{
    classify_system_prompt, classify_user_prompt, conflict_json_schema, conflict_system_prompt,
    conflict_user_prompt, fact_json_schema, parse_conflict_class, parse_fact_class, ConflictClass,
    FactClass, DEFAULT_CONFIDENCE_FLOOR,
};
use crate::local_checks::support::{
    assert_safe_for_cloud, can_load_model, default_lock_path, required_bytes_for_model, HostMemory,
    MemoryView, ModelLock,
};

const DEFAULT_TIMEOUT_SECS: u64 = 180;
const DEFAULT_CONTEXT: u32 = 2048;
const DEFAULT_MAX_TOKENS: u32 = 256;
/// Bound on child stdout (and stderr) before `run_cmd` fails safe.
pub const MAX_STDOUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    LlamaCpp,
    Mlx,
    Cloud,
    Replay,
}

impl RuntimeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LlamaCpp => "llama.cpp",
            Self::Mlx => "mlx",
            Self::Cloud => "cloud",
            Self::Replay => "replay",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    Classify,
    Conflict,
}

struct CloudCtx<'a> {
    kind: CheckKind,
    left: &'a str,
    right: Option<&'a str>,
    started: Instant,
}

impl CheckKind {
    pub fn model_role(self) -> &'static str {
        match self {
            Self::Classify => "classify (4B)",
            Self::Conflict => "conflict (9B)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackReason {
    RuntimeMissing,
    ModelMissing,
    NotEnoughMemory,
    LockBusy,
    InvalidJson,
    LowConfidence,
    CloudBlocked,
    CloudMissing,
}

impl FallbackReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeMissing => "local-runtime-missing",
            Self::ModelMissing => "model-missing",
            Self::NotEnoughMemory => "not-enough-memory",
            Self::LockBusy => "lock-busy",
            Self::InvalidJson => "invalid-json",
            Self::LowConfidence => "low-confidence",
            Self::CloudBlocked => "cloud-redaction",
            Self::CloudMissing => "cloud-missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckSource {
    pub runtime: RuntimeKind,
    pub model: String,
    pub fallback: Option<FallbackReason>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckSuccess<T> {
    pub value: T,
    pub source: CheckSource,
    pub latency: Duration,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckFailure {
    pub reason: FallbackReason,
    pub detail: String,
    pub latency: Duration,
    pub raw: Option<String>,
}

pub type CheckResult<T> = Result<CheckSuccess<T>, CheckFailure>;

#[derive(Debug, Clone)]
pub struct LocalCheckConfig {
    pub llama_bin: Option<PathBuf>,
    pub mlx_bin: Option<PathBuf>,
    pub classify_model: Option<PathBuf>,
    pub conflict_model: Option<PathBuf>,
    /// Must be true *and* `cloud_cmd` set before any cloud call is allowed.
    pub cloud_checks: bool,
    pub cloud_cmd: Option<Vec<String>>,
    pub confidence_floor: f32,
    pub timeout: Duration,
    pub context: u32,
    pub max_tokens: u32,
    pub lock_path: PathBuf,
    pub prefer: RuntimeKind,
    /// Extra memory probes when the first reading is short (Metal teardown).
    pub memory_retries: u32,
    /// MLX has no `--json-schema` parity with llama.cpp. Off unless opted in.
    pub mlx_experimental: bool,
}

impl Default for LocalCheckConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl LocalCheckConfig {
    pub fn from_env() -> Self {
        let prefer = match env_var("ENSEMBLY_LOCAL_RUNTIME")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "mlx" => RuntimeKind::Mlx,
            _ => RuntimeKind::LlamaCpp,
        };
        let floor = env_var("ENSEMBLY_LOCAL_CONFIDENCE_MIN")
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_CONFIDENCE_FLOOR);
        let timeout = Duration::from_secs(
            env_var("ENSEMBLY_LOCAL_TIMEOUT_SECS")
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_TIMEOUT_SECS),
        );
        Self {
            llama_bin: env_path("ENSEMBLY_LLAMA_CLI").or_else(discover_llama_bin),
            mlx_bin: env_path("ENSEMBLY_MLX_BIN").or_else(discover_mlx_bin),
            classify_model: env_path("ENSEMBLY_CLASSIFY_MODEL"),
            conflict_model: env_path("ENSEMBLY_CONFLICT_MODEL"),
            cloud_checks: cloud_checks_enabled(),
            cloud_cmd: parse_cloud_cmd(),
            confidence_floor: floor,
            timeout,
            context: DEFAULT_CONTEXT,
            max_tokens: DEFAULT_MAX_TOKENS,
            lock_path: env_path("ENSEMBLY_LOCAL_LOCK").unwrap_or_else(default_lock_path),
            prefer,
            memory_retries: 8,
            mlx_experimental: env_truthy("ENSEMBLY_MLX_EXPERIMENTAL"),
        }
    }

    /// Cloud is used only when both the opt-in flag and a command template are set.
    pub fn cloud_opted_in(&self) -> bool {
        self.cloud_checks && self.cloud_cmd.as_ref().is_some_and(|c| !c.is_empty())
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    env_var(name).map(PathBuf::from)
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

fn parse_cloud_cmd() -> Option<Vec<String>> {
    // Never auto-detect a cloud binary. The command is used only when
    // `cloud_checks_enabled()` is also true.
    let raw = env_var("ENSEMBLY_CLOUD_CHECK_CMD")?;
    let parts = shell_split(&raw);
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

fn shell_split(raw: &str) -> Vec<String> {
    // Minimal splitter for tests and documented env values (no nested quotes).
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in raw.chars() {
        match (quote, ch) {
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            (None, '"') | (None, '\'') => quote = Some(ch),
            (Some(q), c) if c == q => quote = None,
            (_, c) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn command_exists(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let path = Path::new(name);
    if name.contains('/') || name.contains('\\') || path.components().count() > 1 {
        return is_executable(path);
    }
    let Ok(path_var) = std::env::var("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return true;
        }
        #[cfg(windows)]
        {
            if !name.ends_with(".exe") {
                let exe = dir.join(format!("{name}.exe"));
                if is_executable(&exe) {
                    return true;
                }
            }
        }
    }
    false
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn discover_llama_bin() -> Option<PathBuf> {
    let candidates = [
        "llama-cli",
        "llama-completion",
        "llama-cli.exe",
        "llama-completion.exe",
    ];
    for name in candidates {
        if command_exists(name) {
            return Some(PathBuf::from(name));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let prefix = PathBuf::from(home).join(".local/share/ensembly");
        for rel in [
            "runtime/llama-b12343/llama-completion",
            "runtime/llama-b12343/llama-cli",
            "bin/llama-completion",
            "bin/llama-cli",
        ] {
            let pinned = prefix.join(rel);
            if pinned.is_file() {
                return Some(pinned);
            }
        }
    }
    None
}

fn discover_mlx_bin() -> Option<PathBuf> {
    if command_exists("mlx_lm") {
        return Some(PathBuf::from("mlx_lm"));
    }
    // python -m mlx_lm.generate
    if command_exists("python3") {
        let ok = Command::new("python3")
            .args(["-c", "import mlx_lm"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Some(PathBuf::from("python3-mlx"));
        }
    }
    None
}

/// Opt-in gate for live model processes. CI stays on recorded fixtures.
pub fn live_models_enabled() -> bool {
    env_truthy("ENSEMBLY_LOCAL_MODELS")
}

/// Opt-in gate for cloud fallback. Off unless set, even if a command is present.
pub fn cloud_checks_enabled() -> bool {
    env_truthy("ENSEMBLY_CLOUD_CHECKS")
}

/// True when `name` is set to `1`, `true`, `yes`, or `on` (ASCII case-insensitive).
/// Any other value, including empty or unset, is false. Only `ENSEMBLY_*`
/// names are read; there is no legacy alias.
fn env_truthy(name: &str) -> bool {
    matches!(
        env_var(name)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Log line for a cloud call. Never includes the prompt or statement text.
pub fn cloud_call_log_line(reason: FallbackReason, argv0: &str) -> String {
    format!(
        "ensembly-local-check: cloud call reason={} argv0={}",
        reason.as_str(),
        argv0
    )
}

#[derive(Debug, Clone)]
pub struct LocalChecker<M = HostMemory> {
    pub config: LocalCheckConfig,
    memory: M,
}

impl LocalChecker<HostMemory> {
    pub fn from_env() -> Self {
        Self {
            config: LocalCheckConfig::from_env(),
            memory: HostMemory,
        }
    }
}

impl<M: MemoryView> LocalChecker<M> {
    pub fn new(config: LocalCheckConfig, memory: M) -> Self {
        Self { config, memory }
    }

    pub fn classify(&self, statement: &str) -> CheckResult<FactClass> {
        self.run(CheckKind::Classify, statement, None, parse_fact_class)
    }

    pub fn conflict(&self, left: &str, right: &str) -> CheckResult<ConflictClass> {
        self.run(CheckKind::Conflict, left, Some(right), parse_conflict_class)
    }

    fn run<T, F>(
        &self,
        kind: CheckKind,
        left: &str,
        right: Option<&str>,
        parse: F,
    ) -> CheckResult<T>
    where
        F: Fn(&str) -> Result<T, String>,
        T: HasConfidence,
    {
        let started = Instant::now();
        match self.run_local(kind, left, right, &parse, started) {
            Ok(success) => {
                if success.value.confidence() < self.config.confidence_floor {
                    let detail = format!(
                        "local confidence {:.2} is below {:.2}",
                        success.value.confidence(),
                        self.config.confidence_floor
                    );
                    if self.config.cloud_opted_in() {
                        return self.cloud_or_fail(
                            CloudCtx {
                                kind,
                                left,
                                right,
                                started,
                            },
                            parse,
                            FallbackReason::LowConfidence,
                            Some(success.raw),
                            detail,
                        );
                    }
                    return Err(CheckFailure {
                        reason: FallbackReason::LowConfidence,
                        detail: format!(
                            "{detail}; cloud fallback is off (set ENSEMBLY_CLOUD_CHECKS=1 and ENSEMBLY_CLOUD_CHECK_CMD to opt in)"
                        ),
                        latency: started.elapsed(),
                        raw: Some(success.raw),
                    });
                }
                Ok(success)
            }
            // Lock, memory, missing model/runtime, invalid JSON, and any other
            // local error stay local. They never escalate to the cloud.
            Err(failure) => Err(failure),
        }
    }

    fn run_local<T, F>(
        &self,
        kind: CheckKind,
        left: &str,
        right: Option<&str>,
        parse: &F,
        started: Instant,
    ) -> CheckResult<T>
    where
        F: Fn(&str) -> Result<T, String>,
    {
        let runtime = self.pick_runtime()?;
        let model = self.model_path(kind)?;
        let size = fs::metadata(&model).map(|m| m.len()).map_err(|e| {
            fail(
                FallbackReason::ModelMissing,
                started,
                format!("model stat: {e}"),
            )
        })?;
        self.wait_for_memory(size, started)?;
        let _lock = ModelLock::acquire(&self.config.lock_path)
            .map_err(|e| fail(FallbackReason::LockBusy, started, e))?;
        // Retry once on invalid JSON, then return InvalidJson (no cloud).
        let mut last_raw = String::new();
        let mut last_err = String::new();
        for _ in 0..2 {
            let raw = self
                .invoke(runtime, kind, &model, left, right)
                .map_err(|e| fail(FallbackReason::RuntimeMissing, started, e))?;
            last_raw = raw.clone();
            match parse(&raw) {
                Ok(value) => {
                    return Ok(CheckSuccess {
                        value,
                        source: CheckSource {
                            runtime,
                            model: display_model(&model),
                            fallback: None,
                        },
                        latency: started.elapsed(),
                        raw,
                    });
                }
                Err(err) => last_err = err,
            }
        }
        Err(CheckFailure {
            reason: FallbackReason::InvalidJson,
            detail: last_err,
            latency: started.elapsed(),
            raw: Some(last_raw),
        })
    }

    fn cloud_or_fail<T, F>(
        &self,
        ctx: CloudCtx<'_>,
        parse: F,
        reason: FallbackReason,
        raw: Option<String>,
        detail: String,
    ) -> CheckResult<T>
    where
        F: Fn(&str) -> Result<T, String>,
    {
        let CloudCtx {
            kind,
            left,
            right,
            started,
        } = ctx;
        match self.invoke_cloud(kind, left, right) {
            Ok(cloud_raw) => match parse(&cloud_raw) {
                Ok(value) => Ok(CheckSuccess {
                    value,
                    source: CheckSource {
                        runtime: RuntimeKind::Cloud,
                        model: "cloud-fallback".into(),
                        fallback: Some(reason),
                    },
                    latency: started.elapsed(),
                    raw: cloud_raw,
                }),
                Err(err) => Err(CheckFailure {
                    reason: FallbackReason::InvalidJson,
                    detail: format!("{detail}; cloud parse failed: {err}"),
                    latency: started.elapsed(),
                    raw: Some(cloud_raw),
                }),
            },
            Err((cloud_reason, cloud_detail)) => Err(CheckFailure {
                reason: if cloud_reason == FallbackReason::CloudBlocked {
                    FallbackReason::CloudBlocked
                } else {
                    reason
                },
                detail: format!("{detail}; cloud fallback: {cloud_detail}"),
                latency: started.elapsed(),
                raw,
            }),
        }
    }

    fn pick_runtime(&self) -> Result<RuntimeKind, CheckFailure> {
        let mlx_ok = self.config.mlx_bin.is_some() && self.config.mlx_experimental;
        match self.config.prefer {
            RuntimeKind::Mlx if mlx_ok => Ok(RuntimeKind::Mlx),
            RuntimeKind::LlamaCpp if self.config.llama_bin.is_some() => Ok(RuntimeKind::LlamaCpp),
            _ if self.config.llama_bin.is_some() => Ok(RuntimeKind::LlamaCpp),
            _ if mlx_ok => Ok(RuntimeKind::Mlx),
            _ if self.config.mlx_bin.is_some() && !self.config.mlx_experimental => Err(fail(
                FallbackReason::RuntimeMissing,
                Instant::now(),
                "MLX is not used for production checks until it supports schema-constrained JSON (set ENSEMBLY_MLX_EXPERIMENTAL=1 to override)".into(),
            )),
            _ => Err(fail(
                FallbackReason::RuntimeMissing,
                Instant::now(),
                "llama.cpp and MLX are not installed".into(),
            )),
        }
    }

    fn model_path(&self, kind: CheckKind) -> Result<PathBuf, CheckFailure> {
        let path = match kind {
            CheckKind::Classify => self.config.classify_model.clone(),
            CheckKind::Conflict => self.config.conflict_model.clone(),
        };
        match path {
            Some(p) if p.is_file() || is_mock_model(&p) => Ok(p),
            Some(p) => Err(fail(
                FallbackReason::ModelMissing,
                Instant::now(),
                format!("{} is not a file", display_model(&p)),
            )),
            None => Err(fail(
                FallbackReason::ModelMissing,
                Instant::now(),
                format!("no model configured for {}", kind.model_role()),
            )),
        }
    }

    fn wait_for_memory(&self, model_size: u64, started: Instant) -> Result<u64, CheckFailure> {
        let needed = required_bytes_for_model(model_size);
        let attempts = self.config.memory_retries.saturating_add(1);
        let mut last = 0u64;
        for i in 0..attempts {
            match self.memory.available_bytes() {
                Some(available) if can_load_model(available, model_size) => return Ok(available),
                Some(available) => last = available,
                None => {
                    return Err(fail(
                        FallbackReason::NotEnoughMemory,
                        started,
                        "could not measure free memory".into(),
                    ))
                }
            }
            if i + 1 < attempts {
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        Err(fail(
            FallbackReason::NotEnoughMemory,
            started,
            format!(
                "need {needed} bytes (model + headroom), have {last} bytes available; will not load a local model"
            ),
        ))
    }

    fn invoke(
        &self,
        runtime: RuntimeKind,
        kind: CheckKind,
        model: &Path,
        left: &str,
        right: Option<&str>,
    ) -> Result<String, String> {
        match runtime {
            RuntimeKind::LlamaCpp => self.invoke_llama(kind, model, left, right),
            RuntimeKind::Mlx => self.invoke_mlx(kind, model, left, right),
            RuntimeKind::Cloud | RuntimeKind::Replay => {
                Err("internal: invoke called for a non-local runtime".into())
            }
        }
    }

    fn invoke_llama(
        &self,
        kind: CheckKind,
        model: &Path,
        left: &str,
        right: Option<&str>,
    ) -> Result<String, String> {
        let bin = self
            .config
            .llama_bin
            .as_ref()
            .ok_or_else(|| "llama.cpp binary is not configured".to_string())?;
        let (system, user, schema) = prompts(kind, left, right);
        let mut cmd = Command::new(bin);
        cmd.arg("-m")
            .arg(model)
            .arg("-sys")
            .arg(&system)
            .arg("-p")
            .arg(&user)
            .arg("--json-schema")
            .arg(schema.to_string())
            .arg("-c")
            .arg(self.config.context.to_string())
            .arg("-n")
            .arg(self.config.max_tokens.to_string())
            .arg("--temp")
            .arg("0")
            .arg("--min-p")
            .arg("0")
            .arg("--top-p")
            .arg("1")
            .arg("--top-k")
            .arg("1")
            .arg("--no-display-prompt")
            .arg("--reasoning")
            .arg("off")
            .arg("-no-cnv")
            .arg("-ngl")
            .arg("99");
        // llama-cli starts a chat session unless asked to stop after one turn.
        if bin
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("cli"))
        {
            cmd.arg("-st").arg("--simple-io");
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        run_cmd(cmd, self.config.timeout)
    }

    fn invoke_mlx(
        &self,
        kind: CheckKind,
        model: &Path,
        left: &str,
        right: Option<&str>,
    ) -> Result<String, String> {
        // Reached only when `mlx_experimental` is on. mlx_lm.generate has no
        // `--json-schema` flag; output is parsed after the fact.
        let bin = self
            .config
            .mlx_bin
            .as_ref()
            .ok_or_else(|| "MLX binary is not configured".to_string())?;
        let (system, user, _) = prompts(kind, left, right);
        let prompt = format!("{system}\n\n{user}");
        let mut cmd = if bin == Path::new("python3-mlx") {
            let mut c = Command::new("python3");
            c.args(["-m", "mlx_lm.generate"]);
            c
        } else {
            Command::new(bin)
        };
        cmd.arg("--model")
            .arg(model)
            .arg("--prompt")
            .arg(prompt)
            .arg("--max-tokens")
            .arg(self.config.max_tokens.to_string())
            .arg("--temp")
            .arg("0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        run_cmd(cmd, self.config.timeout)
    }

    fn invoke_cloud(
        &self,
        kind: CheckKind,
        left: &str,
        right: Option<&str>,
    ) -> Result<String, (FallbackReason, String)> {
        if !self.config.cloud_opted_in() {
            return Err((
                FallbackReason::CloudMissing,
                "cloud fallback is off (set ENSEMBLY_CLOUD_CHECKS=1 and ENSEMBLY_CLOUD_CHECK_CMD to opt in)".into(),
            ));
        }
        let mut parts = vec![left];
        if let Some(r) = right {
            parts.push(r);
        }
        if let Err(err) = assert_safe_for_cloud(&parts) {
            return Err((FallbackReason::CloudBlocked, err));
        }
        let Some(template) = &self.config.cloud_cmd else {
            return Err((
                FallbackReason::CloudMissing,
                "no cloud fallback command is configured".into(),
            ));
        };
        let (system, user, schema) = prompts(kind, left, right);
        let prompt = format!("{system}\n\n{user}\n\nThe JSON must match this schema:\n{schema}");
        let mut cmd_parts = template.clone();
        for part in &mut cmd_parts {
            if part.contains("{{prompt}}") {
                *part = part.replace("{{prompt}}", &prompt);
            }
        }
        let (bin, args) = cmd_parts.split_first().ok_or((
            FallbackReason::CloudMissing,
            "cloud command is empty".into(),
        ))?;
        let mut cmd = Command::new(bin);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
        // If the template has no {{prompt}} placeholder, pass the prompt last.
        if !template.iter().any(|p| p.contains("{{prompt}}")) {
            cmd.arg(&prompt);
        }
        eprintln!(
            "{}",
            cloud_call_log_line(FallbackReason::LowConfidence, bin)
        );
        run_cmd(cmd, self.config.timeout).map_err(|e| (FallbackReason::CloudMissing, e))
    }
}

trait HasConfidence {
    fn confidence(&self) -> f32;
}

impl HasConfidence for FactClass {
    fn confidence(&self) -> f32 {
        self.confidence
    }
}

impl HasConfidence for ConflictClass {
    fn confidence(&self) -> f32 {
        self.confidence
    }
}

fn prompts(
    kind: CheckKind,
    left: &str,
    right: Option<&str>,
) -> (String, String, serde_json::Value) {
    match kind {
        CheckKind::Classify => (
            classify_system_prompt().to_string(),
            classify_user_prompt(left),
            fact_json_schema(),
        ),
        CheckKind::Conflict => (
            conflict_system_prompt().to_string(),
            conflict_user_prompt(left, right.unwrap_or("")),
            conflict_json_schema(),
        ),
    }
}

fn run_cmd(mut cmd: Command, timeout: Duration) -> Result<String, String> {
    let started = Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to start local check process: {e}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_h = std::thread::spawn(move || match stdout {
        Some(r) => read_capped(r, MAX_STDOUT_BYTES),
        None => CappedRead::Ok(Vec::new()),
    });
    let stderr_h = std::thread::spawn(move || match stderr {
        Some(r) => read_capped(r, MAX_STDOUT_BYTES),
        None => CappedRead::Ok(Vec::new()),
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_h.join();
                let _ = stderr_h.join();
                return Err(format!(
                    "local check timed out after {}s",
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(err) => return Err(format!("local check wait: {err}")),
        }
    };
    let stdout = stdout_h.join().unwrap_or(CappedRead::Ok(Vec::new()));
    let stderr = stderr_h.join().unwrap_or(CappedRead::Ok(Vec::new()));
    if matches!(stdout, CappedRead::Exceeded) || matches!(stderr, CappedRead::Exceeded) {
        return Err(format!(
            "local check output exceeded {MAX_STDOUT_BYTES} bytes"
        ));
    }
    let stdout = match stdout {
        CappedRead::Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        CappedRead::Exceeded => unreachable!(),
    };
    if !status.success() {
        let stderr = match stderr {
            CappedRead::Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            CappedRead::Exceeded => String::new(),
        };
        return Err(format!(
            "local check exited {}: {stderr}",
            status.code().unwrap_or(-1)
        ));
    }
    Ok(stdout)
}

enum CappedRead {
    Ok(Vec<u8>),
    Exceeded,
}

fn read_capped<R: Read>(mut reader: R, cap: usize) -> CappedRead {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        match reader.read(&mut tmp) {
            Ok(0) => return CappedRead::Ok(buf),
            Ok(n) => {
                if buf.len().saturating_add(n) > cap {
                    let mut sink = [0u8; 8192];
                    while matches!(reader.read(&mut sink), Ok(k) if k > 0) {}
                    return CappedRead::Exceeded;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return CappedRead::Ok(buf),
        }
    }
}

fn fail(reason: FallbackReason, started: Instant, detail: String) -> CheckFailure {
    CheckFailure {
        reason,
        detail,
        latency: started.elapsed(),
        raw: None,
    }
}

/// Never print a home path or user folder in logs, docs, or CI output.
pub fn display_model(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if let Ok(home) = std::env::var("HOME") {
        if let Some(rest) = raw.strip_prefix(&home) {
            return format!("$HOME{rest}");
        }
    }
    if let Some(name) = path.file_name() {
        return name.to_string_lossy().into();
    }
    "model".into()
}

fn is_mock_model(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e == "mock")
        || path.to_string_lossy().contains("fixtures/local-checks")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_checks::support::MEMORY_HEADROOM_BYTES;
    use std::fs::File;
    use std::io::Write;

    struct FixedMemory(u64);
    impl MemoryView for FixedMemory {
        fn available_bytes(&self) -> Option<u64> {
            Some(self.0)
        }
    }

    fn write_exec(path: &Path, body: &str) {
        let mut file = File::create(path).unwrap();
        file.write_all(body.as_bytes()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = file.metadata().unwrap().permissions();
            perm.set_mode(0o755);
            fs::set_permissions(path, perm).unwrap();
        }
    }

    fn classify_script(json: &str) -> String {
        format!("#!/bin/sh\nprintf '%s\\n' '{json}'\n")
    }

    fn base_config(dir: &Path, script: &str, model: &str) -> LocalCheckConfig {
        let bin = dir.join("llama-cli");
        write_exec(&bin, script);
        let model_path = dir.join(model);
        fs::write(&model_path, b"mock-weights").unwrap();
        LocalCheckConfig {
            llama_bin: Some(bin),
            mlx_bin: None,
            classify_model: Some(model_path.clone()),
            conflict_model: Some(model_path),
            cloud_checks: false,
            cloud_cmd: None,
            confidence_floor: DEFAULT_CONFIDENCE_FLOOR,
            timeout: Duration::from_secs(5),
            context: 512,
            max_tokens: 64,
            lock_path: dir.join("model.lock"),
            prefer: RuntimeKind::LlamaCpp,
            memory_retries: 0,
            mlx_experimental: false,
        }
    }

    fn write_cloud(dir: &Path, json: Option<&str>) -> (Vec<String>, PathBuf) {
        let sentinel = dir.join("cloud-called");
        let cloud = dir.join("cloud.sh");
        let body = match json {
            Some(j) => format!(
                "#!/bin/sh\n: > \"{}\"\nprintf '%s\\n' '{j}'\n",
                sentinel.display()
            ),
            None => format!(
                "#!/bin/sh\n: > \"{}\"\necho should-not-be-called >&2; exit 1\n",
                sentinel.display()
            ),
        };
        write_exec(&cloud, &body);
        (
            vec![cloud.to_string_lossy().into(), "{{prompt}}".into()],
            sentinel,
        )
    }

    #[test]
    fn mock_llama_classifies_json() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = base_config(
            dir.path(),
            &classify_script(
                r#"{"label":"fact","confidence":0.93,"rationale":"States a countable stock figure."}"#,
            ),
            "qwen.gguf",
        );
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let got = checker
            .classify("The Northbridge depot of River Co. holds 12 teal crates.")
            .unwrap();
        assert_eq!(
            got.value.label,
            crate::local_checks::schema::FactLabel::Fact
        );
        assert_eq!(got.source.runtime, RuntimeKind::LlamaCpp);
        assert!(got.source.fallback.is_none());
    }

    #[test]
    fn low_confidence_uses_cloud_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = base_config(
            dir.path(),
            &classify_script(
                r#"{"label":"uncertain","confidence":0.21,"rationale":"The local model is unsure."}"#,
            ),
            "qwen.gguf",
        );
        let (cmd, sentinel) = write_cloud(
            dir.path(),
            Some(
                r#"{"label":"fact","confidence":0.81,"rationale":"Cloud fallback parsed the stock figure."}"#,
            ),
        );
        cfg.cloud_checks = true;
        cfg.cloud_cmd = Some(cmd);
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let statement = "The Northbridge depot of River Co. holds 12 teal crates.";
        let got = checker.classify(statement).unwrap();
        assert_eq!(
            got.value.label,
            crate::local_checks::schema::FactLabel::Fact
        );
        assert_eq!(got.source.runtime, RuntimeKind::Cloud);
        assert_eq!(got.source.fallback, Some(FallbackReason::LowConfidence));
        assert!(sentinel.is_file(), "opted-in cloud should have been called");
        let log = cloud_call_log_line(FallbackReason::LowConfidence, "cloud.sh");
        assert!(log.contains("reason=low-confidence"));
        assert!(!log.contains(statement));
        assert!(!log.contains("teal crates"));
    }

    #[test]
    fn cloud_requires_both_flag_and_command() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = base_config(
            dir.path(),
            &classify_script(
                r#"{"label":"uncertain","confidence":0.21,"rationale":"The local model is unsure."}"#,
            ),
            "qwen.gguf",
        );
        let (cmd, sentinel) = write_cloud(
            dir.path(),
            Some(
                r#"{"label":"fact","confidence":0.81,"rationale":"Should not run without the flag."}"#,
            ),
        );
        cfg.cloud_checks = false;
        cfg.cloud_cmd = Some(cmd.clone());
        assert!(!cfg.cloud_opted_in());
        let checker = LocalChecker::new(cfg.clone(), FixedMemory(8 * 1024 * 1024 * 1024));
        let err = checker
            .classify("The Northbridge depot of River Co. holds 12 teal crates.")
            .unwrap_err();
        assert_eq!(err.reason, FallbackReason::LowConfidence);
        assert!(
            err.detail.contains("cloud fallback is off"),
            "{}",
            err.detail
        );
        assert!(!sentinel.is_file());

        cfg.cloud_checks = true;
        cfg.cloud_cmd = None;
        assert!(!cfg.cloud_opted_in());
        cfg.cloud_cmd = Some(vec![]);
        assert!(!cfg.cloud_opted_in());
        cfg.cloud_cmd = Some(cmd);
        assert!(cfg.cloud_opted_in());
    }

    #[test]
    fn missing_runtime_does_not_call_cloud() {
        let dir = tempfile::tempdir().unwrap();
        let (cmd, sentinel) = write_cloud(
            dir.path(),
            Some(
                r#"{"label":"opinion","confidence":0.77,"rationale":"Cloud must not run on a local error."}"#,
            ),
        );
        let cfg = LocalCheckConfig {
            llama_bin: None,
            mlx_bin: None,
            classify_model: Some(dir.path().join("missing.gguf")),
            conflict_model: None,
            cloud_checks: true,
            cloud_cmd: Some(cmd),
            confidence_floor: DEFAULT_CONFIDENCE_FLOOR,
            timeout: Duration::from_secs(5),
            context: 512,
            max_tokens: 64,
            lock_path: dir.path().join("model.lock"),
            prefer: RuntimeKind::LlamaCpp,
            memory_retries: 0,
            mlx_experimental: false,
        };
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let err = checker
            .classify("River Co. makes the most cheerful crates in the valley.")
            .unwrap_err();
        assert_eq!(err.reason, FallbackReason::RuntimeMissing);
        assert!(
            !sentinel.is_file(),
            "cloud must not run on a missing runtime"
        );
    }

    #[test]
    fn memory_shortfall_does_not_start_the_model() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = base_config(
            dir.path(),
            "#!/bin/sh\necho should-not-run >&2; exit 1\n",
            "qwen.gguf",
        );
        let model_size = fs::metadata(cfg.classify_model.as_ref().unwrap())
            .unwrap()
            .len();
        let (cmd, sentinel) = write_cloud(dir.path(), None);
        cfg.cloud_checks = true;
        cfg.cloud_cmd = Some(cmd);
        let checker = LocalChecker::new(cfg, FixedMemory(model_size)); // no headroom
        let err = checker
            .classify("The Northbridge depot of River Co. holds 12 teal crates.")
            .unwrap_err();
        assert_eq!(err.reason, FallbackReason::NotEnoughMemory);
        assert!(err.detail.contains("headroom") || err.detail.contains("available"));
        assert!(
            !sentinel.is_file(),
            "cloud must not run on a memory refusal"
        );
        let _ = MEMORY_HEADROOM_BYTES;
    }

    #[test]
    fn cloud_is_refused_for_personal_data() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = base_config(
            dir.path(),
            &classify_script(
                r#"{"label":"uncertain","confidence":0.21,"rationale":"The local model is unsure."}"#,
            ),
            "qwen.gguf",
        );
        let (cmd, sentinel) = write_cloud(dir.path(), None);
        cfg.cloud_checks = true;
        cfg.cloud_cmd = Some(cmd);
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let mail = format!("{}@{}", "ada", "example.com");
        let err = checker
            .classify(&format!("mail me at {mail} please"))
            .unwrap_err();
        assert_eq!(err.reason, FallbackReason::CloudBlocked);
        assert!(err.detail.contains("cloud fallback"), "{}", err.detail);
        assert!(
            err.detail.contains("email") || err.detail.contains("refusing"),
            "{}",
            err.detail
        );
        assert!(
            !sentinel.is_file(),
            "redaction must refuse the call entirely"
        );
    }

    #[test]
    fn invalid_json_retries_then_stays_local() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = base_config(
            dir.path(),
            "#!/bin/sh\nprintf '%s\\n' 'not-json'\n",
            "qwen.gguf",
        );
        let (cmd, sentinel) = write_cloud(
            dir.path(),
            Some(
                r#"{"label":"other","confidence":0.7,"rationale":"Cloud must not recover invalid local JSON."}"#,
            ),
        );
        cfg.cloud_checks = true;
        cfg.cloud_cmd = Some(cmd);
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let err = checker
            .classify("Hello, this is a test ping with no claim.")
            .unwrap_err();
        assert_eq!(err.reason, FallbackReason::InvalidJson);
        assert!(
            !sentinel.is_file(),
            "cloud must not run on invalid local JSON"
        );
    }

    #[test]
    fn missing_model_does_not_call_cloud() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = base_config(
            dir.path(),
            &classify_script(r#"{"label":"fact","confidence":0.9,"rationale":"Should not run."}"#),
            "qwen.gguf",
        );
        cfg.classify_model = Some(dir.path().join("absent.gguf"));
        let (cmd, sentinel) = write_cloud(dir.path(), None);
        cfg.cloud_checks = true;
        cfg.cloud_cmd = Some(cmd);
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let err = checker
            .classify("The Northbridge depot of River Co. holds 12 teal crates.")
            .unwrap_err();
        assert_eq!(err.reason, FallbackReason::ModelMissing);
        assert!(!sentinel.is_file());
    }

    #[test]
    fn lock_busy_does_not_call_cloud() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = base_config(
            dir.path(),
            &classify_script(r#"{"label":"fact","confidence":0.9,"rationale":"Should not run."}"#),
            "qwen.gguf",
        );
        let (cmd, sentinel) = write_cloud(dir.path(), None);
        cfg.cloud_checks = true;
        cfg.cloud_cmd = Some(cmd);
        let _held = crate::local_checks::support::ModelLock::acquire(&cfg.lock_path).unwrap();
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let err = checker
            .classify("The Northbridge depot of River Co. holds 12 teal crates.")
            .unwrap_err();
        assert_eq!(err.reason, FallbackReason::LockBusy);
        assert!(
            !sentinel.is_file(),
            "cloud must not run when the lock is busy"
        );
    }

    #[test]
    fn mlx_is_rejected_without_experimental_flag() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = base_config(
            dir.path(),
            "#!/bin/sh\necho should-not-run >&2; exit 1\n",
            "qwen.gguf",
        );
        cfg.llama_bin = None;
        cfg.mlx_bin = Some(dir.path().join("mlx"));
        cfg.mlx_experimental = false;
        let (cmd, sentinel) = write_cloud(dir.path(), None);
        cfg.cloud_checks = true;
        cfg.cloud_cmd = Some(cmd);
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let err = checker
            .classify("The Northbridge depot of River Co. holds 12 teal crates.")
            .unwrap_err();
        assert_eq!(err.reason, FallbackReason::RuntimeMissing);
        assert!(
            err.detail.contains("schema-constrained") || err.detail.contains("MLX"),
            "{}",
            err.detail
        );
        assert!(!sentinel.is_file());
    }

    #[test]
    fn stdout_over_the_cap_fails_safe() {
        let dir = tempfile::tempdir().unwrap();
        let script = "#!/bin/sh\ndd if=/dev/zero bs=1024 count=80 2>/dev/null | tr '\\0' 'a'\n";
        let cfg = base_config(dir.path(), script, "qwen.gguf");
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let err = checker
            .classify("The Northbridge depot of River Co. holds 12 teal crates.")
            .unwrap_err();
        assert_eq!(err.reason, FallbackReason::RuntimeMissing);
        assert!(
            err.detail.contains("exceeded") && err.detail.contains(&MAX_STDOUT_BYTES.to_string()),
            "{}",
            err.detail
        );
    }
}
