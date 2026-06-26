use std::fs;
use std::os::unix::fs::symlink;

use crate::service_manager::ServiceManager;

use super::super::flow_support::{
    fake_failure_env, flow_env, flow_paths, lock_env, read_calls, run_enable_only,
    run_install_and_enable, run_install_only, service_flow_root, write_fake_tools, FakeToolMode,
};

#[test]
fn runit_envdir_sync_failure_keeps_down_gate() {
    let _lock = lock_env();
    let root = service_flow_root("install-fail-runit-envdir");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    write_fake_tools(&fake_bin, &log_path, FakeToolMode::RunitSv);
    let _env = flow_env(&root, &fake_bin);
    let service_root = root.join("home").join(".config").join("service");
    let paths = flow_paths(&root, ServiceManager::runit_user(service_root));
    run_install_only(&paths).expect("runit install artifacts should be written");
    let service_dir = paths.service.primary_artifact_path();
    let env_target = root.join("foreign-env-target");
    fs::create_dir_all(&env_target).expect("foreign env target");
    // The envdir path is made unsafe after install so only env sync fails
    symlink(&env_target, service_dir.join("env")).expect("unsafe envdir link");

    let err = run_enable_only(&paths).expect_err("envdir sync should fail");

    // The down gate must remain because env sync failed before the service was allowed to start
    assert!(format!("{err:#}").contains("cannot replace symlink service directory"));
    assert!(service_dir.join("down").is_file());
    let calls = if log_path.exists() {
        read_calls(&log_path)
    } else {
        Vec::new()
    };
    assert!(
        !calls
            .iter()
            .any(|call| call.contains("program=sv argv=[start]")),
        "runit start should not run after envdir sync failure"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn runit_start_failure_happens_after_down_gate_removal() {
    let _lock = lock_env();
    let root = service_flow_root("install-fail-runit-start");
    let log_path = root.join("calls.log");
    let fake_bin = root.join("fake-bin");
    write_fake_tools(&fake_bin, &log_path, FakeToolMode::RunitSv);
    let _env = flow_env(&root, &fake_bin);
    let _failure = fake_failure_env("sv", "start");
    let service_root = root.join("home").join(".config").join("service");
    let paths = flow_paths(&root, ServiceManager::runit_user(service_root));

    let err = run_install_and_enable(&paths).expect_err("runit start should fail");

    // The fake sv sees runit_ready=yes only when envdir exists and down has already been removed
    assert!(err.to_string().contains("command failed"));
    let service_dir = paths.service.primary_artifact_path();
    assert!(service_dir.join("env").join("WAYLAND_DISPLAY").is_file());
    assert!(fs::symlink_metadata(service_dir.join("down")).is_err());
    let calls = read_calls(&log_path);
    assert!(calls
        .iter()
        .any(|call| call.contains("program=sv argv=[start]") && call.contains("runit_ready=yes")));
    let _ = fs::remove_dir_all(&root);
}
