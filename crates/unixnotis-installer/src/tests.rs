use crate::detect::{parse_busctl_json, parse_busctl_status, KNOWN_DAEMONS};

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
