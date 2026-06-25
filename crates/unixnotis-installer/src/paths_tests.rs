use super::{format_with_home, is_unixnotis_repo, InstallPaths};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

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

#[test]
fn format_with_home_rewrites_prefix() {
    // Confirms home-prefixed paths are rendered with the $HOME shorthand.
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let path = PathBuf::from(&home).join(".config").join("unixnotis");
    let rendered = format_with_home(&path);
    assert!(rendered.starts_with("$HOME"));
}

#[test]
fn is_unixnotis_repo_detects_markers() {
    // Validates that known workspace markers are detected in a Cargo.toml file.
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let dir = PathBuf::from(home)
        .join(".cache")
        .join(format!("unixnotis-test-{}", std::process::id()));
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let cargo_path = dir.join("Cargo.toml");
    let contents = r#"
[package]
name = "unixnotis-daemon"

[workspace]
members = ["crates/unixnotis-daemon", "crates/unixnotis-core"]
"#;
    if fs::write(&cargo_path, contents).is_err() {
        let _ = fs::remove_dir_all(&dir);
        return;
    }

    assert!(is_unixnotis_repo(&cargo_path));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn install_paths_use_xdg_config_home_for_systemd_units() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let xdg_root = PathBuf::from(&home).join(".config-xdg-test");
    let previous = set_env("XDG_CONFIG_HOME", Some(xdg_root.to_string_lossy().as_ref()));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("systemd"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    assert_eq!(
        paths.service.artifact_root(),
        xdg_root.join("systemd").join("user")
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("XDG_CONFIG_HOME", previous);
}

#[test]
fn install_paths_use_xdg_config_home_for_dinit_services() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let xdg_root = PathBuf::from(&home).join(".config-xdg-dinit-test");
    let previous = set_env("XDG_CONFIG_HOME", Some(xdg_root.to_string_lossy().as_ref()));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("dinit"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    assert_eq!(paths.service.artifact_root(), xdg_root.join("dinit.d"));
    assert_eq!(
        paths.service.primary_artifact_path(),
        xdg_root.join("dinit.d").join("unixnotis-daemon")
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("XDG_CONFIG_HOME", previous);
}

#[test]
fn install_paths_use_home_config_for_dinit_services_when_xdg_unset() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let previous = set_env("XDG_CONFIG_HOME", None);
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("dinit"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    assert_eq!(
        paths.service.artifact_root(),
        PathBuf::from(&home).join(".config").join("dinit.d")
    );
    assert_eq!(
        paths.service.primary_artifact_path(),
        PathBuf::from(&home)
            .join(".config")
            .join("dinit.d")
            .join("unixnotis-daemon")
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("XDG_CONFIG_HOME", previous);
}

#[test]
fn install_paths_use_xdg_config_home_for_runit_services() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let xdg_root = PathBuf::from(&home).join(".config-xdg-runit-test");
    let previous = set_env("XDG_CONFIG_HOME", Some(xdg_root.to_string_lossy().as_ref()));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("runit"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    assert_eq!(paths.service.artifact_root(), xdg_root.join("service"));
    assert_eq!(
        paths.service.primary_artifact_path(),
        xdg_root.join("service").join("unixnotis-daemon")
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("XDG_CONFIG_HOME", previous);
}

#[test]
fn install_paths_use_home_config_for_runit_services_when_xdg_unset() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let previous = set_env("XDG_CONFIG_HOME", None);
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("runit"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    assert_eq!(
        paths.service.artifact_root(),
        PathBuf::from(&home).join(".config").join("service")
    );
    assert_eq!(
        paths.service.primary_artifact_path(),
        PathBuf::from(&home)
            .join(".config")
            .join("service")
            .join("unixnotis-daemon")
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("XDG_CONFIG_HOME", previous);
}
