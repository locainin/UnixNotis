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
fn parse_busctl_json_walks_nested_arrays_and_objects() {
    // busctl JSON shape has changed across versions; recursive walking keeps
    // owner detection useful even when PID and Comm move inside arrays
    let output = r#"
{
  "outer": [
    { "ignored": true },
    { "nested": [{ "PID": "5252" }, { "Comm": "mako" }] }
  ]
}
"#;

    let owner = parse_busctl_json(output).expect("expected parsed owner info");

    assert_eq!(owner.pid, Some(5252));
    assert_eq!(owner.comm.as_deref(), Some("mako"));
}

#[test]
fn parse_busctl_json_ignores_empty_comm_and_keeps_later_valid_value() {
    let output = r#"
{
  "first": { "Comm": "   " },
  "second": { "Comm": "dunst" }
}
"#;

    let owner = parse_busctl_json(output).expect("expected parsed owner info");

    assert_eq!(owner.pid, None);
    assert_eq!(owner.comm.as_deref(), Some("dunst"));
}

#[test]
fn parse_busctl_json_rejects_zero_and_out_of_range_pid_values() {
    let zero = parse_busctl_json(r#"{ "PID": 0 }"#);
    assert!(zero.is_none());

    let too_large = parse_busctl_json(r#"{ "PID": 4294967296 }"#);
    assert!(too_large.is_none());
}

#[test]
fn parse_busctl_json_rejects_invalid_pid_string() {
    let output = r#"{ "PID": "not-a-pid" }"#;

    let owner = parse_busctl_json(output);

    assert!(owner.is_none());
}

#[test]
fn parse_busctl_json_returns_none_for_invalid_json() {
    let owner = parse_busctl_json("not json");

    assert!(owner.is_none());
}

#[test]
fn missing_systemctl_does_not_emit_per_daemon_status_errors() {
    // Non-systemd installs can still use D-Bus and process detection without systemctl
    let err = Error::from(ErrorKind::NotFound);
    assert!(systemctl_spawn_error(&err).is_none());
}
