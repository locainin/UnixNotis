use std::fs;
use std::path::PathBuf;

use unixnotis_core::CURRENT_CONFIG_VERSION;

use super::load_config_for_path;
use crate::config_path::ConfigPathSource;
use crate::test_support::{test_env_lock, EnvGuard};

#[test]
fn explicit_existing_config_is_loaded_instead_of_the_default() {
    let root = temporary_test_directory("explicit");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        format!("config_version = {CURRENT_CONFIG_VERSION}\n[panel]\nwidth = 517\n"),
    )
    .expect("write command config");

    let config = load_config_for_path(&config_path, ConfigPathSource::Cli)
        .expect("load explicit command config");

    assert_eq!(config.panel.width, 517);
    fs::remove_dir_all(root).expect("remove command config directory");
}

#[test]
fn malformed_existing_config_is_rejected_without_default_fallback() {
    let root = temporary_test_directory("malformed");
    let config_path = root.join("config.toml");
    fs::write(&config_path, "[panel\nwidth = 517").expect("write malformed command config");

    let error = load_config_for_path(&config_path, ConfigPathSource::Cli)
        .expect_err("reject malformed explicit config");

    assert!(error.to_string().contains("load active config"));
    fs::remove_dir_all(root).expect("remove malformed command config directory");
}

#[test]
fn explicit_missing_config_is_rejected() {
    let root = temporary_test_directory("missing-cli");
    let config_path = root.join("missing.toml");

    let error = load_config_for_path(&config_path, ConfigPathSource::Cli)
        .expect_err("reject missing CLI config");

    assert!(error
        .to_string()
        .contains("explicit configuration file does not exist"));
    fs::remove_dir_all(root).expect("remove missing CLI config directory");
}

#[test]
fn missing_environment_config_is_rejected() {
    let root = temporary_test_directory("missing-environment");
    let config_path = root.join("missing.toml");

    let error = load_config_for_path(&config_path, ConfigPathSource::Environment)
        .expect_err("reject missing environment config");

    assert!(error
        .to_string()
        .contains("explicit configuration file does not exist"));
    fs::remove_dir_all(root).expect("remove missing environment config directory");
}

#[test]
fn absent_default_config_uses_builtin_defaults() {
    let _lock = test_env_lock();
    let root = temporary_test_directory("missing-default");
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", &root);
    let config_path = root.join("missing.toml");

    let config = load_config_for_path(&config_path, ConfigPathSource::Default)
        .expect("load built-in defaults for absent default path");

    assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
    fs::remove_dir_all(root).expect("remove absent default config directory");
}

fn temporary_test_directory(case: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "unixnotis-css-check-command-{}-{case}",
        std::process::id()
    ));

    // A prior interrupted test must not affect the current fixture
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create command test directory");
    root
}
