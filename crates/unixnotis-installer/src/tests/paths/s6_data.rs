use super::*;

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

    // Artix local s6 docs use ~/.local/share/s6 as the source/database root
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
fn install_paths_ignore_xdg_data_home_for_s6_services() {
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

    // Keep the documented Artix path stable until custom XDG roots are proven with s6 tools
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
fn install_paths_allow_explicit_s6_data_root() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let data_root = env::temp_dir().join(format!("unixnotis-s6-data-root-{}", std::process::id()));
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

    let paths = InstallPaths::discover().expect("custom s6 data root should resolve");

    assert_eq!(paths.service.artifact_root(), data_root.as_path());

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("UNIXNOTIS_S6RC_LIVE_DIR", previous_live);
    restore_env("UNIXNOTIS_S6_DATA_DIR", previous_data);
}
