use std::process::Command;

#[test]
fn clock_status_json_smoke() {
    let out = Command::new(env!("CARGO_BIN_EXE_ensembly"))
        .args(["--no-memory", "clock", "status", "--json"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json status");
    assert!(v.get("cache_path").is_some());
    assert!(v.get("chronyc").is_some());
}
