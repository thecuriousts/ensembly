//! Local runtimes (llama.cpp, then MLX) and the command-based cloud fallback.
//!
//! No compile-time dependency on those tools. CI never needs the models:
//! tests inject a mock command or replay recorded synthetic JSON.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::env_alias::env_alias;
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
    pub cloud_cmd: Option<Vec<String>>,
    pub confidence_floor: f32,
    pub timeout: Duration,
    pub context: u32,
    pub max_tokens: u32,
    pub lock_path: PathBuf,
    pub prefer: RuntimeKind,
}

impl Default for LocalCheckConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl LocalCheckConfig {
    pub fn from_env() -> Self {
        let prefer = match env_alias("ENSEMBLY_LOCAL_RUNTIME", "PERAM_LOCAL_RUNTIME")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "mlx" => RuntimeKind::Mlx,
            _ => RuntimeKind::LlamaCpp,
        };
        let floor = env_alias(
            "ENSEMBLY_LOCAL_CONFIDENCE_MIN",
            "PERAM_LOCAL_CONFIDENCE_MIN",
        )
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_CONFIDENCE_FLOOR);
        let timeout = Duration::from_secs(
            env_alias("ENSEMBLY_LOCAL_TIMEOUT_SECS", "PERAM_LOCAL_TIMEOUT_SECS")
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_TIMEOUT_SECS),
        );
        Self {
            llama_bin: env_path("ENSEMBLY_LLAMA_CLI", "PERAM_LLAMA_CLI")
                .or_else(discover_llama_bin),
            mlx_bin: env_path("ENSEMBLY_MLX_BIN", "PERAM_MLX_BIN").or_else(discover_mlx_bin),
            classify_model: env_path("ENSEMBLY_CLASSIFY_MODEL", "PERAM_CLASSIFY_MODEL"),
            conflict_model: env_path("ENSEMBLY_CONFLICT_MODEL", "PERAM_CONFLICT_MODEL"),
            cloud_cmd: parse_cloud_cmd(),
            confidence_floor: floor,
            timeout,
            context: DEFAULT_CONTEXT,
            max_tokens: DEFAULT_MAX_TOKENS,
            lock_path: env_path("ENSEMBLY_LOCAL_LOCK", "PERAM_LOCAL_LOCK")
                .unwrap_or_else(default_lock_path),
            prefer,
        }
    }
}

fn env_path(primary: &str, legacy: &str) -> Option<PathBuf> {
    env_alias(primary, legacy).map(PathBuf::from)
}

fn parse_cloud_cmd() -> Option<Vec<String>> {
    if let Some(raw) = env_alias("ENSEMBLY_CLOUD_CHECK_CMD", "PERAM_CLOUD_CHECK_CMD") {
        let parts = shell_split(&raw);
        if !parts.is_empty() {
            return Some(parts);
        }
    }
    if command_exists("grok") {
        return Some(vec![
            "grok".into(),
            "--no-auto-update".into(),
            "-p".into(),
            "{{prompt}}".into(),
            "--output-format".into(),
            "json".into(),
        ]);
    }
    None
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
    Command::new("sh")
        .args(["-c", &format!("command -v {name} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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
        let pinned = PathBuf::from(home).join(".local/share/ensembly/bin/llama-cli");
        if pinned.is_file() {
            return Some(pinned);
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
    matches!(
        env_alias("ENSEMBLY_LOCAL_MODELS", "PERAM_LOCAL_MODELS")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
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
                        format!(
                            "local confidence {:.2} is below {:.2}",
                            success.value.confidence(),
                            self.config.confidence_floor
                        ),
                    );
                }
                Ok(success)
            }
            Err(failure) => self.cloud_or_fail(
                CloudCtx {
                    kind,
                    left,
                    right,
                    started,
                },
                parse,
                failure.reason,
                failure.raw,
                failure.detail,
            ),
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
        let available = self.memory.available_bytes().ok_or_else(|| {
            fail(
                FallbackReason::NotEnoughMemory,
                started,
                "could not measure free memory".into(),
            )
        })?;
        if !can_load_model(available, size) {
            return Err(fail(
                FallbackReason::NotEnoughMemory,
                started,
                format!(
                    "need {} bytes (model + headroom), have {} bytes available; will not load {}",
                    required_bytes_for_model(size),
                    available,
                    kind.model_role()
                ),
            ));
        }
        let _lock = ModelLock::acquire(&self.config.lock_path)
            .map_err(|e| fail(FallbackReason::RuntimeMissing, started, e))?;
        // Retry once on invalid JSON, then fall through to cloud.
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
        match self.config.prefer {
            RuntimeKind::Mlx if self.config.mlx_bin.is_some() => Ok(RuntimeKind::Mlx),
            RuntimeKind::LlamaCpp if self.config.llama_bin.is_some() => Ok(RuntimeKind::LlamaCpp),
            _ if self.config.llama_bin.is_some() => Ok(RuntimeKind::LlamaCpp),
            _ if self.config.mlx_bin.is_some() => Ok(RuntimeKind::Mlx),
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
            .arg("--no-display-prompt")
            .arg("-ngl")
            .arg("99")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        run_cmd(cmd, self.config.timeout)
    }

    fn invoke_mlx(
        &self,
        kind: CheckKind,
        model: &Path,
        left: &str,
        right: Option<&str>,
    ) -> Result<String, String> {
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
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "local check timed out after {}s",
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(err) => return Err(format!("local check wait: {err}")),
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("local check output: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "local check exited {}: {stderr}",
            output.status.code().unwrap_or(-1)
        ));
    }
    Ok(stdout)
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
            cloud_cmd: None,
            confidence_floor: DEFAULT_CONFIDENCE_FLOOR,
            timeout: Duration::from_secs(5),
            context: 512,
            max_tokens: 64,
            lock_path: dir.join("model.lock"),
            prefer: RuntimeKind::LlamaCpp,
        }
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
        let cloud = dir.path().join("cloud.sh");
        write_exec(
            &cloud,
            &classify_script(
                r#"{"label":"fact","confidence":0.81,"rationale":"Cloud fallback parsed the stock figure."}"#,
            ),
        );
        cfg.cloud_cmd = Some(vec![cloud.to_string_lossy().into(), "{{prompt}}".into()]);
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let got = checker
            .classify("The Northbridge depot of River Co. holds 12 teal crates.")
            .unwrap();
        assert_eq!(
            got.value.label,
            crate::local_checks::schema::FactLabel::Fact
        );
        assert_eq!(got.source.runtime, RuntimeKind::Cloud);
        assert_eq!(got.source.fallback, Some(FallbackReason::LowConfidence));
    }

    #[test]
    fn missing_runtime_falls_back_when_cloud_configured() {
        let dir = tempfile::tempdir().unwrap();
        let cloud = dir.path().join("cloud.sh");
        write_exec(
            &cloud,
            &classify_script(
                r#"{"label":"opinion","confidence":0.77,"rationale":"Cloud ran because no local runtime was present."}"#,
            ),
        );
        let cfg = LocalCheckConfig {
            llama_bin: None,
            mlx_bin: None,
            classify_model: Some(dir.path().join("missing.gguf")),
            conflict_model: None,
            cloud_cmd: Some(vec![cloud.to_string_lossy().into(), "{{prompt}}".into()]),
            confidence_floor: DEFAULT_CONFIDENCE_FLOOR,
            timeout: Duration::from_secs(5),
            context: 512,
            max_tokens: 64,
            lock_path: dir.path().join("model.lock"),
            prefer: RuntimeKind::LlamaCpp,
        };
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let got = checker
            .classify("River Co. makes the most cheerful crates in the valley.")
            .unwrap();
        assert_eq!(
            got.value.label,
            crate::local_checks::schema::FactLabel::Opinion
        );
        assert_eq!(got.source.fallback, Some(FallbackReason::RuntimeMissing));
    }

    #[test]
    fn memory_shortfall_does_not_start_the_model() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = base_config(
            dir.path(),
            "#!/bin/sh\necho should-not-run >&2; exit 1\n",
            "qwen.gguf",
        );
        let model_size = fs::metadata(cfg.classify_model.as_ref().unwrap())
            .unwrap()
            .len();
        let checker = LocalChecker::new(cfg, FixedMemory(model_size)); // no headroom
        let err = checker
            .classify("The Northbridge depot of River Co. holds 12 teal crates.")
            .unwrap_err();
        assert_eq!(err.reason, FallbackReason::NotEnoughMemory);
        assert!(err.detail.contains("headroom") || err.detail.contains("available"));
        let _ = MEMORY_HEADROOM_BYTES;
    }

    #[test]
    fn cloud_is_refused_for_personal_data() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = base_config(dir.path(), "#!/bin/sh\nexit 1\n", "qwen.gguf");
        cfg.llama_bin = None; // force fallback
        let cloud = dir.path().join("cloud.sh");
        write_exec(&cloud, "#!/bin/sh\necho should-not-be-called >&2; exit 1\n");
        cfg.cloud_cmd = Some(vec![cloud.to_string_lossy().into(), "{{prompt}}".into()]);
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let err = checker
            .classify("mail me at ada@example.com please")
            .unwrap_err();
        assert_eq!(err.reason, FallbackReason::CloudBlocked);
        assert!(err.detail.contains("cloud fallback"), "{}", err.detail);
        assert!(
            err.detail.contains("email") || err.detail.contains("refusing"),
            "{}",
            err.detail
        );
    }

    #[test]
    fn invalid_json_retries_then_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = base_config(
            dir.path(),
            "#!/bin/sh\nprintf '%s\\n' 'not-json'\n",
            "qwen.gguf",
        );
        let cloud = dir.path().join("cloud.sh");
        write_exec(
            &cloud,
            &classify_script(
                r#"{"label":"other","confidence":0.7,"rationale":"Cloud recovered after invalid local JSON."}"#,
            ),
        );
        cfg.cloud_cmd = Some(vec![cloud.to_string_lossy().into(), "{{prompt}}".into()]);
        let checker = LocalChecker::new(cfg, FixedMemory(8 * 1024 * 1024 * 1024));
        let got = checker
            .classify("Hello, this is a test ping with no claim.")
            .unwrap();
        assert_eq!(
            got.value.label,
            crate::local_checks::schema::FactLabel::Other
        );
        assert_eq!(got.source.fallback, Some(FallbackReason::InvalidJson));
    }
}
