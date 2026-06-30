use std::env;
use std::path::PathBuf;

use crate::Config;

use super::support::{env_lock, restore_env, set_env, test_root};

#[test]
fn default_config_dir_ignores_empty_xdg() {
    let _guard = env_lock();
    let home = env::var("HOME").unwrap_or_default();
    if home.trim().is_empty() {
        // Some CI shells can run without HOME; path fallback cannot be asserted there
        return;
    }
    let prev_xdg = set_env("XDG_CONFIG_HOME", Some(""));
    let prev_home = set_env("HOME", Some(&home));

    let dir = Config::default_config_dir().expect("config dir");

    assert_eq!(dir, PathBuf::from(home).join(".config").join("unixnotis"));
    restore_env("XDG_CONFIG_HOME", prev_xdg);
    restore_env("HOME", prev_home);
}

#[test]
fn default_config_dir_ignores_whitespace_xdg() {
    let _guard = env_lock();
    let home = env::var("HOME").unwrap_or_default();
    if home.trim().is_empty() {
        // Empty HOME makes the fallback path intentionally unavailable
        return;
    }
    // Whitespace-only XDG values should behave the same as a missing value
    let prev_xdg = set_env("XDG_CONFIG_HOME", Some("   "));
    let prev_home = set_env("HOME", Some(&home));

    let dir = Config::default_config_dir().expect("config dir");

    assert_eq!(dir, PathBuf::from(home).join(".config").join("unixnotis"));
    restore_env("XDG_CONFIG_HOME", prev_xdg);
    restore_env("HOME", prev_home);
}

#[test]
fn default_config_dir_ignores_relative_xdg() {
    let _guard = env_lock();
    let home = env::var("HOME").unwrap_or_default();
    if home.trim().is_empty() {
        // Relative fallback checks still need an absolute home directory
        return;
    }
    // Relative XDG roots are unsafe because callers may run from arbitrary directories
    let prev_xdg = set_env("XDG_CONFIG_HOME", Some("relative/path"));
    let prev_home = set_env("HOME", Some(&home));

    let dir = Config::default_config_dir().expect("config dir");

    assert_eq!(dir, PathBuf::from(home).join(".config").join("unixnotis"));
    restore_env("XDG_CONFIG_HOME", prev_xdg);
    restore_env("HOME", prev_home);
}

#[test]
fn default_config_dir_accepts_absolute_xdg() {
    let _guard = env_lock();
    let home = env::var("HOME").unwrap_or_default();
    if home.trim().is_empty() {
        // Absolute XDG test still needs a stable base path
        return;
    }
    let xdg = PathBuf::from(home.clone()).join(".config-test");
    let prev_xdg = set_env("XDG_CONFIG_HOME", Some(xdg.to_string_lossy().as_ref()));
    let prev_home = set_env("HOME", Some(&home));

    let dir = Config::default_config_dir().expect("config dir");

    assert_eq!(dir, xdg.join("unixnotis"));
    restore_env("XDG_CONFIG_HOME", prev_xdg);
    restore_env("HOME", prev_home);
}

#[test]
fn default_config_path_joins_config_file_name() {
    let _guard = env_lock();
    let root = test_root("default-config-path");
    // Use a clean fake XDG root so the exact final filename can be asserted
    let _ = std::fs::remove_dir_all(&root);

    let prev_xdg = set_env("XDG_CONFIG_HOME", Some(root.to_string_lossy().as_ref()));
    let prev_home = set_env("HOME", Some(root.to_string_lossy().as_ref()));
    let path = Config::default_config_path().expect("default config path");

    assert_eq!(path, root.join("unixnotis").join("config.toml"));

    restore_env("XDG_CONFIG_HOME", prev_xdg);
    restore_env("HOME", prev_home);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn config_dir_for_path_uses_current_dir_for_bare_file_name() {
    let dir = Config::config_dir_for_path(std::path::Path::new("config.toml")).expect("config dir");

    // Bare filenames are treated as relative to the process working directory
    assert_eq!(dir, env::current_dir().expect("current dir"));
}

#[test]
fn config_dir_for_path_uses_parent_for_nested_path() {
    let dir = Config::config_dir_for_path(std::path::Path::new("nested/config.toml"))
        .expect("config dir");

    // Nested config paths should use their explicit parent instead of cwd
    assert_eq!(dir, PathBuf::from("nested"));
}

#[test]
fn resolve_theme_paths_from_includes_media_css() {
    let config: Config =
        toml::from_str("[theme]\nmedia_css = \"rice/media.css\"\n").expect("config should parse");
    let base = PathBuf::from("/tmp/unixnotis-theme-paths");

    // Theme path resolution needs to include every active CSS slot, including media
    let paths = config
        .resolve_theme_paths_from(&base)
        .expect("theme paths should resolve");

    assert_eq!(paths.media_css, base.join("rice").join("media.css"));
}
