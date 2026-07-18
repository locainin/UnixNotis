use std::env;
use std::path::PathBuf;

use crate::Config;

use super::support::{env_lock, test_root, EnvGuard};

#[test]
fn default_config_dir_ignores_empty_xdg() {
    let _guard = env_lock();
    let home = env::var("HOME").unwrap_or_default();
    if home.trim().is_empty() {
        // Some CI shells can run without HOME; path fallback cannot be asserted there
        return;
    }
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", "");
    let _home = EnvGuard::set("HOME", home.as_str());

    let dir = Config::default_config_dir().expect("config dir");

    assert_eq!(dir, PathBuf::from(home).join(".config").join("unixnotis"));
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
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", "   ");
    let _home = EnvGuard::set("HOME", home.as_str());

    let dir = Config::default_config_dir().expect("config dir");

    assert_eq!(dir, PathBuf::from(home).join(".config").join("unixnotis"));
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
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", "relative/path");
    let _home = EnvGuard::set("HOME", home.as_str());

    let dir = Config::default_config_dir().expect("config dir");

    assert_eq!(dir, PathBuf::from(home).join(".config").join("unixnotis"));
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
    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg.as_os_str());
    let _home = EnvGuard::set("HOME", home.as_str());

    let dir = Config::default_config_dir().expect("config dir");

    assert_eq!(dir, xdg.join("unixnotis"));
}

#[test]
fn default_config_path_joins_config_file_name() {
    let _guard = env_lock();
    let root = test_root("default-config-path");
    // Use a clean fake XDG root so the exact final filename can be asserted
    let _ = std::fs::remove_dir_all(&root);

    let _xdg = EnvGuard::set("XDG_CONFIG_HOME", root.as_os_str());
    let _home = EnvGuard::set("HOME", root.as_os_str());
    let path = Config::default_config_path().expect("default config path");

    assert_eq!(path, root.join("unixnotis").join("config.toml"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn active_config_path_prefers_nonempty_environment_override() {
    let _guard = env_lock();
    let selected = test_root("active-config-path").join("chosen.toml");
    let _config = EnvGuard::set("UNIXNOTIS_CONFIG_PATH", selected.as_os_str());

    assert_eq!(
        Config::active_config_path().expect("active config path"),
        selected
    );
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
fn resolve_theme_paths_from_includes_media_layer() {
    let config: Config =
        toml::from_str("[theme]\nmedia_css = \"rice/media.css\"\n").expect("config should parse");
    let base = PathBuf::from("/tmp/unixnotis-theme-paths");

    // Theme path resolution needs to include every active CSS slot, including media
    let paths = config
        .resolve_theme_paths_from(&base)
        .expect("theme paths should resolve");

    assert_eq!(paths.media_css, base.join("rice").join("media.css"));
}
