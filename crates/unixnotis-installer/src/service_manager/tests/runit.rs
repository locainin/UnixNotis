use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::service_manager::{ServiceArtifactKind, ServiceManager, UNIXNOTIS_DAEMON_RUNIT_SERVICE};

#[test]
fn runit_backend_renders_service_directory_and_run_script() {
    let manager = ServiceManager::runit_user(PathBuf::from("/tmp/service"));
    let artifacts = manager.artifacts(Path::new("/tmp/bin"));

    assert_eq!(artifacts.len(), 3);
    assert_eq!(
        artifacts[0].path,
        PathBuf::from("/tmp/service").join(UNIXNOTIS_DAEMON_RUNIT_SERVICE)
    );
    assert_eq!(artifacts[0].kind, ServiceArtifactKind::Directory);
    assert_eq!(
        artifacts[1].path,
        PathBuf::from("/tmp/service/unixnotis-daemon/env")
    );
    assert_eq!(artifacts[1].kind, ServiceArtifactKind::Directory);
    assert_eq!(
        artifacts[2].path,
        PathBuf::from("/tmp/service/unixnotis-daemon/run")
    );
    assert_eq!(artifacts[2].kind, ServiceArtifactKind::ExecutableFile);
    assert_eq!(artifacts[2].mode, Some(0o755));
    assert_eq!(
        artifacts[2]
            .contents
            .as_ref()
            .expect("runit run script should render contents"),
        "#!/bin/sh\nexec chpst -e ./env '/tmp/bin/unixnotis-daemon'\n"
    );
}

#[test]
fn runit_backend_commands_match_expected_behavior() {
    let manager = ServiceManager::runit_user(PathBuf::from("/tmp/service"));
    let service_path = "/tmp/service/unixnotis-daemon";

    let availability = manager
        .availability_command()
        .expect("runit checks sv availability");
    assert_eq!(availability.program(), "sv");
    assert_eq!(availability.args(), &["-V"]);

    assert!(manager.is_enabled_command().is_none());
    assert!(manager.reload_after_artifact_change().is_none());

    let active = manager
        .is_active_command()
        .expect("runit can check requested state");
    assert_eq!(active.args(), &["check", service_path]);

    let enable = manager
        .enable_now_command()
        .expect("runit starts watched service directories");
    assert_eq!(enable.args(), &["start", service_path]);

    let disable = manager
        .disable_now_command()
        .expect("runit stops watched service directories");
    assert_eq!(disable.args(), &["stop", service_path]);

    let stop = manager
        .stop_for_reinstall_command()
        .expect("runit can stop before reinstall");
    assert_eq!(stop.args(), &["stop", service_path]);
}

#[test]
fn runit_backend_environment_sync_uses_envdir_artifacts() {
    let manager = ServiceManager::runit_user(PathBuf::from("/tmp/service"));
    let vars = [
        ("WAYLAND_DISPLAY", "wayland-1\nignored".to_string()),
        ("XDG_RUNTIME_DIR", "/run/user/1000\t ".to_string()),
    ];

    let commands = manager.environment_sync_commands(&vars, true);
    let artifacts = manager.environment_sync_artifacts(&vars);

    assert!(commands.is_empty());
    assert_eq!(artifacts.len(), 3);
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
        PathBuf::from("/tmp/service/unixnotis-daemon/env/XDG_RUNTIME_DIR")
    );
    assert_eq!(artifacts[2].contents.as_deref(), Some("/run/user/1000\n"));
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
fn runit_backend_hyprland_startup_lines_update_envdir_and_restart() {
    let manager = ServiceManager::runit_user(PathBuf::from("/tmp/service root"));
    let vars = ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"];

    let commands = manager.hyprland_startup_commands(&vars);

    assert_eq!(commands.len(), 1);
    assert!(commands[0].starts_with("sh -lc "));
    assert!(!commands[0].contains('\n'));
    assert!(commands[0].contains("mkdir -p"));
    assert!(commands[0].contains("/tmp/service root/unixnotis-daemon/env"));
    assert!(commands[0].contains("printenv WAYLAND_DISPLAY >"));
    assert!(commands[0].contains("/tmp/service root/unixnotis-daemon/env/WAYLAND_DISPLAY"));
    assert!(commands[0].contains("sv restart"));
    assert!(commands[0].contains("|| sv start"));
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
        "#!/bin/sh\nexec chpst -e ./env '/tmp/bin dir/quote'\\''and\\slash/unixnotis-daemon'\n"
    );
}

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
