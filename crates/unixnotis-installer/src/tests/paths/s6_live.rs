use super::*;

#[test]
fn install_paths_use_existing_local_s6_live_root_when_run_root_is_missing() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let data_root = env::temp_dir().join(format!("unixnotis-s6-local-live-{}", std::process::id()));
    let local_live = data_root.join("rc").join("live");
    fs::create_dir_all(&local_live).expect("local live root");
    let previous_data = set_env(
        "UNIXNOTIS_S6_DATA_DIR",
        Some(data_root.to_string_lossy().as_ref()),
    );
    let previous_live = set_env("UNIXNOTIS_S6RC_LIVE_DIR", None);
    let previous_user = set_env("USER", Some("unixnotis-test-user"));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("s6"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    assert_eq!(paths.service.artifact_root(), data_root.as_path());
    assert_eq!(
        paths
            .service
            .start_command()
            .expect("s6 start command")
            .args(),
        &[
            "-l",
            local_live.to_string_lossy().as_ref(),
            "-u",
            "change",
            "unixnotis-daemon"
        ]
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("USER", previous_user);
    restore_env("UNIXNOTIS_S6RC_LIVE_DIR", previous_live);
    restore_env("UNIXNOTIS_S6_DATA_DIR", previous_data);
    let _ = fs::remove_dir_all(data_root);
}

#[test]
fn install_paths_use_existing_tmp_s6_live_root_for_standalone_supervision() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let user = format!("unixnotis-s6-test-{}", std::process::id());
    let standalone_live = PathBuf::from("/tmp").join(&user).join("s6-rc");
    fs::create_dir_all(&standalone_live).expect("standalone live root");
    let previous_data = set_env("UNIXNOTIS_S6_DATA_DIR", None);
    let previous_live = set_env("UNIXNOTIS_S6RC_LIVE_DIR", None);
    let previous_user = set_env("USER", Some(&user));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("s6"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    // Artix standalone local s6 uses a user-owned live root outside /run
    assert_eq!(
        paths
            .service
            .start_command()
            .expect("s6 start command")
            .args(),
        &[
            "-l",
            standalone_live.to_string_lossy().as_ref(),
            "-u",
            "change",
            "unixnotis-daemon"
        ]
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("USER", previous_user);
    restore_env("UNIXNOTIS_S6RC_LIVE_DIR", previous_live);
    restore_env("UNIXNOTIS_S6_DATA_DIR", previous_data);
    let _ = fs::remove_dir_all(PathBuf::from("/tmp").join(user));
}

#[test]
fn install_paths_ignore_symlinked_tmp_s6_live_root() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let user = format!("unixnotis-s6-symlink-test-{}", std::process::id());
    let temp_user_root = PathBuf::from("/tmp").join(&user);
    let real_live = env::temp_dir().join(format!("unixnotis-real-s6-live-{user}"));
    let linked_live = temp_user_root.join("s6-rc");
    fs::create_dir_all(&temp_user_root).expect("tmp user root");
    fs::create_dir_all(&real_live).expect("real live root");
    std::os::unix::fs::symlink(&real_live, &linked_live).expect("symlinked live root");
    let previous_data = set_env("UNIXNOTIS_S6_DATA_DIR", None);
    let previous_live = set_env("UNIXNOTIS_S6RC_LIVE_DIR", None);
    let previous_user = set_env("USER", Some(&user));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("s6"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let expected_fallback = PathBuf::from("/run").join(&user).join("s6-rc");
    let expected_fallback = expected_fallback.to_string_lossy();

    // Auto-detection must not follow a symlinked /tmp live root into another tree
    assert_eq!(
        paths
            .service
            .start_command()
            .expect("s6 start command")
            .args(),
        &[
            "-l",
            expected_fallback.as_ref(),
            "-u",
            "change",
            "unixnotis-daemon"
        ]
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("USER", previous_user);
    restore_env("UNIXNOTIS_S6RC_LIVE_DIR", previous_live);
    restore_env("UNIXNOTIS_S6_DATA_DIR", previous_data);
    let _ = fs::remove_dir_all(PathBuf::from("/tmp").join(user));
    let _ = fs::remove_dir_all(real_live);
}

#[test]
fn install_paths_allow_explicit_s6_live_root_with_default_data_root() {
    let _guard = env_lock();
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let live_root = PathBuf::from(&home).join(".local-s6-live-test");
    let previous_data = set_env("UNIXNOTIS_S6_DATA_DIR", None);
    let previous_live = set_env(
        "UNIXNOTIS_S6RC_LIVE_DIR",
        Some(live_root.to_string_lossy().as_ref()),
    );
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("s6"));

    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");

    assert_eq!(
        paths.service.artifact_root(),
        PathBuf::from(&home).join(".local").join("share").join("s6")
    );

    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("UNIXNOTIS_S6RC_LIVE_DIR", previous_live);
    restore_env("UNIXNOTIS_S6_DATA_DIR", previous_data);
}
