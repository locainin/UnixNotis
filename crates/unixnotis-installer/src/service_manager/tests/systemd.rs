use std::path::PathBuf;

use crate::service_manager::{ServiceArtifactKind, ServiceManager, UNIXNOTIS_DAEMON_SERVICE};

#[test]
fn systemd_backend_renders_exact_unit_artifact() {
    let manager = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd/user"));
    let artifacts = manager.artifacts(std::path::Path::new("/tmp/bin"));

    assert_eq!(artifacts.len(), 1);
    assert_eq!(
        artifacts[0].path,
        PathBuf::from("/tmp/systemd/user").join(UNIXNOTIS_DAEMON_SERVICE)
    );
    assert_eq!(artifacts[0].kind, ServiceArtifactKind::File);
    assert_eq!(
        artifacts[0]
            .contents
            .as_ref()
            .expect("systemd artifact should render contents"),
        "[Unit]\n\
         Description=UnixNotis Notification Daemon\n\
         After=graphical-session.target\n\
         Wants=graphical-session.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart=/tmp/bin/unixnotis-daemon\n\
         Restart=on-failure\n\
         RestartSec=1\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    );
}

#[test]
fn systemd_backend_commands_match_existing_behavior() {
    let manager = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd/user"));

    let availability = manager
        .availability_command()
        .expect("systemd has an availability command");
    assert_eq!(availability.program(), "systemctl");
    assert_eq!(
        availability.args(),
        &[
            "--user",
            "--no-pager",
            "--plain",
            "list-units",
            "--type=service"
        ]
    );

    let enabled = manager
        .is_enabled_command()
        .expect("systemd has an enabled-state command");
    assert_eq!(
        enabled.args(),
        &["--user", "is-enabled", "--quiet", UNIXNOTIS_DAEMON_SERVICE]
    );

    let active = manager
        .active_probe()
        .expect("systemd has an active-state command");
    assert_eq!(
        active.command().args(),
        &["--user", "is-active", "--quiet", UNIXNOTIS_DAEMON_SERVICE]
    );

    let reload = manager
        .reload_after_artifact_change()
        .expect("systemd reloads after unit changes");
    assert_eq!(reload.args(), &["--user", "daemon-reload"]);

    let enable = manager
        .enable_now_command()
        .expect("systemd can enable and start");
    assert_eq!(
        enable.args(),
        &["--user", "enable", "--now", UNIXNOTIS_DAEMON_SERVICE]
    );

    let start = manager.start_command().expect("systemd can start");
    assert_eq!(start.args(), &["--user", "start", UNIXNOTIS_DAEMON_SERVICE]);

    let disable = manager
        .disable_now_command()
        .expect("systemd can disable and stop");
    assert_eq!(
        disable.args(),
        &["--user", "disable", "--now", UNIXNOTIS_DAEMON_SERVICE]
    );

    let stop = manager
        .stop_for_reinstall_command()
        .expect("systemd can stop during reinstall");
    assert_eq!(
        stop.args(),
        &[
            "--user",
            "--job-mode=replace-irreversibly",
            "stop",
            UNIXNOTIS_DAEMON_SERVICE,
        ]
    );
}

#[test]
fn hyprland_startup_lines_come_from_selected_backend() {
    let manager = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd/user"));
    let vars = ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"];

    let commands = manager.hyprland_startup_commands(&vars);

    assert_eq!(
        commands,
        vec![
            "dbus-update-activation-environment WAYLAND_DISPLAY XDG_RUNTIME_DIR".to_string(),
            "systemctl --user import-environment WAYLAND_DISPLAY XDG_RUNTIME_DIR".to_string(),
            format!("systemctl --user --no-block restart {UNIXNOTIS_DAEMON_SERVICE}"),
        ]
    );
}

#[test]
fn environment_sync_commands_come_from_selected_backend() {
    let manager = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd/user"));
    let vars = [
        ("WAYLAND_DISPLAY", "wayland-1".to_string()),
        ("XDG_RUNTIME_DIR", "/run/user/1000".to_string()),
    ];

    let with_dbus = manager.environment_sync_commands(&vars, true);
    assert_eq!(with_dbus.len(), 2);
    assert_eq!(with_dbus[0].program(), "dbus-update-activation-environment");
    assert_eq!(with_dbus[0].args(), &["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"]);
    assert_eq!(with_dbus[1].program(), "systemctl");
    assert_eq!(
        with_dbus[1].args(),
        &[
            "--user",
            "--no-pager",
            "import-environment",
            "WAYLAND_DISPLAY",
            "XDG_RUNTIME_DIR",
        ]
    );

    let without_dbus = manager.environment_sync_commands(&vars, false);
    assert_eq!(without_dbus.len(), 1);
    assert_eq!(without_dbus[0].program(), "systemctl");
    assert_eq!(
        without_dbus[0].args(),
        &[
            "--user",
            "--no-pager",
            "import-environment",
            "WAYLAND_DISPLAY",
            "XDG_RUNTIME_DIR",
        ]
    );
}
