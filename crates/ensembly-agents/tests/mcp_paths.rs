use ensembly_agents::McpServeConfig;
use serial_test::serial;
use std::fs;

#[serial]
#[test]
fn mcp_memory_uses_config_data_dir_when_no_env() {
    let t = tempfile::tempdir().unwrap();
    std::env::set_current_dir(t.path()).unwrap();
    let home = t.path().join("home");
    let cfg = home.join("cfg/ensembly");
    let data = home.join("share/ensembly");
    fs::create_dir_all(&cfg).unwrap();
    fs::create_dir_all(&data).unwrap();
    fs::write(
        cfg.join("config.toml"),
        format!("schema = 1\n\n[paths]\ndata_dir = \"{}\"\n", data.display()),
    )
    .unwrap();

    std::env::set_var("HOME", &home);
    std::env::set_var("XDG_CONFIG_HOME", home.join("cfg"));
    std::env::remove_var("ENSEMBLY_MEMORY");
    std::env::remove_var("PERAM_MEMORY");

    let cfg = McpServeConfig::from_env(None).unwrap();
    assert_eq!(cfg.memory_path, data.join("ensembly-memory.json"));
}

#[serial]
#[test]
fn mcp_explicit_memory_skips_malformed_config() {
    let t = tempfile::tempdir().unwrap();
    std::env::set_current_dir(t.path()).unwrap();
    let home = t.path().join("home");
    let cfg = home.join("cfg/ensembly");
    fs::create_dir_all(&cfg).unwrap();
    fs::write(cfg.join("config.toml"), "schema = 1\n[paths\n").unwrap();

    let mem = t.path().join("data/local/ensembly-memory.json");
    fs::create_dir_all(mem.parent().unwrap()).unwrap();

    std::env::set_var("HOME", &home);
    std::env::set_var("XDG_CONFIG_HOME", home.join("cfg"));
    std::env::set_var("ENSEMBLY_MEMORY", mem.display().to_string());

    let got = McpServeConfig::from_env(None).unwrap();
    assert_eq!(got.memory_path, mem);
}
