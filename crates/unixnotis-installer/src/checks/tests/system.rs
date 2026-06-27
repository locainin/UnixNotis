use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::checks::CheckState;
use crate::paths::InstallPaths;
use crate::service_manager::{ReadinessIssue, ServiceManager};

use super::system::{
    dbus_update_env_check, install_paths_check, readiness_error_detail, readiness_messages,
    readiness_warning_detail, service_manager_check_from,
};

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    // PATH is process-wide, so service-manager check tests need one shared guard
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock")
}

struct EnvGuard {
    key: &'static str,
    old: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<str>) -> Self {
        let old = env::var(key).ok();
        env::set_var(key, value.as_ref());
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.old {
            // Restore fake PATHs so unrelated tests see the original command lookup
            Some(value) => env::set_var(self.key, value),
            None => env::remove_var(self.key),
        }
    }
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
fn service_manager_check_fails_for_s6_missing_live_directory() {
    let _lock = env_lock();
    let root = test_root("s6-missing-live-check");
    let fake_bin = root.join("fake-bin");
    write_fake_s6_tools(&fake_bin);
    let _path = EnvGuard::set("PATH", fake_bin.to_string_lossy());
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
    let _path = EnvGuard::set("PATH", fake_bin.to_string_lossy());
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
    let _path = EnvGuard::set("PATH", fake_bin.to_string_lossy());
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
    let _path = EnvGuard::set("PATH", fake_bin.to_string_lossy());
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
    let _path = EnvGuard::set("PATH", fake_bin.to_string_lossy());

    let item = dbus_update_env_check();

    assert_eq!(item.state, CheckState::Warn);
    assert!(item.detail.contains("not found"));
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
        fs::write(&path, "#!/bin/sh\nexit 0\n").expect("fake s6 tool");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("fake tool mode");
    }
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
        fs::write(&path, "#!/bin/sh\nexit 0\n").expect("fake s6 tool");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("fake tool mode");
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
