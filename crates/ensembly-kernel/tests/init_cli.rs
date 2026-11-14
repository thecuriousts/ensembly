//! `ensembly init`: writes the default config under XDG dirs, never overwrites without --force.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn init(home: &Path, xdg: bool, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ensembly"));
    cmd.arg("init")
        .args(args)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_DATA_HOME")
        .current_dir(home);
    if xdg {
        cmd.env("XDG_CONFIG_HOME", home.join("cfg"))
            .env("XDG_DATA_HOME", home.join("share"));
    }
    cmd.output().expect("run init")
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn writes_default_under_xdg_and_prints_path() {
    let t = tempfile::tempdir().unwrap();
    let o = init(t.path(), true, &[]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let path = t.path().join("cfg/ensembly/config.toml");
    assert_eq!(
        String::from_utf8_lossy(&o.stdout).trim(),
        path.display().to_string()
    );
    let body = fs::read_to_string(&path).unwrap();
    assert!(body.lines().any(|l| l == "schema = 1"), "{body}");
    assert!(
        !body.contains("opportunity_finder"),
        "init must not write a no-op pack toggle: {body}"
    );
    let data = t.path().join("share/ensembly");
    assert!(
        body.contains(&format!("data_dir = \"{}\"", data.display())),
        "{body}"
    );
    assert!(data.is_dir());
}

#[test]
fn falls_back_to_home_when_xdg_unset() {
    let t = tempfile::tempdir().unwrap();
    let o = init(t.path(), false, &[]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(t.path().join(".config/ensembly/config.toml").is_file());
    assert!(t.path().join(".local/share/ensembly").is_dir());
}

#[cfg(unix)]
#[test]
fn refuses_dangling_symlink_without_force() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("cfg/ensembly/config.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink("/nonexistent-ensembly-config", &path).unwrap();
    let o = init(t.path(), true, &[]);
    assert_eq!(o.status.code(), Some(7), "{}", text(&o));
    assert!(text(&o).contains("refused without --force"));
}

#[test]
fn refuses_overwrite_without_force() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(init(t.path(), true, &[]).status.code(), Some(0));
    let path = t.path().join("cfg/ensembly/config.toml");
    fs::write(&path, "# mine\n").unwrap();
    let o = init(t.path(), true, &[]);
    assert_eq!(o.status.code(), Some(7), "{}", text(&o));
    assert!(text(&o).contains("refused without --force"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "# mine\n");
}

#[test]
fn force_rewrites_default() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(init(t.path(), true, &[]).status.code(), Some(0));
    let path = t.path().join("cfg/ensembly/config.toml");
    fs::write(&path, "# mine\n").unwrap();
    let o = init(t.path(), true, &["--force"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(fs::read_to_string(&path).unwrap().contains("schema = 1"));
}

#[test]
fn matches_checked_in_example_keys() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(init(t.path(), true, &[]).status.code(), Some(0));
    let written = fs::read_to_string(t.path().join("cfg/ensembly/config.toml")).unwrap();
    let example = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../config.example.toml"
    ))
    .unwrap();
    assert!(written.contains("[paths]"), "{written}");
    assert!(written.contains("data_dir = "), "{written}");
    assert!(example.contains("[paths]"), "{example}");
    assert!(!example.contains("opportunity_finder ="), "{example}");
}
