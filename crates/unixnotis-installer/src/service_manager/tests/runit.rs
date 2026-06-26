use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::service_manager::{ServiceArtifactKind, ServiceManager, UNIXNOTIS_DAEMON_RUNIT_SERVICE};

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

    let availability = manager
        .availability_command()
        .expect("runit checks sv availability");
    assert_eq!(availability.program(), "sv");
    assert_eq!(availability.args(), &["-V"]);

    // A watched service directory is the enablement source, not an sv query
    assert!(manager.is_enabled_command().is_none());
    assert!(manager.reload_after_artifact_change().is_none());

    // sv check tracks the requested state, so active status must parse sv status output
    let active = manager
        .active_probe()
        .expect("runit can parse current status");
    assert_eq!(active.command().args(), &["status", service_path]);
    assert_eq!(
        active.parser_matches("run: /tmp/service/unixnotis-daemon: (pid 123) 2s"),
        Some(true)
    );
    assert_eq!(
        active.parser_matches("down: /tmp/service/unixnotis-daemon: 1s"),
        Some(false)
    );

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
    let names = ["WAYLAND_DISPLAY", "DISPLAY", "XDG_RUNTIME_DIR", "PATH"];
    let vars = [
        ("WAYLAND_DISPLAY", "wayland-1\nignored".to_string()),
        ("XDG_RUNTIME_DIR", "/run/user/1000\t ".to_string()),
    ];

    // runit has no manager environment import command, so sync is pure envdir artifacts
    let commands = manager.environment_sync_commands(&vars, true);
    let artifacts = manager.environment_sync_artifacts(&names, &vars);

    assert!(commands.is_empty());
    assert_eq!(artifacts.len(), 4);
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
    assert!(commands[0].starts_with("sh -lc "));
    assert!(!commands[0].contains('\n'));
    assert!(commands[0].contains("umask 077"));
    assert!(commands[0].contains("[ ! -L \"$envdir\" ] || exit 1"));
    assert!(commands[0].contains("mkdir -p \"$envdir\" || exit 1"));
    assert!(commands[0].contains("[ -d \"$envdir\" ] && [ ! -L \"$envdir\" ] || exit 1"));
    assert!(commands[0].contains("/tmp/service root/unixnotis-daemon/env"));
    assert!(commands[0].contains("mktemp \"$envdir/.WAYLAND_DISPLAY.XXXXXX\""));
    assert!(commands[0].contains("printenv WAYLAND_DISPLAY > \"$tmp\" || : > \"$tmp\""));
    assert!(commands[0].contains("chmod 600 \"$tmp\""));
    assert!(commands[0].contains("mv -f \"$tmp\" \"$envdir/WAYLAND_DISPLAY\""));
    assert!(commands[0].contains("\"$envdir/WAYLAND_DISPLAY\""));
    assert!(!commands[0].contains(".PATH.XXXXXX"));
    assert!(!commands[0].contains("$envdir/PATH"));
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
        "#!/bin/sh\n\
         PATH='/usr/local/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin'; export PATH\n\
         exec chpst -e ./env '/tmp/bin dir/quote'\\''and\\slash/unixnotis-daemon'\n"
    );
}

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
