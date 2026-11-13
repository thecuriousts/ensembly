//! `ensembly doctor`: versions, harness and plugin state. Reads only.

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const PRODUCT: &str = "ensembly-cli";
pub const DEFAULT_REMOTE: &str = "https://github.com/thecuriousts/ensembly";
const PLUGINS: [&str; 2] = ["flowingstack", "ensembly"];
const TOOLS: [&str; 4] = ["git", "python3", "claude", "cursor-agent"];

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Install {
    pub harness: &'static str,
    pub plugin: String,
    pub version: String,
    pub ok: bool,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub product: &'static str,
    pub version: &'static str,
    pub kernel: &'static str,
    pub platform: String,
    pub tools: Vec<(String, Option<String>)>,
    pub claude_code: String,
    pub cursor: String,
    pub installs: Vec<Install>,
    pub ops_db: String,
    pub latest: String,
    pub failures: Vec<String>,
}

impl Report {
    pub fn exit_code(&self) -> i32 {
        i32::from(!self.failures.is_empty())
    }

    pub fn render(&self) -> String {
        let mut out = vec![
            format!("{} {}", self.product, self.version),
            format!("kernel {}", self.kernel),
            format!("platform {}", self.platform),
        ];
        for (t, v) in &self.tools {
            out.push(format!("tool {t} {}", v.as_deref().unwrap_or("missing")));
        }
        out.push(format!("harness claude-code {}", self.claude_code));
        out.push(format!("harness cursor {}", self.cursor));
        for i in &self.installs {
            let tag = if i.ok { "" } else { "fail " };
            out.push(format!(
                "{tag}plugin {} {} {}",
                i.harness, i.plugin, i.version
            ));
        }
        out.push(format!("ops-db {}", self.ops_db));
        out.push(format!("latest {}", self.latest));
        for f in &self.failures {
            out.push(format!("fail {f}"));
        }
        out.join("\n")
    }
}

pub struct Opts {
    pub home: PathBuf,
    pub claude_dir: PathBuf,
    pub ops_db: PathBuf,
    pub remote: String,
    pub offline: bool,
}

impl Opts {
    pub fn from_env(ops_db: PathBuf, offline: bool) -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let claude_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".claude"));
        let remote =
            std::env::var("ENSEMBLY_DOCTOR_REMOTE").unwrap_or_else(|_| DEFAULT_REMOTE.into());
        let offline = offline || std::env::var("ENSEMBLY_OFFLINE").is_ok_and(|v| v == "1");
        Self {
            home,
            claude_dir,
            ops_db,
            remote,
            offline,
        }
    }
}

pub fn run(o: &Opts) -> Report {
    let mut failures = Vec::new();
    let mut installs = Vec::new();
    let claude_code = claude_installs(&o.claude_dir, &mut installs, &mut failures);
    let cursor = cursor_installs(&o.home.join(".cursor/plugins"), &mut installs);
    for i in installs.iter().filter(|i| !i.ok) {
        failures.push(format!(
            "{} {} install path missing, reinstall the plugin",
            i.harness, i.plugin
        ));
    }
    let ops_db = ops_db_state(&o.ops_db, &mut failures);
    let latest = if o.offline {
        "skipped".into()
    } else {
        latest_tag(&o.remote)
    };
    Report {
        product: PRODUCT,
        version: env!("CARGO_PKG_VERSION"),
        kernel: crate::kernel_version(),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        tools: TOOLS
            .iter()
            .map(|t| (t.to_string(), tool_version(t)))
            .collect(),
        claude_code,
        cursor,
        installs,
        ops_db,
        latest,
        failures,
    }
}

fn claude_installs(dir: &Path, out: &mut Vec<Install>, failures: &mut Vec<String>) -> String {
    if !dir.is_dir() {
        return "absent".into();
    }
    let path = dir.join("plugins/installed_plugins.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return "no-plugins".into();
    };
    let Some(plugins) = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("plugins").and_then(|p| p.as_object()).cloned())
    else {
        failures.push("claude-code installed_plugins.json unreadable".into());
        return "broken".into();
    };
    for (key, entries) in plugins {
        if !PLUGINS.contains(&key.split('@').next().unwrap_or_default()) {
            continue;
        }
        for e in entries.as_array().into_iter().flatten() {
            let path = e.get("installPath").and_then(|p| p.as_str()).unwrap_or("");
            out.push(Install {
                harness: "claude-code",
                plugin: key.clone(),
                version: e
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .into(),
                ok: Path::new(path).is_dir(),
            });
        }
    }
    "present".into()
}

fn cursor_installs(dir: &Path, out: &mut Vec<Install>) -> String {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return "absent".into();
    };
    let mut manifests = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        manifests.push(p.join(".cursor-plugin/plugin.json"));
        if let Ok(inner) = std::fs::read_dir(&p) {
            for i in inner.flatten() {
                manifests.push(i.path().join(".cursor-plugin/plugin.json"));
            }
        }
    }
    manifests.sort();
    for m in manifests {
        let Some(v) = std::fs::read_to_string(&m)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        else {
            continue;
        };
        let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if PLUGINS.contains(&name) {
            out.push(Install {
                harness: "cursor",
                plugin: name.into(),
                version: v
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .into(),
                ok: true,
            });
        }
    }
    "present".into()
}

fn ops_db_state(path: &Path, failures: &mut Vec<String>) -> String {
    if !path.is_file() {
        return "absent".into();
    }
    let check = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|c| c.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0)));
    match check {
        Ok(s) if s == "ok" => "ok".into(),
        Ok(s) => {
            failures.push(format!("ops-db quick_check {s}"));
            "broken".into()
        }
        Err(e) => {
            failures.push(format!("ops-db unreadable: {e}"));
            "broken".into()
        }
    }
}

fn output_within(cmd: &mut Command, limit: Duration) -> Option<String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < limit => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let out = child.wait_with_output().ok()?;
    out.status.success().then(|| {
        let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
        s.push_str(&String::from_utf8_lossy(&out.stderr));
        s
    })
}

fn tool_version(name: &str) -> Option<String> {
    let text = output_within(Command::new(name).arg("--version"), Duration::from_secs(10))?;
    let re = regex::Regex::new(r"\d+\.\d+(?:\.\d+)?").expect("static regex");
    Some(
        re.find(&text)
            .map_or("present".into(), |m| m.as_str().into()),
    )
}

fn semver(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .take(3)
        .filter_map(|s| s.parse().ok())
        .collect()
}

pub fn newest_tag(ls_remote: &str, product: &str) -> Option<String> {
    let prefix = format!("{product}--v");
    ls_remote
        .lines()
        .filter_map(|l| l.split("refs/tags/").nth(1))
        .filter(|t| t.starts_with(&prefix))
        .max_by_key(|t| semver(&t[prefix.len()..]))
        .map(str::to_string)
}

fn latest_tag(remote: &str) -> String {
    let pattern = format!("refs/tags/{PRODUCT}--v*");
    let out = output_within(
        Command::new("git")
            .args(["ls-remote", "--tags", "--refs", remote, &pattern])
            .env("GIT_TERMINAL_PROMPT", "0"),
        Duration::from_secs(8),
    );
    match out {
        None => "offline".into(),
        Some(text) => match newest_tag(&text, PRODUCT) {
            None => "none".into(),
            Some(t) if semver(&t[PRODUCT.len() + 3..]) > semver(env!("CARGO_PKG_VERSION")) => {
                format!("{t} update available")
            }
            Some(t) => t,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_tag_orders_by_semver_and_ignores_other_products() {
        let ls = "a\trefs/tags/ensembly-cli--v0.9.0\nb\trefs/tags/ensembly-cli--v0.10.0\nc\trefs/tags/flowingstack--v9.0.0\nd\trefs/tags/v1.0.0\n";
        assert_eq!(
            newest_tag(ls, PRODUCT).as_deref(),
            Some("ensembly-cli--v0.10.0")
        );
        assert_eq!(newest_tag("", PRODUCT), None);
    }
}
