//! Explicit theme-source persistence tests

use std::fs;
use std::os::unix::fs::symlink;

use crate::{persist_theme_mode, Config, ThemeMode};

use super::support::test_root;

#[test]
fn persist_theme_mode_updates_only_the_existing_theme_mode() {
    let root = test_root("theme-mode-update");
    fs::create_dir_all(&root).expect("test directory should be created");
    let path = root.join("config.toml");
    let original = "# retained heading\n[theme]\n# retained mode comment\nmode = \"custom\"\nbase_css = \"personal.css\"\n\n[panel]\nwidth = 418\n";
    fs::write(&path, original).expect("test config should be written");

    persist_theme_mode(&path, ThemeMode::Stock).expect("theme mode should be persisted");

    let contents = fs::read_to_string(&path).expect("updated config should be readable");
    assert!(contents.contains("# retained heading"));
    assert!(contents.contains("# retained mode comment"));
    assert!(contents.contains("base_css = \"personal.css\""));
    assert!(contents.contains("width = 418"));
    assert_eq!(
        Config::load_from_path(&path)
            .expect("updated config should remain valid")
            .theme
            .mode,
        ThemeMode::Stock
    );
    fs::remove_dir_all(root).expect("test directory should be removable");
}

#[test]
fn persist_theme_mode_creates_a_theme_table_when_it_is_absent() {
    let root = test_root("theme-mode-create");
    fs::create_dir_all(&root).expect("test directory should be created");
    let path = root.join("config.toml");
    fs::write(&path, "[panel]\nwidth = 419\n").expect("test config should be written");

    persist_theme_mode(&path, ThemeMode::Custom).expect("theme table should be created");

    let config = Config::load_from_path(&path).expect("updated config should remain valid");
    assert_eq!(config.theme.mode, ThemeMode::Custom);
    assert_eq!(config.panel.width, 419);
    fs::remove_dir_all(root).expect("test directory should be removable");
}

#[test]
fn persist_theme_mode_rejects_invalid_config_without_replacing_it() {
    let root = test_root("theme-mode-invalid");
    fs::create_dir_all(&root).expect("test directory should be created");
    let path = root.join("config.toml");
    let invalid = b"[theme\nmode = \"custom\"\n";
    fs::write(&path, invalid).expect("invalid test config should be written");

    persist_theme_mode(&path, ThemeMode::Stock).expect_err("invalid config must not be replaced");

    assert_eq!(
        fs::read(&path).expect("invalid config should remain readable"),
        invalid
    );
    fs::remove_dir_all(root).expect("test directory should be removable");
}

#[test]
fn persist_theme_mode_rejects_a_symlink_without_touching_its_target() {
    let root = test_root("theme-mode-symlink");
    fs::create_dir_all(&root).expect("test directory should be created");
    let outside = root.join("outside.toml");
    let path = root.join("config.toml");
    let original = b"[theme]\nmode = \"custom\"\n";
    fs::write(&outside, original).expect("outside config should be written");
    symlink(&outside, &path).expect("config symlink should be created");

    persist_theme_mode(&path, ThemeMode::Stock)
        .expect_err("theme mode persistence must reject symbolic links");

    assert_eq!(
        fs::read(&outside).expect("outside config should remain readable"),
        original
    );
    fs::remove_dir_all(root).expect("test directory should be removable");
}
