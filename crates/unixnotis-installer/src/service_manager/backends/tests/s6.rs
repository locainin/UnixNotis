use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

use crate::service_manager::contract::MANAGED_DIRECTORY_MARKER;
use crate::service_manager::{
    ReadinessIssue, ServiceArtifactKind, ServiceArtifactRefresh, ServiceManager,
};
use crate::system_tools::routing::use_fake_tool_bin;
use crate::test_support::fs::write_executable as write_test_executable;

use super::super::s6::SERVICE_NAME as UNIXNOTIS_DAEMON_S6_SERVICE;

#[test]
fn s6_backend_renders_service_source_and_default_bundle_member() {
    let manager = ServiceManager::s6_user(
        PathBuf::from("/tmp/s6-data"),
        PathBuf::from("/run/user/s6-rc"),
    );
    let artifacts = manager.artifacts(Path::new("/tmp/bin"));

    // s6-rc source state includes the longrun, run script, and default bundle membership
    assert_eq!(artifacts.len(), 5);
    assert_eq!(
        artifacts[0].path,
        PathBuf::from("/tmp/s6-data/sv/unixnotis-daemon")
    );
    assert_eq!(artifacts[0].kind, ServiceArtifactKind::ManagedDirectory);
    assert_eq!(
        artifacts[1].path,
        PathBuf::from("/tmp/s6-data/sv/unixnotis-daemon/type")
    );
    assert_eq!(artifacts[1].contents.as_deref(), Some("longrun\n"));
    assert_eq!(
        artifacts[2].path,
        PathBuf::from("/tmp/s6-data/sv/unixnotis-daemon/run")
    );
    assert_eq!(artifacts[2].kind, ServiceArtifactKind::ExecutableFile);
    assert_eq!(
        artifacts[2].contents.as_deref(),
        Some(
            "#!/bin/sh\n\
             PATH='/usr/local/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin'; export PATH\n\
             exec s6-envdir ./env '/tmp/bin/unixnotis-daemon'\n"
        )
    );
    assert_eq!(
        artifacts[3].path,
        PathBuf::from("/tmp/s6-data/sv/default/type")
    );
    assert_eq!(
        artifacts[3].kind,
        ServiceArtifactKind::SharedFile {
            created_marker: Some(PathBuf::from(
                "/tmp/s6-data/sv/default/.unixnotis-created-type"
            ))
        }
    );
    assert_eq!(artifacts[3].contents.as_deref(), Some("bundle\n"));
    assert_eq!(
        artifacts[4].path,
        PathBuf::from("/tmp/s6-data/sv/default/contents.d/unixnotis-daemon")
    );
    assert_eq!(artifacts[4].contents.as_deref(), Some(""));
}

#[test]
fn s6_backend_commands_match_expected_behavior() {
    let manager = ServiceManager::s6_user(
        PathBuf::from("/tmp/s6-data"),
        PathBuf::from("/run/user/s6-rc"),
    );

    // Readiness checks own tool validation because availability needs several s6 programs
    assert!(manager.availability_command().is_none());
    assert!(manager.is_enabled_command().is_none());
    // Database refresh compiles the user source tree before s6-rc can change the live service
    let Some(ServiceArtifactRefresh::S6Database(refresh)) = manager.refresh_after_artifact_change()
    else {
        panic!("s6 should refresh through a database plan");
    };
    assert_eq!(refresh.source_root(), PathBuf::from("/tmp/s6-data/sv"));
    assert_eq!(refresh.rc_root(), PathBuf::from("/tmp/s6-data/rc"));
    assert_eq!(
        refresh
            .compile_command(Path::new("/tmp/s6-data/rc/compiled-next"))
            .args(),
        &["/tmp/s6-data/rc/compiled-next", "/tmp/s6-data/sv"]
    );
    assert_eq!(
        refresh
            .update_command(Path::new("/tmp/s6-data/rc/compiled-next"))
            .args(),
        &["-l", "/run/user/s6-rc", "/tmp/s6-data/rc/compiled-next"]
    );
    assert_eq!(
        manager.start_command().expect("s6 start command").args(),
        &[
            "-l",
            "/run/user/s6-rc",
            "-u",
            "change",
            UNIXNOTIS_DAEMON_S6_SERVICE
        ]
    );
    assert_eq!(
        manager
            .disable_now_command()
            .expect("s6 stop command")
            .args(),
        &[
            "-l",
            "/run/user/s6-rc",
            "-d",
            "change",
            UNIXNOTIS_DAEMON_S6_SERVICE
        ]
    );
}

#[test]
fn s6_backend_active_probe_parses_s6_svstat_output() {
    let manager = ServiceManager::s6_user(
        PathBuf::from("/tmp/s6-data"),
        PathBuf::from("/run/user/s6-rc"),
    );
    let active = manager.active_probe().expect("s6 active probe");

    // s6-svstat -o up prints a boolean, so parsing stays exact and cheap
    assert_eq!(active.parser_matches("true\n"), Some(true));
    assert_eq!(active.parser_matches("false\n"), Some(false));
}

#[test]
fn s6_backend_environment_sync_uses_envdir_artifacts() {
    let manager = ServiceManager::s6_user(
        PathBuf::from("/tmp/s6-data"),
        PathBuf::from("/run/user/s6-rc"),
    );
    let names = [
        "WAYLAND_DISPLAY",
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

    // s6-envdir reads files at service start, so sync writes artifact files instead of commands
    let artifacts = manager.environment_sync_artifacts(&names, &vars);

    assert_eq!(artifacts.len(), 4);
    assert_eq!(
        artifacts[0].path,
        PathBuf::from("/tmp/s6-data/sv/unixnotis-daemon/env")
    );
    assert_eq!(
        artifacts[1].path,
        PathBuf::from("/tmp/s6-data/sv/unixnotis-daemon/env/WAYLAND_DISPLAY")
    );
    assert_eq!(artifacts[1].contents.as_deref(), Some("wayland-1\n"));
    assert_eq!(
        artifacts[2].path,
        PathBuf::from("/tmp/s6-data/sv/unixnotis-daemon/env/XDG_RUNTIME_DIR")
    );
    assert_eq!(artifacts[2].contents.as_deref(), Some("/run/user/1000\n"));
    assert_eq!(
        artifacts[3].path,
        PathBuf::from("/tmp/s6-data/sv/unixnotis-daemon/env/DBUS_SESSION_BUS_ADDRESS")
    );
    assert_eq!(
        artifacts[3].contents.as_deref(),
        Some("unix:path=/tmp/unixnotis-bus\n")
    );
    // PATH is excluded for the same reason as runit: the run script fixes lookup first
    assert!(!artifacts
        .iter()
        .any(|artifact| artifact.path.ends_with("PATH")));
}

#[test]
fn s6_backend_hyprland_startup_lines_update_envdir_and_start_service() {
    let manager = ServiceManager::s6_user(
        PathBuf::from("/tmp/s6 data"),
        PathBuf::from("/run/user/s6 rc"),
    );
    let vars = ["WAYLAND_DISPLAY", "PATH"];

    let commands = manager.hyprland_startup_commands(&vars);

    // Hyprland receives one shell line because it does not manage multi-step service hooks
    assert_eq!(commands.len(), 1);
    assert!(commands[0].starts_with("sh -lc "));
    assert!(commands[0].contains("[ ! -L \"$envdir\" ] || exit 1"));
    assert!(commands[0].contains("mkdir -p \"$envdir\" || exit 1"));
    assert!(commands[0].contains("mktemp \"$envdir/.WAYLAND_DISPLAY.XXXXXX\""));
    assert!(!commands[0].contains(".PATH.XXXXXX"));
    assert!(!commands[0].contains("s6-db-reload"));
    assert!(!commands[0].contains("s6-rc-compile"));
    assert!(commands[0].contains("s6-rc -l "));
    assert!(commands[0].contains("/run/user/s6 rc"));
    assert!(commands[0].contains("-u change"));
    assert!(commands[0].contains("unixnotis-daemon"));
    assert!(commands[0].contains("s6-svc -r "));
    assert!(commands[0].contains("/run/user/s6 rc/servicedirs/unixnotis-daemon"));
}

#[test]
fn s6_readiness_warns_when_default_bundle_type_can_be_initialized() {
    let root = test_root("s6-missing-default-type");
    let live = root.join("run").join("s6-rc");
    fs::create_dir_all(&live).expect("live dir");
    let manager = ServiceManager::s6_user(root.join("s6"), live);

    let issues = manager.readiness_issues();

    assert!(issues.iter().any(|issue| {
        matches!(issue, ReadinessIssue::Warning(_))
            && issue.message().contains("default bundle is missing")
    }));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn s6_readiness_errors_when_default_bundle_type_is_not_bundle() {
    let root = test_root("s6-invalid-default-type");
    let live = root.join("run").join("s6-rc");
    fs::create_dir_all(&live).expect("live dir");
    let default_dir = root.join("s6").join("sv").join("default");
    fs::create_dir_all(&default_dir).expect("default bundle dir");
    fs::write(default_dir.join("type"), "longrun\n").expect("invalid default type");
    let manager = ServiceManager::s6_user(root.join("s6"), live);

    let issues = manager.readiness_issues();

    assert!(issues.iter().any(|issue| {
        matches!(issue, ReadinessIssue::Error(_))
            && issue.message().contains("refusing to overwrite")
    }));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn s6_readiness_errors_when_live_directory_is_missing() {
    let root = test_root("s6-missing-live");
    let default_dir = root.join("s6").join("sv").join("default");
    fs::create_dir_all(&default_dir).expect("default bundle dir");
    fs::write(default_dir.join("type"), "bundle\n").expect("default bundle type");
    let manager = ServiceManager::s6_user(root.join("s6"), root.join("run").join("s6-rc"));

    let issues = manager.readiness_issues();

    assert!(issues.iter().any(|issue| {
        matches!(issue, ReadinessIssue::Error(_)) && issue.message().contains("live directory")
    }));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn s6_readiness_accepts_symlinked_live_directory() {
    let root = test_root("s6-symlink-live");
    let real_live = root.join("real-live");
    let linked_live = root.join("run").join("s6-rc");
    let default_dir = root.join("s6").join("sv").join("default");
    fs::create_dir_all(&real_live).expect("real live dir");
    fs::create_dir_all(linked_live.parent().expect("linked live parent")).expect("run dir");
    unix_fs::symlink(&real_live, &linked_live).expect("live symlink");
    fs::create_dir_all(&default_dir).expect("default bundle dir");
    fs::write(default_dir.join("type"), "bundle\n").expect("default bundle type");
    let manager = ServiceManager::s6_user(root.join("s6"), linked_live);

    let issues = manager.readiness_issues();

    // s6-rc-init rotates the real live tree under a stable symlink, so this is valid
    assert!(!issues.iter().any(|issue| {
        matches!(issue, ReadinessIssue::Error(_)) && issue.message().contains("live directory")
    }));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn s6_readiness_rejects_tools_that_exist_only_on_path() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("s6-path-only-tools");
    let path_bin = root.join("path-bin");
    let trusted_bin = root.join("trusted-bin");
    let data = root.join("s6");
    let live = root.join("run").join("s6-rc");
    fs::create_dir_all(&path_bin).expect("path bin");
    fs::create_dir_all(&trusted_bin).expect("trusted bin");
    fs::create_dir_all(&live).expect("live dir");
    fs::create_dir_all(data.join("sv").join("default")).expect("default dir");
    fs::write(data.join("sv").join("default").join("type"), "bundle\n").expect("default type");
    for tool in [
        "s6-rc-compile",
        "s6-rc-update",
        "s6-rc",
        "s6-envdir",
        "s6-svstat",
    ] {
        write_executable(path_bin.join(tool), "#!/bin/sh\nexit 0\n");
    }
    let _path = EnvPathGuard::prepend(&path_bin);
    let _tools = use_fake_tool_bin(&trusted_bin);

    let issues = ServiceManager::s6_user(data, live).readiness_issues();

    assert!(issues
        .iter()
        .any(|issue| issue.message().contains("s6-rc not found")));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn s6_enabled_state_requires_every_source_artifact() {
    let root = test_root("s6-enabled-layout");
    let data = root.join("s6");
    let live = root.join("run").join("s6-rc");
    let manager = ServiceManager::s6_user(data.clone(), live);
    let service = data.join("sv").join("unixnotis-daemon");
    let default = data.join("sv").join("default");

    assert_eq!(manager.enabled_by_artifacts(), Some(false));

    fs::create_dir_all(&service).expect("service dir");
    fs::write(service.join(MANAGED_DIRECTORY_MARKER), "unixnotis\n").expect("marker");
    fs::write(service.join("type"), "longrun\n").expect("type");
    fs::write(service.join("run"), "#!/bin/sh\n").expect("run");
    fs::create_dir_all(default.join("contents.d")).expect("contents dir");
    fs::write(default.join("type"), "bundle\n").expect("default type");

    // Missing bundle membership means the service is not part of the default graph
    assert_eq!(manager.enabled_by_artifacts(), Some(false));

    fs::write(default.join("contents.d").join("unixnotis-daemon"), "").expect("member");

    assert_eq!(manager.enabled_by_artifacts(), Some(true));

    fs::write(default.join("type"), "longrun\n").expect("wrong default type");

    // A default bundle with the wrong type is not valid enablement
    assert_eq!(manager.enabled_by_artifacts(), Some(false));

    fs::write(default.join("type"), "bundle\n").expect("restore default type");
    fs::remove_file(service.join("run")).expect("remove run");
    fs::create_dir(service.join("run")).expect("directory at run path");

    // The run artifact must be a regular file, not any existing path
    assert_eq!(manager.enabled_by_artifacts(), Some(false));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn s6_environment_sync_rejects_unsafe_env_names_and_empty_names() {
    let manager = ServiceManager::s6_user(
        PathBuf::from("/tmp/s6-data"),
        PathBuf::from("/run/user/s6-rc"),
    );
    let names = ["WAYLAND_DISPLAY", "", "BAD-NAME", "1BAD", "_SAFE"];
    let vars = [
        ("WAYLAND_DISPLAY", "wayland-1".to_string()),
        ("_SAFE", "yes".to_string()),
        ("BAD-NAME", "no".to_string()),
    ];

    let artifacts = manager.environment_sync_artifacts(&names, &vars);

    // Envdir file names are shell-variable-shaped so generated shell cannot escape the envdir
    assert_eq!(artifacts.len(), 3);
    assert!(artifacts
        .iter()
        .any(|artifact| artifact.path.ends_with("WAYLAND_DISPLAY")));
    assert!(artifacts
        .iter()
        .any(|artifact| artifact.path.ends_with("_SAFE")));
    assert!(!artifacts
        .iter()
        .any(|artifact| artifact.path.ends_with("BAD-NAME")));
    assert!(!artifacts
        .iter()
        .any(|artifact| artifact.path.ends_with("1BAD")));
}

#[test]
fn s6_active_probe_rejects_truthy_but_non_exact_output() {
    let manager = ServiceManager::s6_user(
        PathBuf::from("/tmp/s6-data"),
        PathBuf::from("/run/user/s6-rc"),
    );
    let active = manager.active_probe().expect("s6 active probe");

    // s6-svstat -o up emits exact true/false, so loose text must not count as active
    assert_eq!(active.parser_matches(" true\n"), Some(true));
    assert_eq!(active.parser_matches("true enough\n"), Some(false));
    assert_eq!(active.parser_matches("1\n"), Some(false));
}

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

fn write_executable(path: PathBuf, contents: &str) {
    write_test_executable(&path, contents);
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
