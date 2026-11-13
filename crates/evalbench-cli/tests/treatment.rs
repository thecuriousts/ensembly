//! E5 treatment run (pulse on vs off) on the frozen E1 tasks. Offline, synthetic replies.
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use evalbench_cli::load_task_set;
use evalbench_core::runner::{render_prompt, render_prompt_with, PulseContext, PULSE_HEADER};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn tasks() -> PathBuf {
    root().join("crates/evalbench-core/fixtures/tasks")
}
fn pulse() -> PathBuf {
    root().join("crates/evalbench-core/fixtures/pulse/e5-pulse.json")
}
fn base() -> PathBuf {
    root().join("crates/evalbench-cli/treatments/e5-pulse")
}
fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}
fn evalbench(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_evalbench"))
        .args(args)
        .output()
        .unwrap()
}
fn python(script: &str, args: &[&OsStr]) -> Output {
    Command::new("python3")
        .arg("-I")
        .arg(root().join(script))
        .args(args)
        .output()
        .unwrap()
}
fn os(s: &str) -> &OsStr {
    OsStr::new(s)
}
const RUNS: [(&str, bool); 6] = [
    ("off-r1", false),
    ("off-r2", false),
    ("off-r3", false),
    ("on-r1", true),
    ("on-r2", true),
    ("on-r3", true),
];

#[test]
fn the_only_change_is_the_pulse_pack_in_context() {
    let set = load_task_set(&tasks()).unwrap();
    let p = PulseContext {
        text: read(&pulse()),
    };
    assert_eq!(set.tasks.len(), 12);
    for t in &set.tasks {
        let off = render_prompt(t);
        assert_eq!(render_prompt_with(t, None), off);
        assert_eq!(
            render_prompt_with(t, Some(&p)),
            format!("{off}{PULSE_HEADER}{}", p.text)
        );
    }
}

#[test]
fn pulse_fixture_is_a_canonical_pack_that_checks_ok() {
    let c = python(
        "scripts/pulse/pulse_pack_check.py",
        &[os("canon"), pulse().as_os_str()],
    );
    assert_eq!(c.status.code(), Some(0), "{c:?}");
    assert_eq!(
        c.stdout,
        fs::read(pulse()).unwrap(),
        "fixture is not canonical bytes"
    );
    let k = python(
        "scripts/pulse/pulse_pack_check.py",
        &[os("check"), pulse().as_os_str()],
    );
    assert_eq!(k.status.code(), Some(0), "{k:?}");
}

#[test]
fn treatment_runs_replay_and_rescore_identically() {
    let hash = read(&tasks().join("../tasks.sha256")).trim().to_string();
    let e1: serde_json::Value = serde_json::from_str(&read(
        &root().join("crates/evalbench-cli/baselines/replay-001/score.json"),
    ))
    .unwrap();
    for (run, on) in RUNS {
        let dir = base().join(run);
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("traces.jsonl");
        let run_id = format!("e5-{run}");
        let task_dir = tasks();
        let mut args: Vec<&OsStr> = vec![
            os("run"),
            os("--tasks"),
            task_dir.as_os_str(),
            os("--run-id"),
            os(&run_id),
            os("--out"),
            out.as_os_str(),
            os("--replay"),
        ];
        let replies = dir.join("replies.jsonl");
        args.push(replies.as_os_str());
        let pp = pulse();
        if on {
            args.push(os("--pulse-pack"));
            args.push(pp.as_os_str());
        }
        let r = evalbench(&args);
        assert_eq!(r.status.code(), Some(0), "{r:?}");
        assert_eq!(read(&out), read(&dir.join("traces.jsonl")), "{run}");
        let want_prompt = if on { "gaps-v1+pulse" } else { "gaps-v1" };
        for line in read(&out).lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(v["prompt_id"], want_prompt, "{run}");
            assert_eq!(v["task_set_hash"], hash.as_str(), "{run}");
        }
        let s = evalbench(&[
            os("score"),
            os("--tasks"),
            tasks().as_os_str(),
            out.as_os_str(),
        ]);
        assert_eq!(s.status.code(), Some(0), "{s:?}");
        assert_eq!(
            String::from_utf8(s.stdout).unwrap(),
            read(&dir.join("score.json"))
        );
        let v: serde_json::Value = serde_json::from_str(&read(&dir.join("score.json"))).unwrap();
        assert_eq!(v["model_ids"], e1["model_ids"], "same model ids as E1");
        assert_eq!(v["tasks"], 12);
    }
}

#[test]
fn synthetic_replies_match_their_generator() {
    let o = python("scripts/e5/synthetic_replies.py", &[os("--check")]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
}

#[test]
fn report_table_and_summary_regenerate() {
    let o = python(
        "scripts/e5/report.py",
        &[base().as_os_str(), os("any"), os("--check")],
    );
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    let s: serde_json::Value = serde_json::from_str(&read(&base().join("summary.json"))).unwrap();
    assert_eq!(s["reruns"], 3);
    assert_eq!(s["synthetic"], true);
    let report = read(&base().join("REPORT.md"));
    for must in [
        "**Synthetic.**",
        "**Not a CV number.**",
        "n = 12 tasks per run, 3 reruns per arm",
    ] {
        assert!(report.contains(must), "REPORT.md lacks {must:?}");
    }
}

#[test]
fn chart_rebuilds_offline_with_bundled_fonts_and_the_minicharts_notice() {
    let o = python("scripts/e5/build_chart.py", &[os("--check")]);
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    let chart = base().join("chart");
    let page = read(&chart.join("index.html"));
    for bad in [
        "fonts.googleapis.com",
        "fonts.gstatic.com",
        "src=\"http",
        "url(http",
        "@import",
    ] {
        assert!(!page.contains(bad), "page holds {bad:?}");
    }
    assert!(page.contains("href=\"fonts/fonts.css\""));
    let css = read(&chart.join("fonts/fonts.css"));
    let mut fonts = 0;
    for part in css.split("url(\"").skip(1) {
        let name = part.split('"').next().unwrap();
        assert!(
            !name.contains(':') && !name.contains('/'),
            "non-local font {name}"
        );
        assert!(
            chart.join("fonts").join(name).is_file(),
            "missing font {name}"
        );
        fonts += 1;
    }
    assert_eq!(fonts, 6);
    for ofl in ["OFL-YoungSerif.txt", "OFL-Gaegu.txt", "OFL-DMSans.txt"] {
        assert!(read(&chart.join("fonts").join(ofl)).contains("SIL OPEN FONT LICENSE"));
    }
    let v = chart.join("vendor/minicharts");
    let license = read(&v.join("LICENSE"));
    assert!(license.starts_with("MIT License"));
    assert!(license.contains("Copyright (c) 2026 April Zhu"));
    assert!(read(&v.join("NOTICE.md")).contains("Minicharts is April Zhu's own work"));
    assert!(page.contains("Minicharts is used under the MIT licence"));
}

#[test]
fn chart_license_guard_is_clean_and_catches_a_hit() {
    let o = python("scripts/chart-license-guard.py", &[]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );

    let tmp = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let s = Command::new("git")
            .args(args)
            .current_dir(tmp.path())
            .output()
            .unwrap();
        assert!(s.status.success(), "{s:?}");
    };
    git(&["init", "-q"]);
    fs::write(tmp.path().join("ok.md"), "drawn with Minicharts\n").unwrap();
    let r = python(
        "scripts/chart-license-guard.py",
        &[os("--root"), tmp.path().as_os_str()],
    );
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let banned = ["tiny", "charts"].concat();
    fs::write(tmp.path().join("page.html"), format!("<p>{banned}</p>\n")).unwrap();
    let r = python(
        "scripts/chart-license-guard.py",
        &[os("--root"), tmp.path().as_os_str()],
    );
    assert_eq!(r.status.code(), Some(1), "{r:?}");
    fs::remove_file(tmp.path().join("page.html")).unwrap();
    fs::write(tmp.path().join(format!("{banned}-sheet.svg")), "<svg/>\n").unwrap();
    let r = python(
        "scripts/chart-license-guard.py",
        &[os("--root"), tmp.path().as_os_str()],
    );
    assert_eq!(r.status.code(), Some(1), "{r:?}");
}
