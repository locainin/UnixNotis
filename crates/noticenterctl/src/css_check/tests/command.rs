use std::fs;
use std::path::PathBuf;

use unixnotis_core::CURRENT_CONFIG_VERSION;

use super::load_config_for_path;

#[test]
fn explicit_existing_config_is_loaded_instead_of_the_default() {
    let root = temporary_test_directory("explicit");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        format!("config_version = {CURRENT_CONFIG_VERSION}\n[panel]\nwidth = 517\n"),
    )
    .expect("write command config");

    let config = load_config_for_path(&config_path).expect("load explicit command config");

    assert_eq!(config.panel.width, 517);
    fs::remove_dir_all(root).expect("remove command config directory");
}

#[test]
fn malformed_existing_config_is_rejected_without_default_fallback() {
    let root = temporary_test_directory("malformed");
    let config_path = root.join("config.toml");
    fs::write(&config_path, "[panel\nwidth = 517").expect("write malformed command config");

    let error = load_config_for_path(&config_path).expect_err("reject malformed explicit config");

    assert!(error.to_string().contains("load active config"));
    fs::remove_dir_all(root).expect("remove malformed command config directory");
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
