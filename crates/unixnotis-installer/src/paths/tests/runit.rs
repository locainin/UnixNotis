use super::*;

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
