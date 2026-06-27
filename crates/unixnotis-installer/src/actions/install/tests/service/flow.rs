use std::fs;

// These tests execute the real installer service phases against fake manager binaries
// That catches ordering bugs that pure CommandSpec tests cannot see
use crate::service_manager::ServiceManager;

use super::flow_support::{
    assert_call_order, flow_env, flow_paths, lock_env, read_calls, run_install_and_enable,
    service_flow_root, write_fake_tools, FakeToolMode,
};

#[test]
fn systemd_install_flow_runs_reload_env_import_and_enable() {
    let _lock = lock_env();
    // The lock keeps process-wide environment edits serialized across this test module
    let root = service_flow_root("install-flow-systemd");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    // Fake binaries let the real installer flow run without depending on host systemd state
    write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
    let _env = flow_env(&root, &fake_bin);
    let paths = flow_paths(
        &root,
        ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    );

    run_install_and_enable(&paths).expect("systemd flow should complete");

    let calls = read_calls(&log_path);
    // systemd still owns reload, D-Bus import, systemd import, and enable --now
    assert_call_order(
        &calls,
        &[
            "program=systemctl argv=[--user][daemon-reload]",
            "program=dbus-update-activation-environment argv=[WAYLAND_DISPLAY]",
            "program=systemctl argv=[--user][--no-pager][import-environment][WAYLAND_DISPLAY]",
            "program=systemctl argv=[--user][enable][--now][unixnotis-daemon.service]",
        ],
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn dinit_install_flow_sets_environment_from_env_and_starts_without_reload() {
    let _lock = lock_env();
    // dinit is command-backed for env sync, so this test checks argv and env separately
    let root = service_flow_root("install-flow-dinit");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
    let _env = flow_env(&root, &fake_bin);
    let paths = flow_paths(
        &root,
        ServiceManager::dinit_user(root.join("home").join(".config").join("dinit.d")),
    );

    run_install_and_enable(&paths).expect("dinit flow should complete");

    let calls = read_calls(&log_path);
    // dinit should receive variable names in argv and sensitive values through env overrides
    assert!(calls
        .iter()
        .any(|call| call.contains("program=dinitctl argv=[--user][setenv][WAYLAND_DISPLAY]")));
    assert!(calls
        .iter()
        .any(|call| call.contains("WAYLAND_DISPLAY=wayland-test")));
    assert!(
        !calls
            .iter()
            .any(|call| call.contains("WAYLAND_DISPLAY=wayland-test]")),
        "dinit env values should stay out of argv"
    );
    assert_call_order(
        &calls,
        &[
            // dinit intentionally has no first-install reload command
            "program=dinitctl argv=[--user][setenv][WAYLAND_DISPLAY]",
            "program=dinitctl argv=[--user][start][unixnotis-daemon]",
        ],
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn runit_install_flow_syncs_envdir_before_removing_down_and_starting() {
    let _lock = lock_env();
    // runit is the race-sensitive backend because runsvdir can start as soon as ./run exists
    let root = service_flow_root("install-flow-runit");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    // The fake sv command fails if start runs before envdir exists or while down remains
    write_fake_tools(&fake_bin, &log_path, FakeToolMode::RunitSv);
    let _env = flow_env(&root, &fake_bin);
    let service_root = root.join("home").join(".config").join("service");
    let paths = flow_paths(&root, ServiceManager::runit_user(service_root));

    run_install_and_enable(&paths).expect("runit flow should complete");

    let service_dir = paths.service.primary_artifact_path();
    // Envdir sync must happen before the temporary down gate is removed
    assert!(service_dir.join("env").join("WAYLAND_DISPLAY").is_file());
    assert!(
        fs::symlink_metadata(service_dir.join("down")).is_err(),
        "runit down gate should be removed immediately before sv start"
    );
    let calls = read_calls(&log_path);
    assert!(
        calls.iter().any(
            |call| call.contains("program=sv argv=[start]") && call.contains("runit_ready=yes")
        ),
        "fake sv should see synced envdir and no down gate at start"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn s6_install_flow_compiles_database_then_changes_service() {
    let _lock = lock_env();
    // s6 needs a compiled database before the live service change can see the new source tree
    let root = service_flow_root("install-flow-s6");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
    let _env = flow_env(&root, &fake_bin);
    fs::create_dir_all(root.join("run").join("s6-rc")).expect("s6 live dir");
    let paths = flow_paths(
        &root,
        ServiceManager::s6_user(
            root.join("home").join(".local").join("share").join("s6"),
            root.join("run").join("s6-rc"),
        ),
    );

    run_install_and_enable(&paths).expect("s6 flow should complete");

    // s6 is envdir-backed, so environment sync should produce files before s6-rc change
    assert!(paths
        .service
        .primary_artifact_path()
        .join("env")
        .join("WAYLAND_DISPLAY")
        .is_file());
    let calls = read_calls(&log_path);
    assert_call_order(
        &calls,
        &[
            "program=s6-rc-compile argv=",
            "program=s6-rc-update argv=[-l]",
            "program=s6-rc argv=[-l]",
        ],
    );
    assert!(calls
        .iter()
        .any(|call| call.contains("[change][unixnotis-daemon]")));
    assert!(
        root.join("home")
            .join(".local")
            .join("share")
            .join("s6")
            .join("rc")
            .join("compiled")
            .exists(),
        "s6 compiled symlink should be switched after database update"
    );
    let _ = fs::remove_dir_all(&root);
}
