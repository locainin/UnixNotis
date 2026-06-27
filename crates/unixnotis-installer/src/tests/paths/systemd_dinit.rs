use super::*;

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
