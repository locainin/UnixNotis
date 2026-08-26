use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::service_manager::{ServiceArtifactKind, ServiceManager};
use crate::system_tools::routing::use_fake_tool_bin;
use crate::test_support::fs::write_executable as write_test_executable;

use super::super::runit::SERVICE_NAME as UNIXNOTIS_DAEMON_RUNIT_SERVICE;

#[test]
fn runit_backend_renders_service_directory_and_run_script() {
    let manager = ServiceManager::runit_user(PathBuf::from("/tmp/service"));
    let artifacts = manager.artifacts(Path::new("/tmp/bin"));

    // Steady state is only the managed directory plus executable run script
    assert_eq!(artifacts.len(), 2);
    assert_eq!(
        artifacts[0].path,
        PathBuf::from("/tmp/service").join(UNIXNOTIS_DAEMON_RUNIT_SERVICE)
    );
    assert_eq!(artifacts[0].kind, ServiceArtifactKind::ManagedDirectory);
    assert_eq!(
        artifacts[1].path,
        PathBuf::from("/tmp/service/unixnotis-daemon/run")
    );
    assert_eq!(artifacts[1].kind, ServiceArtifactKind::ExecutableFile);
    assert_eq!(artifacts[1].mode, Some(0o755));
    assert_eq!(
        artifacts[1]
            .contents
            .as_ref()
            .expect("runit run script should render contents"),
        "#!/bin/sh\n\
         PATH='/usr/local/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin'; export PATH\n\
         exec chpst -e ./env '/tmp/bin/unixnotis-daemon'\n"
    );
}

#[test]
fn runit_backend_install_artifacts_write_down_before_run_script() {
    let manager = ServiceManager::runit_user(PathBuf::from("/tmp/service"));
    let artifacts = manager.install_artifacts(Path::new("/tmp/bin"));

    // Install-time state includes the temporary down gate to prevent runsvdir races
    assert_eq!(artifacts.len(), 3);
    assert_eq!(
        artifacts[0].path,
        PathBuf::from("/tmp/service/unixnotis-daemon")
    );
    assert_eq!(artifacts[0].kind, ServiceArtifactKind::ManagedDirectory);
    assert_eq!(
        artifacts[1].path,
        PathBuf::from("/tmp/service/unixnotis-daemon/down")
    );
    // The down gate is placed before ./run so a watching supervisor cannot start early
    assert_eq!(artifacts[1].kind, ServiceArtifactKind::File);
    assert_eq!(artifacts[1].mode, Some(0o600));
    assert_eq!(artifacts[1].contents.as_deref(), Some(""));
    assert_eq!(
        artifacts[2].path,
        PathBuf::from("/tmp/service/unixnotis-daemon/run")
    );
    assert_eq!(artifacts[2].kind, ServiceArtifactKind::ExecutableFile);
}

#[test]
fn runit_backend_commands_match_expected_behavior() {
    let manager = ServiceManager::runit_user(PathBuf::from("/tmp/service"));
    let service_path = "/tmp/service/unixnotis-daemon";

    // A watched service directory is the enablement source, not an sv query
    assert!(manager.is_enabled_command().is_none());
    assert!(manager.refresh_after_artifact_change().is_none());

    // sv check tracks the requested state, so active status must parse sv status output
    let active = manager.active_probe();
    assert_eq!(active.command().args(), &["status", service_path]);
    assert_eq!(
        active.parser_state(true, "run: /tmp/service/unixnotis-daemon: (pid 123) 2s"),
        crate::service_manager::contract::ServiceProbeState::Active
    );
    assert_eq!(
        active.parser_state(true, "down: /tmp/service/unixnotis-daemon: 1s"),
        crate::service_manager::contract::ServiceProbeState::Inactive
    );
    assert_eq!(
        active.parser_state_with_result(
            Some(1),
            "fail: /tmp/service/unixnotis-daemon: runsv not running\n",
            ""
        ),
        crate::service_manager::contract::ServiceProbeState::Absent
    );
    assert_eq!(
        active.parser_state_with_result(
            Some(1),
            "fail: /tmp/service/unixnotis-daemon: unable to change to service directory: No such file or directory\n",
            ""
        ),
        crate::service_manager::contract::ServiceProbeState::Absent
    );
    assert_eq!(
        active.parser_state_with_result(
            Some(1),
            "timeout: down: /tmp/service/unixnotis-daemon: 30s\n",
            ""
        ),
        crate::service_manager::contract::ServiceProbeState::Indeterminate
    );

    let enable = manager.enable_now_command();
    assert_eq!(enable.args(), &["start", service_path]);

    let disable = manager.disable_now_command();
    assert_eq!(disable.args(), &["stop", service_path]);

    let stop = manager.stop_for_reinstall_command();
    assert_eq!(stop.args(), &["stop", service_path]);
}

#[test]
fn runit_backend_environment_sync_uses_envdir_artifacts() {
    let manager = ServiceManager::runit_user(PathBuf::from("/tmp/service"));
    let names = [
        "WAYLAND_DISPLAY",
        "DISPLAY",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
        "PATH",
    ];
    let vars = [
        ("WAYLAND_DISPLAY", "wayland-1\nignored".to_string()),
        ("XDG_RUNTIME_DIR", "/run/user/1000\t ".to_string()),
        (
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/tmp/unixnotis-bus".to_string(),
        ),
    ];

    // runit has no manager environment import command, so sync is pure envdir artifacts
    let commands = manager.environment_sync_commands(&vars, true);
    let artifacts = manager.environment_sync_artifacts(&names, &vars);

    assert!(commands.is_empty());
    assert_eq!(artifacts.len(), 5);
    assert_eq!(
        artifacts[0].path,
        PathBuf::from("/tmp/service/unixnotis-daemon/env")
    );
    assert_eq!(artifacts[0].kind, ServiceArtifactKind::Directory);
    assert_eq!(
        artifacts[1].path,
        PathBuf::from("/tmp/service/unixnotis-daemon/env/WAYLAND_DISPLAY")
    );
    assert_eq!(artifacts[1].kind, ServiceArtifactKind::File);
    assert_eq!(artifacts[1].mode, Some(0o600));
    assert_eq!(artifacts[1].contents.as_deref(), Some("wayland-1\n"));
    assert_eq!(
        artifacts[2].path,
        PathBuf::from("/tmp/service/unixnotis-daemon/env/DISPLAY")
    );
    assert_eq!(artifacts[2].contents.as_deref(), Some(""));
    assert_eq!(
        artifacts[3].path,
        PathBuf::from("/tmp/service/unixnotis-daemon/env/XDG_RUNTIME_DIR")
    );
    assert_eq!(artifacts[3].contents.as_deref(), Some("/run/user/1000\n"));
    assert_eq!(
        artifacts[4].path,
        PathBuf::from("/tmp/service/unixnotis-daemon/env/DBUS_SESSION_BUS_ADDRESS")
    );
    assert_eq!(
        artifacts[4].contents.as_deref(),
        Some("unix:path=/tmp/unixnotis-bus\n")
    );
    // PATH is intentionally excluded because the run script sets a safe fixed PATH first
    assert!(!artifacts
        .iter()
        .any(|artifact| artifact.path.ends_with("PATH")));
}

#[test]
fn runit_backend_pre_start_removes_down_after_env_sync() {
    let manager = ServiceManager::runit_user(PathBuf::from("/tmp/service"));
    let gates = manager.pre_start_artifacts_to_remove();
    let staged = manager.pre_start_artifacts_to_write();
    let artifacts = manager.install_artifacts(Path::new("/tmp/bin"));

    // The same down file written during install is removed immediately before sv start
    assert_eq!(gates.len(), 1);
    assert!(staged.is_empty());
    assert_eq!(
        gates[0].path,
        PathBuf::from("/tmp/service/unixnotis-daemon/down")
    );
    assert_eq!(gates[0].kind, ServiceArtifactKind::File);
    assert_eq!(artifacts[1], gates[0]);
}

#[test]
fn runit_enabled_state_rejects_symlink_service_directory() {
    let root = test_root("runit-symlink-service-dir");
    let manager = ServiceManager::runit_user(root.join("service"));
    let service = manager.artifact_root().join(UNIXNOTIS_DAEMON_RUNIT_SERVICE);
    let foreign_service = root.join("foreign-service");
    fs::create_dir_all(foreign_service.join("env")).expect("foreign service dir");
    fs::write(foreign_service.join("run"), "#!/bin/sh\n").expect("foreign run script");
    fs::create_dir_all(manager.artifact_root()).expect("service root");
    symlink(&foreign_service, &service).expect("service symlink");

    assert_eq!(manager.enabled_by_artifacts(), Some(false));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn runit_enabled_state_rejects_down_symlink() {
    let root = test_root("runit-down-symlink");
    let manager = ServiceManager::runit_user(root.join("service"));
    let service = manager.artifact_root().join(UNIXNOTIS_DAEMON_RUNIT_SERVICE);
    fs::create_dir_all(service.join("env")).expect("env dir");
    fs::write(service.join("run"), "#!/bin/sh\n").expect("run script");
    symlink("missing-target", service.join("down")).expect("down symlink");

    assert_eq!(manager.enabled_by_artifacts(), Some(false));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn runit_enabled_state_requires_managed_marker() {
    let root = test_root("runit-managed-marker");
    let manager = ServiceManager::runit_user(root.join("service"));
    let service = manager.artifact_root().join(UNIXNOTIS_DAEMON_RUNIT_SERVICE);
    fs::create_dir_all(service.join("env")).expect("env dir");
    fs::write(service.join("run"), "#!/bin/sh\n").expect("run script");

    assert_eq!(manager.enabled_by_artifacts(), Some(false));

    fs::write(service.join(".unixnotis-managed"), "unixnotis\n").expect("marker");

    assert_eq!(manager.enabled_by_artifacts(), Some(true));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn runit_backend_hyprland_startup_lines_update_envdir_and_restart() {
    let manager = ServiceManager::runit_user(PathBuf::from("/tmp/service root"));
    let vars = ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "PATH"];

    let commands = manager.hyprland_startup_commands(&vars);

    assert_eq!(commands.len(), 1);
    assert_eq!(
        commands[0],
        "noticenterctl doctor repair-session --service-manager runit"
    );
    assert!(!commands[0].contains("sh -lc"));
}

#[test]
fn runit_backend_escapes_run_script_command_path_with_quotes() {
    let manager = ServiceManager::runit_user(PathBuf::from("/tmp/service"));
    let artifacts = manager.artifacts(Path::new("/tmp/bin dir/quote'and\\slash"));
    let run = artifacts
        .iter()
        .find(|artifact| artifact.path == Path::new("/tmp/service/unixnotis-daemon/run"))
        .expect("runit run script should exist");

    assert_eq!(
        run.contents
            .as_ref()
            .expect("runit run script should render contents"),
        "#!/bin/sh\n\
         PATH='/usr/local/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin'; export PATH\n\
         exec chpst -e ./env '/tmp/bin dir/quote'\\''and\\slash/unixnotis-daemon'\n"
    );
}

#[test]
fn runit_readiness_rejects_chpst_that_exists_only_on_path() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("runit-path-only-chpst");
    let path_bin = root.join("path-bin");
    let trusted_bin = root.join("trusted-bin");
    fs::create_dir_all(&path_bin).expect("path bin");
    fs::create_dir_all(&trusted_bin).expect("trusted bin");
    write_executable(&path_bin.join("chpst"), "#!/bin/sh\nexit 0\n");
    let _path = EnvPathGuard::prepend(&path_bin);
    let _tools = use_fake_tool_bin(&trusted_bin);

    let issues = ServiceManager::runit_user(root.join("service")).readiness_issues();

    assert!(issues
        .iter()
        .any(|issue| issue.message().contains("chpst not found")));

    let _ = fs::remove_dir_all(root);
}

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

fn write_executable(path: &Path, contents: &str) {
    write_test_executable(path, contents);
}

struct EnvPathGuard {
    previous: Option<std::ffi::OsString>,
}

impl EnvPathGuard {
    fn prepend(path: &Path) -> Self {
        let previous = std::env::var_os("PATH");
        let old_path = previous.clone().unwrap_or_default();
        let new_path = format!("{}:{}", path.display(), old_path.to_string_lossy());
        std::env::set_var("PATH", new_path);
        Self { previous }
    }
}

impl Drop for EnvPathGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
    }
}
