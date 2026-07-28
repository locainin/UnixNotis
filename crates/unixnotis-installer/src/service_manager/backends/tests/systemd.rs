use std::path::PathBuf;

use crate::service_manager::{ServiceArtifactKind, ServiceArtifactRefresh, ServiceManager};

use super::super::systemd::{CONTROL_ACTIVATION_SERVICE, SERVICE_NAME as UNIXNOTIS_DAEMON_SERVICE};

#[test]
fn systemd_backend_renders_exact_unit_artifact() {
    let manager = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd/user"));
    let artifacts = manager.artifacts(std::path::Path::new("/tmp/bin"));

    assert_eq!(artifacts.len(), 2);
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
         PartOf=graphical-session.target\n\
         \n\
         [Service]\n\
         Type=dbus\n\
         BusName=com.unixnotis.Control\n\
         ExecStart=/tmp/bin/unixnotis-daemon\n\
         Restart=on-failure\n\
         RestartSec=1\n\
         TimeoutStartSec=20\n\
         TimeoutStopSec=10\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    );
    assert_eq!(
        artifacts[1].path,
        PathBuf::from("/tmp")
            .join("share")
            .join("dbus-1")
            .join("services")
            .join(CONTROL_ACTIVATION_SERVICE)
    );
    assert_eq!(artifacts[1].kind, ServiceArtifactKind::File);
    assert_eq!(
        artifacts[1]
            .contents
            .as_ref()
            .expect("activation artifact should render contents"),
        "[D-BUS Service]\n\
         Name=com.unixnotis.Control\n\
         Exec=/tmp/bin/unixnotis-daemon\n\
         SystemdService=unixnotis-daemon.service\n"
    );
}

#[test]
fn systemd_backend_commands_match_existing_behavior() {
    let manager = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd/user"));

    // Availability should remain a read-only user-manager query
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

    // Enabled and active probes intentionally use quiet status checks for fast install-state reads
    let enabled = manager
        .is_enabled_command()
        .expect("systemd has an enabled-state command");
    assert_eq!(
        enabled.args(),
        &["--user", "is-enabled", "--quiet", UNIXNOTIS_DAEMON_SERVICE]
    );

    let active = manager.active_probe();
    assert_eq!(
        active.command().args(),
        &["--user", "is-active", "--quiet", UNIXNOTIS_DAEMON_SERVICE]
    );

    // Unit file changes still require daemon-reload before enable/start
    let Some(ServiceArtifactRefresh::Command(reload)) = manager.refresh_after_artifact_change()
    else {
        panic!("systemd should refresh through daemon-reload");
    };
    assert_eq!(reload.args(), &["--user", "daemon-reload"]);

    let enable = manager.enable_now_command();
    assert_eq!(
        enable.args(),
        &["--user", "enable", "--now", UNIXNOTIS_DAEMON_SERVICE]
    );

    let start = manager.start_command();
    assert_eq!(start.args(), &["--user", "start", UNIXNOTIS_DAEMON_SERVICE]);

    let disable = manager.disable_now_command();
    assert_eq!(
        disable.args(),
        &["--user", "disable", "--now", UNIXNOTIS_DAEMON_SERVICE]
    );

    // Reinstall should stop only UnixNotis and never broaden into user-session targets
    let stop = manager.stop_for_reinstall_command();
    assert_eq!(stop.args(), &["--user", "stop", UNIXNOTIS_DAEMON_SERVICE]);
}

#[test]
fn hyprland_startup_lines_come_from_selected_backend() {
    let manager = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd/user"));
    let vars = ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"];

    let commands = manager.hyprland_startup_commands(&vars);

    // Hyprland systemd lines stay explicit because they run from a login-session config file
    assert_eq!(
        commands,
        vec![
            "systemctl --user unset-environment DBUS_SESSION_BUS_ADDRESS".to_string(),
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
        (
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/tmp/unixnotis-bus".to_string(),
        ),
    ];

    // Legacy bus state is removed before either graphical environment store is updated
    let with_dbus = manager.environment_sync_commands(&vars, true);
    assert_eq!(with_dbus.len(), 3);
    assert_eq!(with_dbus[0].program(), "systemctl");
    assert_eq!(
        with_dbus[0].args(),
        &["--user", "unset-environment", "DBUS_SESSION_BUS_ADDRESS"]
    );
    assert_eq!(with_dbus[1].program(), "dbus-update-activation-environment");
    assert_eq!(with_dbus[1].args(), &["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"]);
    assert_eq!(with_dbus[2].program(), "systemctl");
    assert_eq!(
        with_dbus[2].args(),
        &[
            "--user",
            "--no-pager",
            "import-environment",
            "WAYLAND_DISPLAY",
            "XDG_RUNTIME_DIR",
        ]
    );

    let without_dbus = manager.environment_sync_commands(&vars, false);
    assert_eq!(without_dbus.len(), 2);
    assert_eq!(without_dbus[0].program(), "systemctl");
    assert_eq!(
        without_dbus[0].args(),
        &["--user", "unset-environment", "DBUS_SESSION_BUS_ADDRESS"]
    );
    assert_eq!(without_dbus[1].program(), "systemctl");
    assert_eq!(
        without_dbus[1].args(),
        &[
            "--user",
            "--no-pager",
            "import-environment",
            "WAYLAND_DISPLAY",
            "XDG_RUNTIME_DIR",
        ]
    );
}
