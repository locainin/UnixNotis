use super::Config;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock")
}

fn set_env(key: &str, value: Option<&str>) -> Option<String> {
    let previous = env::var(key).ok();
    match value {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
    previous
}

fn restore_env(key: &str, previous: Option<String>) {
    match previous {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
}

fn test_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::current_dir()
        .expect("current dir")
        .join("target")
        .join(format!("unixnotis-{name}-{}-{unique}", std::process::id()))
}

#[test]
fn default_config_dir_ignores_empty_xdg() {
    let _guard = env_lock();
    let home = env::var("HOME").unwrap_or_default();
    if home.trim().is_empty() {
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
        return;
    }
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
        return;
    }
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
fn config_dir_for_path_uses_current_dir_for_bare_file_name() {
    let dir = Config::config_dir_for_path(std::path::Path::new("config.toml")).expect("config dir");
    assert_eq!(dir, env::current_dir().expect("current dir"));
}

#[test]
fn config_dir_for_path_uses_parent_for_nested_path() {
    let dir = Config::config_dir_for_path(std::path::Path::new("nested/config.toml"))
        .expect("config dir");
    assert_eq!(dir, PathBuf::from("nested"));
}

#[test]
fn resolve_theme_paths_from_includes_media_css() {
    let config: Config =
        toml::from_str("[theme]\nmedia_css = \"rice/media.css\"\n").expect("config should parse");
    let base = PathBuf::from("/tmp/unixnotis-theme-paths");
    let paths = config
        .resolve_theme_paths_from(&base)
        .expect("theme paths should resolve");

    assert_eq!(paths.media_css, base.join("rice").join("media.css"));
}

#[test]
fn ensure_default_scripts_in_creates_every_shipped_script() {
    let root = test_root("default-scripts");
    let _ = fs::remove_dir_all(&root);

    Config::ensure_default_scripts_in(&root).expect("default scripts");

    for script in crate::DEFAULT_SCRIPTS {
        let path = root.join(script.relative_path);
        let contents = fs::read_to_string(&path).expect("read default script");
        assert_eq!(contents, script.contents);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(&path)
                .expect("script metadata")
                .permissions()
                .mode();
            assert_ne!(mode & 0o111, 0, "script should be executable: {path:?}");
        }
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ensure_default_scripts_in_preserves_user_edited_script_contents() {
    let root = test_root("default-script-preserve");
    let _ = fs::remove_dir_all(&root);
    let script = crate::DEFAULT_SCRIPTS
        .iter()
        .find(|script| script.relative_path.ends_with("unixnotis-blue-light-on"))
        .expect("blue light on script");
    let path = root.join(script.relative_path);
    fs::create_dir_all(path.parent().expect("script parent")).expect("script parent dir");
    fs::write(&path, "#!/bin/sh\nexit 42\n").expect("custom script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("clear exec bit");
    }

    Config::ensure_default_scripts_in(&root).expect("default scripts");

    assert_eq!(
        fs::read_to_string(&path).expect("read custom script"),
        "#!/bin/sh\nexit 42\n"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(&path)
            .expect("script metadata")
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "custom script should be executable");
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn enabled_default_script_commands_have_shipped_files() {
    let shipped = crate::DEFAULT_SCRIPTS
        .iter()
        .map(|script| script.relative_path)
        .collect::<Vec<_>>();
    let config = Config::default();

    for toggle in config
        .widgets
        .toggles
        .iter()
        .filter(|toggle| toggle.enabled)
    {
        for command in [
            toggle.state_cmd.as_deref(),
            toggle.toggle_cmd.as_deref(),
            toggle.on_cmd.as_deref(),
            toggle.off_cmd.as_deref(),
            toggle.watch_cmd.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if command.starts_with("scripts/") {
                assert!(
                    shipped.contains(&command),
                    "default command must be shipped: {command}"
                );
            }
        }
    }
}
