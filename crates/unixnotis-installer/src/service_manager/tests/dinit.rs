use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::service_manager::{
    ReadinessIssue, ServiceArtifactKind, ServiceManager, UNIXNOTIS_DAEMON_DINIT_SERVICE,
};

#[test]
fn dinit_backend_renders_exact_service_artifact() {
    let manager = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit.d"));
    let artifacts = manager.artifacts(std::path::Path::new("/tmp/bin"));
    let service = artifacts
        .iter()
        .find(|artifact| artifact.path == Path::new("/tmp/dinit.d/unixnotis-daemon"))
        .expect("dinit service artifact should exist");

    assert_eq!(service.kind, ServiceArtifactKind::File);
    assert_eq!(
        service
            .contents
            .as_ref()
            .expect("dinit service should render contents"),
        "type = process\n\
         command = /tmp/bin/unixnotis-daemon\n\
         restart = on-failure\n"
    );
}

#[test]
fn dinit_backend_renders_boot_dependency_artifacts() {
    let manager = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit.d"));
    let artifacts = manager.artifacts(std::path::Path::new("/tmp/bin"));

    assert_eq!(artifacts.len(), 3);
    assert_eq!(artifacts[0].path, PathBuf::from("/tmp/dinit.d/boot.d"));
    assert_eq!(artifacts[0].kind, ServiceArtifactKind::Directory);
    assert_eq!(
        artifacts[2].path,
        PathBuf::from("/tmp/dinit.d/boot.d/unixnotis-daemon")
    );
    assert_eq!(
        artifacts[2].kind,
        ServiceArtifactKind::Symlink {
            target: PathBuf::from("../unixnotis-daemon"),
        }
    );
}

#[test]
fn dinit_backend_commands_match_expected_behavior() {
    let manager = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit.d"));

    let availability = manager
        .availability_command()
        .expect("dinit has an availability command");
    assert_eq!(availability.program(), "dinitctl");
    assert_eq!(availability.args(), &["--user", "--quiet", "list"]);

    assert!(manager.is_enabled_command().is_none());

    let active = manager
        .active_probe()
        .expect("dinit has an active-state command");
    assert_eq!(
        active.command().args(),
        &[
            "--user",
            "--quiet",
            "is-started",
            UNIXNOTIS_DAEMON_DINIT_SERVICE
        ]
    );

    assert!(manager.reload_after_artifact_change().is_none());

    let enable = manager
        .enable_now_command()
        .expect("dinit starts after artifacts handle persistence");
    assert_eq!(
        enable.args(),
        &["--user", "start", UNIXNOTIS_DAEMON_DINIT_SERVICE]
    );

    let disable = manager
        .disable_now_command()
        .expect("dinit can stop during uninstall");
    assert_eq!(
        disable.args(),
        &[
            "--user",
            "stop",
            "--ignore-unstarted",
            UNIXNOTIS_DAEMON_DINIT_SERVICE,
        ]
    );
}

#[test]
fn dinit_first_install_does_not_require_loaded_service_reload() {
    let manager = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit.d"));

    assert!(manager.reload_after_artifact_change().is_none());
}

#[test]
fn dinit_enabled_state_uses_boot_symlink_artifact() {
    let root = test_root("dinit-enabled-artifacts");
    let manager = ServiceManager::dinit_user(root.join("dinit.d"));
    let service = manager.artifact_root().join(UNIXNOTIS_DAEMON_DINIT_SERVICE);
    let boot_dir = manager.artifact_root().join("boot.d");
    let boot_link = boot_dir.join(UNIXNOTIS_DAEMON_DINIT_SERVICE);
    fs::create_dir_all(&boot_dir).expect("boot dir");
    fs::write(&service, "type = process\n").expect("service file");
    symlink("../unixnotis-daemon", &boot_link).expect("boot symlink");

    assert_eq!(manager.enabled_by_artifacts(), Some(true));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn dinit_enabled_state_rejects_wrong_boot_symlink_target() {
    let root = test_root("dinit-wrong-boot-link");
    let manager = ServiceManager::dinit_user(root.join("dinit.d"));
    let service = manager.artifact_root().join(UNIXNOTIS_DAEMON_DINIT_SERVICE);
    let boot_dir = manager.artifact_root().join("boot.d");
    let boot_link = boot_dir.join(UNIXNOTIS_DAEMON_DINIT_SERVICE);
    fs::create_dir_all(&boot_dir).expect("boot dir");
    fs::write(&service, "type = process\n").expect("service file");
    symlink("../other-service", &boot_link).expect("boot symlink");

    assert_eq!(manager.enabled_by_artifacts(), Some(false));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn dinit_enabled_state_rejects_symlink_service_file() {
    let root = test_root("dinit-symlink-service-file");
    let manager = ServiceManager::dinit_user(root.join("dinit.d"));
    let service = manager.artifact_root().join(UNIXNOTIS_DAEMON_DINIT_SERVICE);
    let foreign_service = root.join("foreign-service");
    let boot_dir = manager.artifact_root().join("boot.d");
    let boot_link = boot_dir.join(UNIXNOTIS_DAEMON_DINIT_SERVICE);
    fs::create_dir_all(&boot_dir).expect("boot dir");
    fs::write(&foreign_service, "type = process\n").expect("foreign service file");
    symlink(&foreign_service, &service).expect("service symlink");
    symlink("../unixnotis-daemon", &boot_link).expect("boot symlink");

    assert_eq!(manager.enabled_by_artifacts(), Some(false));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn dinit_warns_when_boot_service_missing_waits_for_boot_d() {
    let root = test_root("dinit-missing-boot-waits-for");
    let manager = ServiceManager::dinit_user(root.join("dinit.d"));
    fs::create_dir_all(manager.artifact_root()).expect("dinit dir");
    fs::write(
        manager.artifact_root().join("boot"),
        "type = internal\nwaits-for.d: other.d\n",
    )
    .expect("boot service");

    let issues = manager.readiness_issues();

    assert_eq!(issues.len(), 1);
    assert!(matches!(issues[0], ReadinessIssue::Warning(_)));
    assert!(issues[0].message().contains("waits-for.d: boot.d"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn dinit_backend_environment_sync_uses_setenv() {
    let manager = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit.d"));
    let vars = [
        ("WAYLAND_DISPLAY", "wayland-1".to_string()),
        ("XDG_RUNTIME_DIR", "/run/user/1000".to_string()),
    ];

    let commands = manager.environment_sync_commands(&vars, true);

    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].program(), "dinitctl");
    assert_eq!(
        commands[0].args(),
        &["--user", "setenv", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"]
    );
    assert_eq!(
        commands[0].envs(),
        &[
            ("WAYLAND_DISPLAY".to_string(), "wayland-1".to_string()),
            ("XDG_RUNTIME_DIR".to_string(), "/run/user/1000".to_string()),
        ]
    );
}

#[test]
fn dinit_boot_readiness_accepts_plus_equals_dependency_syntax() {
    let root = test_root("dinit-boot-plus-equals");
    let manager = ServiceManager::dinit_user(root.join("dinit.d"));
    fs::create_dir_all(manager.artifact_root()).expect("dinit dir");
    fs::write(
        manager.artifact_root().join("boot"),
        "type = internal\nwaits-for.d += boot.d\n",
    )
    .expect("boot service");

    assert!(manager.readiness_issues().is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn dinit_backend_hyprland_startup_lines_use_dinitctl() {
    let manager = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit.d"));
    let vars = ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"];

    let commands = manager.hyprland_startup_commands(&vars);

    assert_eq!(
        commands,
        vec![
            "dinitctl --user setenv WAYLAND_DISPLAY XDG_RUNTIME_DIR".to_string(),
            "dinitctl --user restart --ignore-unstarted unixnotis-daemon".to_string(),
            "dinitctl --user start unixnotis-daemon".to_string(),
        ]
    );
}

#[test]
fn dinit_backend_escapes_command_path_with_dollar() {
    let manager = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit.d"));
    let artifacts = manager.artifacts(std::path::Path::new("/tmp/price$HOME/bin"));
    let service = artifacts
        .iter()
        .find(|artifact| artifact.path == Path::new("/tmp/dinit.d/unixnotis-daemon"))
        .expect("dinit service artifact should exist");

    assert_eq!(
        service
            .contents
            .as_ref()
            .expect("dinit service should render contents"),
        "type = process\n\
         command = \"/tmp/price$$HOME/bin/unixnotis-daemon\"\n\
         restart = on-failure\n"
    );
}

#[test]
fn dinit_backend_escapes_command_path_with_spaces() {
    let manager = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit.d"));
    let artifacts = manager.artifacts(std::path::Path::new(
        "/tmp/dir with space/quote\"and\\slash",
    ));
    let service = artifacts
        .iter()
        .find(|artifact| artifact.path == Path::new("/tmp/dinit.d/unixnotis-daemon"))
        .expect("dinit service artifact should exist");

    assert_eq!(
        service
            .contents
            .as_ref()
            .expect("dinit service should render contents"),
        "type = process\n\
         command = \"/tmp/dir with space/quote\\\"and\\\\slash/unixnotis-daemon\"\n\
         restart = on-failure\n"
    );
}

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
