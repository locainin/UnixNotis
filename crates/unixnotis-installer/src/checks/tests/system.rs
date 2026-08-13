use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::checks::CheckState;
use crate::paths::InstallPaths;
use crate::service_manager::contract::ServiceManagerAvailability;
use crate::service_manager::{ReadinessIssue, ServiceManager};
use crate::test_support::fs::write_executable;

use super::{
    command_success, dbus_update_env_check, install_paths_check, path_is_writable,
    readiness_error_detail, readiness_messages, readiness_warning_detail,
    service_manager_check_from, wayland_check,
};

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    // PATH is process-wide, so service-manager check tests use the crate-wide guard
    crate::test_support::env::test_env_lock()
}

#[test]
fn readiness_error_detail_collects_only_blocking_issues() {
    let issues = [
        ReadinessIssue::warning("boot setup incomplete"),
        ReadinessIssue::error("s6-rc-compile not found"),
        ReadinessIssue::error("s6 live directory missing"),
    ];

    let detail = readiness_error_detail(&issues).expect("blocking detail");

    assert!(detail.contains("s6-rc-compile not found"));
    assert!(detail.contains("s6 live directory missing"));
    assert!(!detail.contains("boot setup incomplete"));
}

#[test]
fn wayland_check_accepts_exact_wayland_session_without_display_fallback() {
    let _lock = env_lock();
    let root = test_root("exact-wayland-session-check");
    let _session = crate::test_support::env::EnvGuard::set("XDG_SESSION_TYPE", "wayland");
    let _display = crate::test_support::env::EnvGuard::set("WAYLAND_DISPLAY", "");
    let _runtime = crate::test_support::env::EnvGuard::set("XDG_RUNTIME_DIR", &root);

    let item = wayland_check();

    assert_eq!(item.state, CheckState::Ok);
    assert_eq!(item.detail, "session detected");
}

#[test]
fn readiness_warning_detail_keeps_backend_label() {
    let manager = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit.d"));
    let issues = [ReadinessIssue::warning("boot setup incomplete")];

    let detail = readiness_warning_detail(&manager, &issues).expect("warning detail");

    assert_eq!(
        detail,
        "dinit --user ready with warnings: boot setup incomplete"
    );
}

#[test]
fn readiness_messages_split_warnings_and_errors() {
    let issues = [
        ReadinessIssue::warning("warning one"),
        ReadinessIssue::error("error one"),
        ReadinessIssue::warning("warning two"),
    ];

    assert_eq!(
        readiness_messages(&issues, false),
        ["warning one".to_string(), "warning two".to_string()]
    );
    assert_eq!(readiness_messages(&issues, true), ["error one".to_string()]);
}

#[test]
fn service_manager_check_uses_canonical_reachable_nonzero_systemd_state() {
    let _lock = env_lock();
    let root = test_root("reachable-nonzero-systemd-check");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    write_fake_tool(
        &fake_bin.join("systemctl"),
        "#!/bin/sh\nprintf '%s\\n' degraded\nexit 1\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let manager = ServiceManager::systemd_user(root.join("systemd"));

    let availability = manager
        .availability_state()
        .expect("systemd availability query")
        .expect("systemd manager-level probe");
    let item = service_manager_check_from(&manager);

    assert_eq!(availability, ServiceManagerAvailability::Available);
    assert_eq!(item.state, CheckState::Ok);
    assert_eq!(item.detail, "systemd --user available");
    fs::remove_dir_all(root).expect("remove systemd availability fixture");
}

#[test]
fn service_manager_check_rejects_indeterminate_systemd_availability() {
    let _lock = env_lock();
    let root = test_root("indeterminate-systemd-check");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    write_fake_tool(
        &fake_bin.join("systemctl"),
        "#!/bin/sh\nprintf '%s\\n' unexpected-manager-state\nexit 1\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let manager = ServiceManager::systemd_user(root.join("systemd"));

    let item = service_manager_check_from(&manager);

    assert_eq!(item.state, CheckState::Fail);
    assert_eq!(item.detail, "systemd --user availability is indeterminate");
    fs::remove_dir_all(root).expect("remove indeterminate systemd fixture");
}

#[test]
fn service_manager_check_rejects_unavailable_systemd_transport() {
    let _lock = env_lock();
    let root = test_root("unavailable-systemd-check");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    write_fake_tool(
        &fake_bin.join("systemctl"),
        "#!/bin/sh\nprintf '%s\\n' 'Failed to connect to bus: No medium found' >&2\nexit 1\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let manager = ServiceManager::systemd_user(root.join("systemd"));

    let item = service_manager_check_from(&manager);

    assert_eq!(item.state, CheckState::Fail);
    assert_eq!(item.detail, "systemd --user unavailable");
    fs::remove_dir_all(root).expect("remove unavailable systemd fixture");
}

#[test]
fn service_manager_check_accepts_absent_runit_service_as_ready_backend() {
    let _lock = env_lock();
    let root = test_root("absent-runit-service-check");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    write_fake_tool(&fake_bin.join("chpst"), "#!/bin/sh\nexit 0\n");
    write_fake_tool(
        &fake_bin.join("sv"),
        "#!/bin/sh\nprintf '%s\\n' 'fail: unixnotis-daemon: runsv not running'\nexit 1\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let manager = ServiceManager::runit_user(root.join("service"));

    let item = service_manager_check_from(&manager);

    assert_eq!(item.state, CheckState::Ok);
    assert_eq!(item.detail, "runit user services ready");
    fs::remove_dir_all(root).expect("remove absent runit fixture");
}

#[test]
fn service_manager_check_rejects_ambiguous_runit_service_state() {
    let _lock = env_lock();
    let root = test_root("ambiguous-runit-service-check");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    write_fake_tool(&fake_bin.join("chpst"), "#!/bin/sh\nexit 0\n");
    write_fake_tool(
        &fake_bin.join("sv"),
        "#!/bin/sh\nprintf '%s\\n' ambiguous\nexit 0\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let manager = ServiceManager::runit_user(root.join("service"));

    let item = service_manager_check_from(&manager);

    assert_eq!(item.state, CheckState::Fail);
    assert_eq!(item.detail, "runit user services state is indeterminate");
    fs::remove_dir_all(root).expect("remove ambiguous runit fixture");
}

#[test]
fn service_manager_check_fails_for_s6_missing_live_directory() {
    let _lock = env_lock();
    let root = test_root("s6-missing-live-check");
    let fake_bin = root.join("fake-bin");
    write_fake_s6_tools(&fake_bin);
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let data = root.join("s6");
    let default_dir = data.join("sv").join("default");
    fs::create_dir_all(&default_dir).expect("default bundle dir");
    fs::write(default_dir.join("type"), "bundle\n").expect("default bundle type");
    let manager = ServiceManager::s6_user(data, root.join("run").join("s6-rc"));

    let item = service_manager_check_from(&manager);

    assert_eq!(item.state, CheckState::Fail);
    assert!(item.detail.contains("s6 live directory"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn service_manager_check_warns_for_initializable_s6_layout() {
    let _lock = env_lock();
    let root = test_root("s6-initializable-check");
    let fake_bin = root.join("fake-bin");
    write_fake_s6_tools(&fake_bin);
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let data = root.join("s6");
    let live = root.join("run").join("s6-rc");
    fs::create_dir_all(&live).expect("live dir");
    let manager = ServiceManager::s6_user(data, live);

    let item = service_manager_check_from(&manager);

    assert_eq!(item.state, CheckState::Warn);
    assert!(item
        .detail
        .contains("s6-rc user services ready with warnings"));
    assert!(item.detail.contains("default bundle is missing"));
    assert!(item.detail.contains("source directory is missing"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn service_manager_check_fails_when_s6_required_tool_is_missing() {
    let _lock = env_lock();
    let root = test_root("s6-missing-tool-check");
    let fake_bin = root.join("fake-bin");
    write_fake_s6_tools_except(&fake_bin, "s6-envdir");
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let data = root.join("s6");
    let default_dir = data.join("sv").join("default");
    fs::create_dir_all(&default_dir).expect("default bundle dir");
    fs::write(default_dir.join("type"), "bundle\n").expect("default bundle type");
    let live = root.join("run").join("s6-rc");
    fs::create_dir_all(&live).expect("live dir");
    let manager = ServiceManager::s6_user(data, live);

    let item = service_manager_check_from(&manager);

    assert_eq!(item.state, CheckState::Fail);
    assert!(item.detail.contains("s6-envdir not found"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn service_manager_check_fails_for_user_owned_s6_default_type() {
    let _lock = env_lock();
    let root = test_root("s6-invalid-default-type-check");
    let fake_bin = root.join("fake-bin");
    write_fake_s6_tools(&fake_bin);
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let data = root.join("s6");
    let default_dir = data.join("sv").join("default");
    fs::create_dir_all(&default_dir).expect("default bundle dir");
    fs::write(default_dir.join("type"), "longrun\n").expect("foreign default type");
    let live = root.join("run").join("s6-rc");
    fs::create_dir_all(&live).expect("live dir");
    let manager = ServiceManager::s6_user(data, live);

    let item = service_manager_check_from(&manager);

    assert_eq!(item.state, CheckState::Fail);
    assert!(item
        .detail
        .contains("refusing to overwrite user service layout"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dbus_update_env_check_warns_when_helper_is_not_on_path() {
    let _lock = env_lock();
    let root = test_root("missing-dbus-update-env-check");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    let manager = ServiceManager::systemd_user(root.join("systemd"));

    let item = dbus_update_env_check(Some(&manager));

    assert_eq!(item.state, CheckState::Warn);
    assert!(item.detail.contains("not found"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dbus_update_env_check_is_ok_when_selected_backend_does_not_need_helper() {
    let _lock = env_lock();
    let root = test_root("missing-dbus-update-env-non-systemd-check");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let manager = ServiceManager::runit_user(root.join("service"));

    let item = dbus_update_env_check(Some(&manager));

    assert_eq!(item.state, CheckState::Ok);
    assert!(item.detail.contains("not required"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn install_paths_check_fails_when_service_root_is_not_directory() {
    let root = test_root("install-paths-service-file-check");
    let bin_dir = root.join("bin");
    let service_root = root.join("service-root");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    fs::write(&service_root, "not a directory\n").expect("service root file");
    let paths = InstallPaths {
        repo_root: root.clone(),
        bin_dir,
        service: ServiceManager::systemd_user(service_root),
    };

    let item = install_paths_check(&paths);

    assert_eq!(item.state, CheckState::Fail);
    assert_eq!(item.detail, "not writable");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn install_paths_check_accepts_writable_binary_and_service_directories() {
    let root = test_root("writable-install-paths-check");
    let bin_dir = root.join("bin");
    let service_root = root.join("service-root");
    fs::create_dir_all(&bin_dir).expect("bin dir");
    fs::create_dir_all(&service_root).expect("service root");
    let paths = InstallPaths {
        repo_root: root.clone(),
        bin_dir,
        service: ServiceManager::systemd_user(service_root),
    };

    let item = install_paths_check(&paths);

    assert_eq!(item.state, CheckState::Ok);
    assert_eq!(item.detail, "writable");
    fs::remove_dir_all(root).expect("remove writable install paths fixture");
}

#[test]
fn path_is_writable_accepts_a_real_directory_and_removes_its_probe() {
    let root = test_root("writable-path-check");
    fs::create_dir_all(&root).expect("writable directory");

    assert!(path_is_writable(&root));
    assert_eq!(fs::read_dir(&root).expect("empty directory").count(), 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn path_is_writable_rejects_a_symlinked_directory() {
    let root = test_root("linked-writable-path-check");
    let outside = root.join("outside");
    let linked = root.join("linked");
    fs::create_dir_all(&outside).expect("outside directory");
    std::os::unix::fs::symlink(&outside, &linked).expect("linked directory");

    assert!(!path_is_writable(&linked));
    assert_eq!(
        fs::read_dir(&outside).expect("untouched directory").count(),
        0
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn command_success_distinguishes_success_failure_and_missing_trusted_tools() {
    let _lock = env_lock();
    let root = test_root("command-success");
    let fake_bin = root.join("fake-bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    write_fake_tool(&fake_bin.join("ok-tool"), "#!/bin/sh\nexit 0\n");
    write_fake_tool(&fake_bin.join("fail-tool"), "#!/bin/sh\nexit 7\n");
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    assert_eq!(command_success("ok-tool", &[]), Ok(true));
    assert_eq!(command_success("fail-tool", &[]), Ok(false));
    assert!(command_success("missing-tool", &[]).is_err());
    let _ = fs::remove_dir_all(root);
}

fn write_fake_s6_tools(fake_bin: &std::path::Path) {
    fs::create_dir_all(fake_bin).expect("fake bin dir");
    for tool in [
        "s6-rc-compile",
        "s6-rc-update",
        "s6-rc",
        "s6-envdir",
        "s6-svstat",
    ] {
        let path = fake_bin.join(tool);
        write_executable(&path, "#!/bin/sh\nexit 0\n");
    }
    write_executable(
        &fake_bin.join("s6-svstat"),
        "#!/bin/sh\n# Exit 1 means the service is not supervised\nexit 1\n",
    );
}

fn write_fake_tool(path: &std::path::Path, contents: &str) {
    write_executable(path, contents);
}

fn write_fake_s6_tools_except(fake_bin: &std::path::Path, missing_tool: &str) {
    fs::create_dir_all(fake_bin).expect("fake bin dir");
    for tool in [
        "s6-rc-compile",
        "s6-rc-update",
        "s6-rc",
        "s6-envdir",
        "s6-svstat",
    ] {
        if tool == missing_tool {
            // Leave one command absent so readiness reports the exact missing helper
            continue;
        }
        let path = fake_bin.join(tool);
        write_executable(&path, "#!/bin/sh\nexit 0\n");
    }
    if missing_tool != "s6-svstat" {
        write_executable(
            &fake_bin.join("s6-svstat"),
            "#!/bin/sh\n# Exit 1 means the service is not supervised\nexit 1\n",
        );
    }
}

fn test_root(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let root = env::temp_dir().join(format!("unixnotis-{name}-{}-{suffix}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
