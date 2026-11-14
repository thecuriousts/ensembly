//! CLI golden: pulse refusal (core kernel; no opportunity-finder feature).

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const PULSE_REFUSED: &str = include_str!("golden/pulse-refused.txt");
const SPEC: &str = "\
# Fixture spec

| Mission | Owner | Horizon | Current G text source | Links |
|---|---|---|---|---|
| Acme plan | Desk One | short | notes only | none |
";

fn cmd(dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ensembly"));
    c.arg("--no-memory").arg("--db").arg(dir.join("ops.sqlite"));
    c
}

fn run(cmd: &mut Command) -> Output {
    cmd.output().expect("run ensembly")
}

#[test]
fn pulse_refuse_write_message_is_frozen() {
    let dir = tempfile::tempdir().expect("temp");
    pulse(dir.path());
}

fn pulse(dir: &Path) {
    let spec = dir.join("spec.md");
    fs::write(&spec, SPEC).unwrap();
    fs::create_dir_all(dir.join("g")).unwrap();
    fs::create_dir_all(dir.join("home")).unwrap();
    fs::create_dir_all(dir.join("notes")).unwrap();
    let missions = dir.join("missions");
    let first = run(&mut pulse_cmd(dir, &spec, &missions));
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = run(&mut pulse_cmd(dir, &spec, &missions));
    let err = String::from_utf8(second.stderr).expect("utf8");
    let path = missions.join("missions.json").display().to_string();
    let err = err.replace(&path, "<missions>");
    let code = second.status.code().expect("code");
    assert_eq!(format!("{code}\n{err}"), PULSE_REFUSED);
}

fn pulse_cmd(dir: &Path, spec: &Path, missions: &Path) -> Command {
    let mut c = cmd(dir);
    c.args(["pulse", "--pulse-db"])
        .arg(dir.join("pulse.sqlite"))
        .args(["--premflow-bin"])
        .arg(dir.join("premflow"))
        .args(["--missions-dir"])
        .arg(missions)
        .args([
            "--now",
            "2026-10-27T12:00:00+01:00",
            "missions",
            "import",
            "--spec",
        ])
        .arg(spec)
        .env("HOME", dir.join("home"))
        .env("ENSEMBLY_PULSE_G_ROOTS", dir.join("g"))
        .env("ENSEMBLY_PULSE_NOTES", dir.join("notes"));
    c
}
