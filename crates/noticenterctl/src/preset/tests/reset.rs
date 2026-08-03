use std::fs;
use std::io::Cursor;

use super::super::reset::{confirm_reset, run_reset_config};
use crate::test_support::{test_env_lock, EnvGuard};

#[test]
fn confirmation_accepts_yes_and_defaults_to_no() {
    assert!(confirm_reset(&mut Cursor::new("yes\n"), true).expect("read yes"));
    assert!(!confirm_reset(&mut Cursor::new("\n"), true).expect("read default"));
}

#[test]
fn noninteractive_confirmation_fails_closed() {
    let error = confirm_reset(&mut Cursor::new("yes\n"), false).expect_err("require --yes");
    assert!(error.to_string().contains("--yes"));
}

#[test]
fn yes_mode_executes_reset_and_creates_shared_settings() {
    let _lock = test_env_lock();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-cli-reset-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", &root);
    let config_dir = root.join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config fixture");
    fs::write(config_dir.join("config.toml"), "custom = true\n").expect("seed config");

    run_reset_config(true).expect("--yes reset should execute");

    assert!(config_dir.join("installer.toml").is_file());
    let config = fs::read_to_string(config_dir.join("config.toml")).expect("read reset config");
    toml::from_str::<unixnotis_core::Config>(&config).expect("reset config should parse");
    assert!(fs::read_dir(&config_dir)
        .expect("read reset directory")
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().starts_with("Backup-")));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn yes_mode_rejects_invalid_settings_without_changes() {
    let _lock = test_env_lock();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-cli-invalid-settings-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", &root);
    let config_dir = root.join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config fixture");
    fs::write(config_dir.join("config.toml"), "custom config\n").expect("seed config");
    fs::write(config_dir.join("installer.toml"), "[backups\n").expect("corrupt settings");
    let before = fs::read(config_dir.join("config.toml")).expect("read config before reset");

    let error = run_reset_config(true).expect_err("invalid settings must fail");

    assert!(error.to_string().contains("installer settings"));
    assert_eq!(
        fs::read(config_dir.join("config.toml")).expect("read config after reset"),
        before
    );
    assert_eq!(
        fs::read_dir(&config_dir)
            .expect("read reset directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("Backup-"))
            .count(),
        0,
        "invalid settings must not create a backup"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn yes_mode_rejects_non_file_settings_without_changes() {
    let _lock = test_env_lock();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-cli-directory-settings-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", &root);
    let config_dir = root.join("unixnotis");
    fs::create_dir_all(&config_dir).expect("create config fixture");
    fs::write(config_dir.join("config.toml"), "custom config\n").expect("seed config");
    fs::create_dir(config_dir.join("installer.toml")).expect("create settings directory");
    let before = fs::read(config_dir.join("config.toml")).expect("read config before reset");

    let error = run_reset_config(true).expect_err("directory settings must fail");

    assert!(error.to_string().contains("installer settings"));
    assert_eq!(
        fs::read(config_dir.join("config.toml")).expect("read config after reset"),
        before
    );
    assert_eq!(
        fs::read_dir(&config_dir)
            .expect("read reset directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("Backup-"))
            .count(),
        0,
        "non-file settings must not create a backup"
    );
    let _ = fs::remove_dir_all(root);
}
