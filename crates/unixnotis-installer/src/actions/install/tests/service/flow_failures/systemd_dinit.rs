use std::fs;

use crate::service_manager::ServiceManager;

use super::super::flow_support::{
    fake_failure_env, flow_env, flow_paths, lock_env, read_calls, run_install_and_enable,
    service_flow_root, write_fake_tools, FakeToolMode,
};

#[test]
fn systemd_install_fails_before_env_sync_when_daemon_reload_fails() {
    let _lock = lock_env();
    let root = service_flow_root("install-fail-systemd-reload");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
    let _env = flow_env(&root, &fake_bin);
    let _failure = fake_failure_env("systemctl", "daemon-reload");
    let paths = flow_paths(
        &root,
        ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    );

    let err = run_install_and_enable(&paths).expect_err("daemon reload should fail");

    // Reload failure must stop before importing environment or enabling a stale unit
    assert!(err.to_string().contains("command failed"));
    let calls = read_calls(&log_path);
    assert!(calls
        .iter()
        .any(|call| call.contains("program=systemctl argv=[--user][daemon-reload]")));
    assert!(
        !calls
            .iter()
            .any(|call| call.contains("dbus-update-activation-environment")),
        "env sync should not run after a failed manager reload"
    );
    assert!(
        !calls.iter().any(|call| call.contains("[enable][--now]")),
        "service start should not run after a failed manager reload"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn dinit_install_fails_before_start_when_setenv_fails() {
    let _lock = lock_env();
    let root = service_flow_root("install-fail-dinit-setenv");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    write_fake_tools(&fake_bin, &log_path, FakeToolMode::Default);
    let _env = flow_env(&root, &fake_bin);
    let _failure = fake_failure_env("dinitctl", "setenv");
    let paths = flow_paths(
        &root,
        ServiceManager::dinit_user(root.join("home").join(".config").join("dinit.d")),
    );

    let err = run_install_and_enable(&paths).expect_err("dinit setenv should fail");

    // dinit has no artifact-backed env fallback, so a failed setenv prevents service start
    assert!(err
        .to_string()
        .contains("failed to synchronize service manager environment"));
    let calls = read_calls(&log_path);
    assert!(calls
        .iter()
        .any(|call| call.contains("program=dinitctl argv=[--user][setenv]")));
    assert!(
        !calls
            .iter()
            .any(|call| call.contains("program=dinitctl argv=[--user][start]")),
        "dinit start should not run when env sync failed"
    );
    let _ = fs::remove_dir_all(&root);
}
