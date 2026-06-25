use super::{format_with_home, is_unixnotis_repo, InstallPaths, ServiceManagerChoice};
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
fn explicit_service_manager_choice_overrides_environment() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let xdg_root = PathBuf::from(&home).join(".config-explicit-service-manager-test");
    let previous_xdg = set_env("XDG_CONFIG_HOME", Some(xdg_root.to_string_lossy().as_ref()));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("systemd"));

    let paths = InstallPaths::discover_with_service_manager(Some(ServiceManagerChoice::Dinit))
        .expect("explicit choice should resolve");

    assert_eq!(paths.service.artifact_root(), xdg_root.join("dinit.d"));
    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("XDG_CONFIG_HOME", previous_xdg);
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
fn service_manager_choice_accepts_cli_names() {
    assert_eq!(
        ServiceManagerChoice::parse("systemd").expect("systemd"),
        ServiceManagerChoice::Systemd
    );
    assert_eq!(
        ServiceManagerChoice::parse("dinit").expect("dinit"),
        ServiceManagerChoice::Dinit
    );
    assert_eq!(
        ServiceManagerChoice::parse("runit").expect("runit"),
        ServiceManagerChoice::Runit
    );
    assert_eq!(
        ServiceManagerChoice::parse("s6").expect("s6"),
        ServiceManagerChoice::S6
    );
}

#[test]
fn install_paths_use_explicit_runit_service_dir_first() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let explicit_root = PathBuf::from(&home).join(".config-explicit-runit-test");
    let svdir_root = PathBuf::from(&home).join(".config-svdir-runit-test");
    let previous_explicit = set_env(
        "UNIXNOTIS_RUNIT_SERVICE_DIR",
        Some(explicit_root.to_string_lossy().as_ref()),
    );
    let previous_svdir = set_env("SVDIR", Some(svdir_root.to_string_lossy().as_ref()));
    let previous_xdg = set_env(
        "XDG_CONFIG_HOME",
        Some(
            PathBuf::from(&home)
                .join(".config-xdg-runit-test")
                .to_string_lossy()
                .as_ref(),
        ),
    );
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("runit"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    assert_eq!(paths.service.artifact_root(), explicit_root);
    assert_eq!(
        paths.service.primary_artifact_path(),
        explicit_root.join("unixnotis-daemon")
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("XDG_CONFIG_HOME", previous_xdg);
    restore_env("SVDIR", previous_svdir);
    restore_env("UNIXNOTIS_RUNIT_SERVICE_DIR", previous_explicit);
}

#[test]
fn install_paths_use_svdir_for_runit_services_when_explicit_unset() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let svdir_root = PathBuf::from(&home).join(".config-svdir-runit-test");
    let previous_explicit = set_env("UNIXNOTIS_RUNIT_SERVICE_DIR", None);
    let previous_svdir = set_env("SVDIR", Some(svdir_root.to_string_lossy().as_ref()));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("runit"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    assert_eq!(paths.service.artifact_root(), svdir_root);
    assert_eq!(
        paths.service.primary_artifact_path(),
        svdir_root.join("unixnotis-daemon")
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("SVDIR", previous_svdir);
    restore_env("UNIXNOTIS_RUNIT_SERVICE_DIR", previous_explicit);
}

#[test]
fn install_paths_use_home_config_for_runit_services_when_overrides_unset() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let previous_explicit = set_env("UNIXNOTIS_RUNIT_SERVICE_DIR", None);
    let previous_svdir = set_env("SVDIR", None);
    let previous_xdg = set_env("XDG_CONFIG_HOME", Some("/tmp/ignored-xdg-runit"));
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
    restore_env("XDG_CONFIG_HOME", previous_xdg);
    restore_env("SVDIR", previous_svdir);
    restore_env("UNIXNOTIS_RUNIT_SERVICE_DIR", previous_explicit);
}

#[test]
fn install_paths_use_home_local_share_for_s6_services_when_overrides_unset() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let previous_data = set_env("UNIXNOTIS_S6_DATA_DIR", None);
    let previous_xdg_data = set_env("XDG_DATA_HOME", None);
    let previous_live = set_env("UNIXNOTIS_S6RC_LIVE_DIR", None);
    let previous_user = set_env("USER", Some("unixnotis-test-user"));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("s6"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    assert_eq!(
        paths.service.artifact_root(),
        PathBuf::from(&home).join(".local").join("share").join("s6")
    );
    assert_eq!(
        paths.service.primary_artifact_path(),
        PathBuf::from(&home)
            .join(".local")
            .join("share")
            .join("s6")
            .join("sv")
            .join("unixnotis-daemon")
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("USER", previous_user);
    restore_env("UNIXNOTIS_S6RC_LIVE_DIR", previous_live);
    restore_env("XDG_DATA_HOME", previous_xdg_data);
    restore_env("UNIXNOTIS_S6_DATA_DIR", previous_data);
}

#[test]
fn install_paths_use_xdg_data_home_for_s6_services_when_explicit_unset() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let xdg_data = PathBuf::from(&home).join(".local-xdg-data-s6-test");
    let previous_data = set_env("UNIXNOTIS_S6_DATA_DIR", None);
    let previous_xdg_data = set_env("XDG_DATA_HOME", Some(xdg_data.to_string_lossy().as_ref()));
    let previous_live = set_env("UNIXNOTIS_S6RC_LIVE_DIR", None);
    let previous_user = set_env("USER", Some("unixnotis-test-user"));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("s6"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    assert_eq!(paths.service.artifact_root(), xdg_data.join("s6"));
    assert_eq!(
        paths.service.primary_artifact_path(),
        xdg_data.join("s6").join("sv").join("unixnotis-daemon")
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("USER", previous_user);
    restore_env("UNIXNOTIS_S6RC_LIVE_DIR", previous_live);
    restore_env("XDG_DATA_HOME", previous_xdg_data);
    restore_env("UNIXNOTIS_S6_DATA_DIR", previous_data);
}

#[test]
fn install_paths_use_explicit_s6_roots() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let data_root = PathBuf::from(&home).join(".local-s6-data-test");
    let live_root = PathBuf::from(&home).join(".local-s6-live-test");
    let previous_data = set_env(
        "UNIXNOTIS_S6_DATA_DIR",
        Some(data_root.to_string_lossy().as_ref()),
    );
    let previous_live = set_env(
        "UNIXNOTIS_S6RC_LIVE_DIR",
        Some(live_root.to_string_lossy().as_ref()),
    );
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("s6"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    assert_eq!(paths.service.artifact_root(), data_root);
    assert_eq!(
        paths.service.primary_artifact_path(),
        data_root.join("sv").join("unixnotis-daemon")
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("UNIXNOTIS_S6RC_LIVE_DIR", previous_live);
    restore_env("UNIXNOTIS_S6_DATA_DIR", previous_data);
}

#[test]
fn install_paths_reject_relative_runit_service_dir_override() {
    let _guard = env_lock();
    let previous_explicit = set_env("UNIXNOTIS_RUNIT_SERVICE_DIR", Some("relative/service"));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("runit"));

    let Err(err) = InstallPaths::discover() else {
        panic!("relative runit override should fail");
    };

    assert!(err.to_string().contains("must be an absolute path"));
    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("UNIXNOTIS_RUNIT_SERVICE_DIR", previous_explicit);
}
