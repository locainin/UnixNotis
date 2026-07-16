use std::fs;

use crate::{Config, ConfigError};

use super::support::{env_lock, test_root, EnvGuard};

#[test]
fn load_from_path_reads_toml_and_applies_runtime_defaults() {
    let root = test_root("load-from-path");
    // Start from a clean root so the test only sees the TOML written below
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("config dir");
    let path = root.join("config.toml");
    // Deliberately use too-small refresh intervals to prove load sanitization still runs
    fs::write(
        &path,
        r#"
        [panel]
        title = "Loaded Title"

        [widgets]
        refresh_interval_ms = 1
        refresh_interval_slow_ms = 50
        "#,
    )
    .expect("config file");

    let config = Config::load_from_path(&path).expect("config should load");

    // User text should survive loading while runtime defaults repair unsafe timing values
    assert_eq!(config.panel.title, "Loaded Title");
    assert_eq!(config.widgets.refresh_interval_ms, 100);
    assert_eq!(config.widgets.refresh_interval_slow_ms, 100);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_from_path_returns_parse_error_for_invalid_toml() {
    let root = test_root("load-invalid");
    // Parse failures should come from the target file, not from leftover temp data
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("config dir");
    let path = root.join("config.toml");
    fs::write(&path, "[panel\n").expect("invalid config");

    let err = Config::load_from_path(&path).expect_err("invalid toml should fail");

    assert!(matches!(err, ConfigError::ParseFailed(_)));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn shareable_error_summaries_never_echo_private_error_details() {
    let error = ConfigError::ParseFailed("secret_command = '/home/private/tool'".to_string());

    let summary = error.shareable_summary();

    assert_eq!(summary, "Configuration TOML or schema is invalid");
    assert!(!summary.contains("secret_command"));
    assert!(!summary.contains("/home/private"));
}

#[test]
fn load_default_reads_config_when_default_file_exists() {
    let _guard = env_lock();
    let root = test_root("load-default-existing");
    // Default-path discovery reads process-global env, so this test owns the env lock
    let _ = fs::remove_dir_all(&root);
    let config_dir = root.join("unixnotis");
    fs::create_dir_all(&config_dir).expect("config dir");
    fs::write(
        config_dir.join("config.toml"),
        r#"
        [panel]
        title = "Default Path Title"
        "#,
    )
    .expect("default config file");

    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", root.as_os_str());
    let _home = EnvGuard::set("HOME", root.as_os_str());
    // This exercises the public default loader instead of the explicit-path helper
    let config = Config::load_default().expect("default config should load");

    assert_eq!(config.panel.title, "Default Path Title");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_default_returns_sanitized_stock_config_when_file_is_missing() {
    let _guard = env_lock();
    let root = test_root("load-default-missing");
    // Missing config should not require creating the config directory first
    let _ = fs::remove_dir_all(&root);

    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", root.as_os_str());
    let _home = EnvGuard::set("HOME", root.as_os_str());
    let config = Config::load_default().expect("missing config should fall back");

    // Fallback config still passes through the runtime sanitizer
    assert_eq!(config.panel.title, crate::PanelConfig::default().title);
    assert_eq!(config.widgets.refresh_interval_ms, 1000);

    let _ = fs::remove_dir_all(root);
}
