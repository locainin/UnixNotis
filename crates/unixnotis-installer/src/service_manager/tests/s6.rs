use std::fs;
use std::path::{Path, PathBuf};

use crate::service_manager::{
    ReadinessIssue, ServiceArtifactKind, ServiceManager, UNIXNOTIS_DAEMON_S6_SERVICE,
};

#[test]
fn s6_backend_renders_service_source_and_default_bundle_member() {
    let manager = ServiceManager::s6_user(
        PathBuf::from("/tmp/s6-data"),
        PathBuf::from("/run/user/s6-rc"),
    );
    let artifacts = manager.artifacts(Path::new("/tmp/bin"));

    assert_eq!(artifacts.len(), 4);
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
        PathBuf::from("/tmp/s6-data/sv/default/contents.d/unixnotis-daemon")
    );
    assert_eq!(artifacts[3].contents.as_deref(), Some(""));
}

#[test]
fn s6_backend_commands_match_expected_behavior() {
    let manager = ServiceManager::s6_user(
        PathBuf::from("/tmp/s6-data"),
        PathBuf::from("/run/user/s6-rc"),
    );

    assert!(manager.availability_command().is_none());
    assert!(manager.is_enabled_command().is_none());
    assert_eq!(
        manager
            .reload_after_artifact_change()
            .expect("s6 reload command")
            .args(),
        &["-u"]
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

    assert_eq!(active.parser_matches("true\n"), Some(true));
    assert_eq!(active.parser_matches("false\n"), Some(false));
}

#[test]
fn s6_backend_environment_sync_uses_envdir_artifacts() {
    let manager = ServiceManager::s6_user(
        PathBuf::from("/tmp/s6-data"),
        PathBuf::from("/run/user/s6-rc"),
    );
    let names = ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "PATH"];
    let vars = [
        ("WAYLAND_DISPLAY", "wayland-1\nignored".to_string()),
        ("XDG_RUNTIME_DIR", "/run/user/1000\t ".to_string()),
    ];

    let artifacts = manager.environment_sync_artifacts(&names, &vars);

    assert_eq!(artifacts.len(), 3);
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
    assert!(!artifacts
        .iter()
        .any(|artifact| artifact.path.ends_with("PATH")));
}

#[test]
fn s6_backend_hyprland_startup_lines_update_envdir_and_reload_database() {
    let manager = ServiceManager::s6_user(
        PathBuf::from("/tmp/s6 data"),
        PathBuf::from("/run/user/s6 rc"),
    );
    let vars = ["WAYLAND_DISPLAY", "PATH"];

    let commands = manager.hyprland_startup_commands(&vars);

    assert_eq!(commands.len(), 1);
    assert!(commands[0].starts_with("sh -lc "));
    assert!(commands[0].contains("[ ! -L \"$envdir\" ] || exit 1"));
    assert!(commands[0].contains("mkdir -p \"$envdir\" || exit 1"));
    assert!(commands[0].contains("mktemp \"$envdir/.WAYLAND_DISPLAY.XXXXXX\""));
    assert!(!commands[0].contains(".PATH.XXXXXX"));
    assert!(commands[0].contains("s6-db-reload -u || exit 1"));
    assert!(commands[0].contains("s6-rc -l "));
    assert!(commands[0].contains("/run/user/s6 rc"));
    assert!(commands[0].contains("-u change"));
    assert!(commands[0].contains("unixnotis-daemon"));
    assert!(commands[0].contains("s6-svc -r "));
    assert!(commands[0].contains("/run/user/s6 rc/servicedirs/unixnotis-daemon"));
}

#[test]
fn s6_readiness_errors_when_default_bundle_type_is_missing() {
    let root = test_root("s6-missing-default-type");
    let live = root.join("run").join("s6-rc");
    fs::create_dir_all(&live).expect("live dir");
    let manager = ServiceManager::s6_user(root.join("s6"), live);

    let issues = manager.readiness_issues();

    assert!(issues.iter().any(|issue| {
        matches!(issue, ReadinessIssue::Error(_))
            && issue.message().contains("default bundle type file")
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

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
