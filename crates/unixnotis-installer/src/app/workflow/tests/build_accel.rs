use super::super::build_accel::{
    build_accel_enter_action, handle_build_accel_enter, BuildAccelEnterAction,
};
use crate::actions::{BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome};
use crate::app::{App, BuildAccelMenuMode, BuildAccelState, Screen};

#[test]
fn build_accel_enter_selection_maps_each_menu_mode_to_one_action() {
    assert_eq!(
        build_accel_enter_action(BuildAccelMenuMode::ReturnOnly, 0),
        BuildAccelEnterAction::ReturnToMenu
    );
    assert_eq!(
        build_accel_enter_action(BuildAccelMenuMode::EnableOrSkip, 0),
        BuildAccelEnterAction::ApplySetup
    );
    assert_eq!(
        build_accel_enter_action(BuildAccelMenuMode::EnableOrSkip, 1),
        BuildAccelEnterAction::ReturnToMenu
    );
    assert_eq!(
        build_accel_enter_action(BuildAccelMenuMode::Reinstall, 0),
        BuildAccelEnterAction::ReturnToMenu
    );
    assert_eq!(
        build_accel_enter_action(BuildAccelMenuMode::Reinstall, 1),
        BuildAccelEnterAction::ApplySetup
    );
}

#[test]
fn build_accel_enable_action_writes_repo_local_setup_and_records_the_outcome() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = crate::test_support::fs::unique_temp_path("workflow-build-accel-setup");
    let repo = root.join("repo");
    let home = root.join("home");
    let config_home = home.join(".config");
    std::fs::create_dir_all(&repo).expect("create build acceleration repo");
    std::fs::create_dir_all(&home).expect("create build acceleration home");
    std::fs::write(
        repo.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/unixnotis-daemon\", \"crates/unixnotis-core\"]\n",
    )
    .expect("write build acceleration workspace identity");
    let _repo_env = crate::test_support::env::EnvGuard::set("UNIXNOTIS_REPO_ROOT", &repo);
    let _home_env = crate::test_support::env::EnvGuard::set("HOME", &home);
    let _config_env = crate::test_support::env::EnvGuard::set("XDG_CONFIG_HOME", &config_home);
    let _manager_env =
        crate::test_support::env::EnvGuard::set("UNIXNOTIS_SERVICE_MANAGER", "systemd");
    let mut app = App::new(None);
    app.screen = Screen::BuildAccel;
    app.build_accel = Some(BuildAccelState {
        detection: BuildAccelDetection {
            sccache_installed: true,
            mold_installed: false,
            config_status: BuildAccelConfigStatus::Missing,
        },
        outcome: None,
    });
    app.build_accel_menu_index = 0;

    handle_build_accel_enter(&mut app);

    assert!(matches!(
        app.build_accel
            .as_ref()
            .and_then(|state| state.outcome.as_ref()),
        Some(BuildAccelOutcome::Written { .. })
    ));
    assert!(repo.join(".cargo/config.toml").is_file());
    assert_eq!(app.build_accel_menu_index, 0);
    std::fs::remove_dir_all(root).expect("remove build acceleration workflow fixture");
}
