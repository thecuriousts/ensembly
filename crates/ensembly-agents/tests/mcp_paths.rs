use ensembly_agents::McpServeConfig;
use std::fs;

#[test]
fn mcp_memory_uses_config_data_dir_when_no_env() {
    let t = tempfile::tempdir().unwrap();
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
