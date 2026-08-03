//! Tests for bounded configuration loading and parsing

use std::fs;
use std::io::Cursor;

use crate::{Config, ConfigError, MAX_CONFIG_BYTES};

use super::super::load::read_config_contents;
use super::support::{env_lock, test_root, EnvGuard};

const EXPECTED_MAX_CONFIG_BYTES: usize = 1_048_576;

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
fn parse_returns_the_config_produced_by_the_report_pipeline() {
    let config = Config::parse(
        r#"
        [panel]
        title = "Parsed Title"
        "#,
    )
    .expect("valid config text should parse");

    assert_eq!(config.panel.title, "Parsed Title");
}

#[test]
fn legacy_theme_mode_is_ignored_and_default_rendering_omits_it() {
    let report = Config::parse_with_report(
        r#"
        [theme]
        mode = "stock"
        popup_css = "popup.css"
        "#,
    )
    .expect("legacy theme mode should be ignored");

    assert_eq!(report.config.theme.popup_css, "popup.css");
    assert!(!report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "config.unknown-key" && diagnostic.path.as_deref() == Some("theme.mode")
    }));
    let rendered = toml::to_string_pretty(&Config::default()).expect("default config renders");
    assert!(!rendered.contains("mode = \"stock\""));
    assert!(!rendered.contains("mode = \"custom\""));
}

#[test]
fn sound_file_hints_require_explicit_configuration() {
    let defaults = Config::parse("").expect("default config should parse");
    let enabled = Config::parse(
        r#"
        [sound]
        allow_file_hints = true
        allowed_file_hint_dirs = ["sounds", "/srv/notification-sounds"]
        "#,
    )
    .expect("sound hint policy should parse");

    assert!(!defaults.sound.allow_file_hints);
    assert!(defaults.sound.allowed_file_hint_dirs.is_empty());
    assert!(enabled.sound.allow_file_hints);
    assert_eq!(
        enabled.sound.allowed_file_hint_dirs,
        ["sounds", "/srv/notification-sounds"]
    );
}

#[test]
fn load_from_path_rejects_oversized_config_before_parsing() {
    assert_eq!(MAX_CONFIG_BYTES, EXPECTED_MAX_CONFIG_BYTES as u64);
    let root = test_root("load-oversized");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("config dir");
    let path = root.join("config.toml");
    let file = fs::File::create(&path).expect("oversized config file");
    // A sparse file exercises the metadata guard without allocating the payload in the test
    file.set_len(EXPECTED_MAX_CONFIG_BYTES as u64 + 1)
        .expect("oversized config length");

    let error = Config::load_from_path(&path).expect_err("oversized config should fail");

    assert!(matches!(
        error,
        ConfigError::TooLarge {
            size,
            max,
        } if size == EXPECTED_MAX_CONFIG_BYTES as u64 + 1
            && max == EXPECTED_MAX_CONFIG_BYTES as u64
    ));
    assert_eq!(
        error.shareable_summary(),
        "Configuration file exceeds the maximum supported size"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn bounded_reader_accepts_exact_limit_and_rejects_a_growing_stream() {
    assert_eq!(MAX_CONFIG_BYTES, EXPECTED_MAX_CONFIG_BYTES as u64);
    let declared_oversized = read_config_contents(
        Cursor::new(Vec::<u8>::new()),
        EXPECTED_MAX_CONFIG_BYTES as u64 + 1,
    )
    .expect_err("declared oversized input should fail before reading");

    assert!(matches!(
        declared_oversized,
        ConfigError::TooLarge { size, max }
            if size == EXPECTED_MAX_CONFIG_BYTES as u64 + 1
                && max == EXPECTED_MAX_CONFIG_BYTES as u64
    ));

    let exact = vec![b' '; EXPECTED_MAX_CONFIG_BYTES];

    let contents = read_config_contents(Cursor::new(exact), EXPECTED_MAX_CONFIG_BYTES as u64)
        .expect("a config at the exact size limit should be accepted");

    assert_eq!(contents.len(), EXPECTED_MAX_CONFIG_BYTES);

    let grew_after_metadata = vec![b' '; EXPECTED_MAX_CONFIG_BYTES + 1];
    let error = read_config_contents(Cursor::new(grew_after_metadata), 0)
        .expect_err("a stream that grows beyond the limit should be rejected");

    assert!(matches!(
        error,
        ConfigError::TooLarge { size, max }
            if size == EXPECTED_MAX_CONFIG_BYTES as u64 + 1
                && max == EXPECTED_MAX_CONFIG_BYTES as u64
    ));
}

#[test]
fn shareable_error_summaries_never_echo_private_error_details() {
    let error = ConfigError::ParseFailed("secret_command = 'private-parser-sentinel'".to_string());

    let summary = error.shareable_summary();

    assert_eq!(summary, "Configuration TOML or schema is invalid");
    assert!(!summary.contains("secret_command"));
    assert!(!summary.contains("private-parser-sentinel"));
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
