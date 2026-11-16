//! The `peram` compat alias still runs and names itself `peram`.

use std::process::Command;

#[test]
fn peram_alias_runs_version() {
    let o = Command::new(env!("CARGO_BIN_EXE_peram"))
        .arg("version")
        .output()
        .unwrap();
    assert!(o.status.success(), "{o:?}");
    assert!(String::from_utf8_lossy(&o.stdout).contains("ensembly-kernel"));

    let help = Command::new(env!("CARGO_BIN_EXE_peram"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage: peram"));
}
