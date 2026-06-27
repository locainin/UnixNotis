use std::io::{Error, ErrorKind};

use crate::detect::{parse_busctl_json, parse_busctl_status, systemctl_spawn_error, KNOWN_DAEMONS};

#[test]
fn known_daemons_include_quickshell_owner() {
    // Installer detection should match daemon trial-mode owner handling
    let quickshell = KNOWN_DAEMONS
        .iter()
        .find(|daemon| daemon.name == "quickshell")
        .expect("quickshell should be known");

    // Unit metadata keeps status output and restore hints consistent
    assert_eq!(quickshell.unit, "quickshell.service");
}

#[test]
fn known_daemons_include_recent_wayland_notifiers() {
    // These daemons are common enough to deserve explicit regression coverage
    let expected = [
        ("hyprnotify", "hyprnotify.service"),
        ("fnott", "fnott.service"),
    ];

    for (name, unit) in expected {
        let daemon = KNOWN_DAEMONS
            .iter()
            .find(|daemon| daemon.name == name)
            .expect("daemon should be known");

        assert_eq!(daemon.unit, unit);
    }
}

#[test]
fn parse_busctl_status_reads_indented_fields() {
    // Confirms indented output with spaced separators still yields PID and command name
    let output = "\
Status of org.freedesktop.Notifications:
   Name=org.freedesktop.Notifications
   PID = 4242
   UID=1000
   User=user
   Comm = unixnotis-daemon
";
    let owner = parse_busctl_status(output).expect("expected parsed owner info");
    assert_eq!(owner.pid, Some(4242));
    assert_eq!(owner.comm.as_deref(), Some("unixnotis-daemon"));
}

#[test]
fn parse_busctl_status_handles_comm_only() {
    // Verifies comm-only output remains useful when PID is absent
    let output = "\
Status of org.freedesktop.Notifications:
    Comm=dunst
";
    let owner = parse_busctl_status(output).expect("expected parsed owner info");
    assert_eq!(owner.pid, None);
    assert_eq!(owner.comm.as_deref(), Some("dunst"));
}

#[test]
fn parse_busctl_status_ignores_invalid_pid() {
    // Ensures invalid PID values do not produce a false-positive owner
    let output = "\
Status of org.freedesktop.Notifications:
    PID=not-a-number
";
    let owner = parse_busctl_status(output);
    assert!(owner.is_none());
}

#[test]
fn parse_busctl_status_ignores_zero_pid() {
    // Treats PID 0 as invalid while still preserving the command name
    let output = "\
Status of org.freedesktop.Notifications:
    PID=0
    Comm=notify-osd
";
    let owner = parse_busctl_status(output).expect("expected parsed owner info");
    assert_eq!(owner.pid, None);
    assert_eq!(owner.comm.as_deref(), Some("notify-osd"));
}

#[test]
fn parse_busctl_json_reads_pid_and_comm() {
    // Confirms JSON parsing extracts PID and command name when present
    let output = r#"
{
  "Status": {
    "PID": 4242,
    "Comm": "unixnotis-daemon"
  }
}
"#;
    let owner = parse_busctl_json(output).expect("expected parsed owner info");
    assert_eq!(owner.pid, Some(4242));
    assert_eq!(owner.comm.as_deref(), Some("unixnotis-daemon"));
}

#[test]
fn missing_systemctl_does_not_emit_per_daemon_status_errors() {
    // Non-systemd installs can still use D-Bus and process detection without systemctl
    let err = Error::from(ErrorKind::NotFound);
    assert!(systemctl_spawn_error(&err).is_none());
}
