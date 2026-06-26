use std::fs;

use crate::service_manager::ServiceManager;

use super::super::flow_support::{
    assert_call_order, fake_failure_env, flow_env, flow_paths, lock_env, read_calls,
    run_install_and_enable, run_install_only, run_uninstall_only, service_flow_root,
    write_fake_tools, FakeToolMode,
};

#[test]
fn s6_install_fails_before_env_sync_when_database_reload_fails() {
    let _lock = lock_env();
    let root = service_flow_root("install-fail-s6-db-reload");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
    let _env = flow_env(&root, &fake_bin);
    let _failure = fake_failure_env("s6-db-reload", "-u");
    let paths = flow_paths(
        &root,
        ServiceManager::s6_user(
            root.join("home").join(".local").join("share").join("s6"),
            root.join("run").join("s6-rc"),
        ),
    );

    let err = run_install_and_enable(&paths).expect_err("s6 database reload should fail");

    // A failed database reload means the live tree cannot know about the new service yet
    assert!(err.to_string().contains("command failed"));
    let calls = read_calls(&log_path);
    assert!(calls
        .iter()
        .any(|call| call.contains("program=s6-db-reload argv=[-u]")));
    assert!(
        !calls.iter().any(|call| call.contains("program=s6-rc")),
        "s6-rc change should not run after database reload failure"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn s6_install_fails_after_env_sync_when_change_fails() {
    let _lock = lock_env();
    let root = service_flow_root("install-fail-s6-change");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
    let _env = flow_env(&root, &fake_bin);
    let _failure = fake_failure_env("s6-rc", "change");
    let paths = flow_paths(
        &root,
        ServiceManager::s6_user(
            root.join("home").join(".local").join("share").join("s6"),
            root.join("run").join("s6-rc"),
        ),
    );

    let err = run_install_and_enable(&paths).expect_err("s6 change should fail");

    // Envdir files are written before the live service change command is attempted
    assert!(err.to_string().contains("command failed"));
    assert!(paths
        .service
        .primary_artifact_path()
        .join("env")
        .join("WAYLAND_DISPLAY")
        .is_file());
    let calls = read_calls(&log_path);
    assert_call_order(
        &calls,
        &["program=s6-db-reload argv=[-u]", "program=s6-rc argv=[-l]"],
    );
    assert!(calls
        .iter()
        .any(|call| call.contains("[change][unixnotis-daemon]")));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn uninstall_removes_artifacts_even_when_stop_command_fails() {
    let _lock = lock_env();
    let root = service_flow_root("uninstall-fail-stop");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
    let _env = flow_env(&root, &fake_bin);
    let _failure = fake_failure_env("systemctl", "disable");
    let paths = flow_paths(
        &root,
        ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    );
    run_install_only(&paths).expect("systemd artifact should be written");
    let unit_path = paths.service.primary_artifact_path();
    assert!(unit_path.is_file());

    run_uninstall_only(&paths).expect("artifact cleanup should continue after stop warning");

    // Disable failures are logged as warnings because artifact cleanup is still safe and desired
    assert!(fs::symlink_metadata(&unit_path).is_err());
    let calls = read_calls(&log_path);
    assert_call_order(
        &calls,
        &[
            "program=systemctl argv=[--user][disable][--now][unixnotis-daemon.service]",
            "program=systemctl argv=[--user][daemon-reload]",
        ],
    );
    let _ = fs::remove_dir_all(&root);
}
