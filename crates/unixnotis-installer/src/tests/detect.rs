use crate::test_support::fs::write_executable;
use std::fs;
use std::io::{Error, ErrorKind};
use std::os::unix::process::ExitStatusExt;

use crate::detect::{
    ensure_owner_is_current, notification_owner_for_mutation_until, parse_busctl_json,
    parse_busctl_status, parse_busctl_string_reply, read_busctl_owner_strict, read_cmdline_program,
    read_comm, systemctl_spawn_error, validate_busctl_output, KNOWN_DAEMONS,
    MAX_BUSCTL_OUTPUT_BYTES,
};

#[test]
fn known_daemons_include_quickshell_owner() {
    // Installer detection should match daemon trial-mode owner handling
    let quickshell = KNOWN_DAEMONS
        .iter()
        .find(|daemon| daemon.name == "quickshell")
        .expect("quickshell should be known");

    // Unit metadata keeps status output and restore hints consistent
    assert_eq!(quickshell.systemd_unit, None);
}

#[test]
fn known_daemons_include_recent_wayland_notifiers() {
    // These daemons are common enough to deserve explicit regression coverage
    let expected = [("hyprnotify", None), ("fnott", Some("fnott.service"))];

    for (name, unit) in expected {
        let daemon = KNOWN_DAEMONS
            .iter()
            .find(|daemon| daemon.name == name)
            .expect("daemon should be known");

        assert_eq!(daemon.systemd_unit, unit);
    }
}

#[test]
fn known_daemons_cover_standalone_desktop_and_wayland_owners() {
    for name in [
        "xfce4-notifyd",
        "lxqt-notificationd",
        "mate-notification-daemon",
        "notification-daemon",
        "wired",
        "deadd-notification-center",
        "tiramisu",
        "runst",
    ] {
        assert!(
            KNOWN_DAEMONS.iter().any(|daemon| daemon.name == name),
            "{name} should be recognized"
        );
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
fn parse_busctl_json_keeps_the_first_valid_pid_and_command_identity() {
    let output = r#"
{
  "first": { "PID": 111, "Comm": "first-owner" },
  "second": { "PID": 222, "Comm": "second-owner" }
}
"#;

    let owner = parse_busctl_json(output).expect("expected parsed owner info");

    assert_eq!(owner.pid, Some(111));
    assert_eq!(owner.comm.as_deref(), Some("first-owner"));
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
fn parse_busctl_string_reply_reads_unique_owner_name() {
    assert_eq!(
        parse_busctl_string_reply("s \":1.77\"\n").as_deref(),
        Some(":1.77")
    );
    assert!(parse_busctl_string_reply("s \"\"").is_none());
    assert!(parse_busctl_string_reply("invalid").is_none());
}

#[test]
fn strict_owner_detection_keeps_broker_failure_distinct_from_unowned() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("strict-owner-error");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake tool directory");
    write_executable(&fake_bin.join("busctl"), "#!/bin/sh\nexit 7\n");
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    let error = read_busctl_owner_strict().expect_err("broker failure must block mutation");

    assert!(
        error.to_string().contains("busctl owner query failed"),
        "unexpected strict detection error: {error:#}"
    );
    fs::remove_dir_all(root).expect("remove strict owner fixture");
}

#[test]
fn strict_owner_detection_accepts_only_explicit_unowned_reply() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("strict-owner-unowned");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake tool directory");
    write_executable(
        &fake_bin.join("busctl"),
        "#!/bin/sh\nprintf '%s\\n' 'b false'\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    assert!(
        read_busctl_owner_strict()
            .expect("explicit unowned reply")
            .is_none(),
        "explicit false must be the only unowned state"
    );
    fs::remove_dir_all(root).expect("remove strict unowned fixture");
}

#[test]
fn strict_owner_detection_retains_the_exact_unique_address() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("strict-owner-identity");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake tool directory");
    write_executable(
        &fake_bin.join("busctl"),
        "#!/bin/sh\ncase \"$*\" in *NameHasOwner*) printf 'b true\\n' ;; *GetNameOwner*) printf 's \":1.77\"\\n' ;; *'status :1.77'*) printf 'Comm=unixnotis-daemon\\n' ;; *) exit 1 ;; esac\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    let owner = read_busctl_owner_strict()
        .expect("strict owned query")
        .expect("owned notification name");

    assert_eq!(owner.unique_name.as_deref(), Some(":1.77"));
    fs::remove_dir_all(root).expect("remove strict owner fixture");
}

#[test]
fn strict_owner_revalidation_rejects_a_different_current_address() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("strict-owner-handoff");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake tool directory");
    write_executable(
        &fake_bin.join("busctl"),
        "#!/bin/sh\ncase \"$*\" in *NameHasOwner*) printf 'b true\\n' ;; *GetNameOwner*) printf 's \":1.88\"\\n' ;; *) exit 1 ;; esac\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    let error = ensure_owner_is_current(":1.77")
        .expect_err("a new transport owner must invalidate inspected process metadata");

    assert!(error.to_string().contains("owner changed"));
    fs::remove_dir_all(root).expect("remove strict owner handoff fixture");
}

#[test]
fn strict_bus_output_budget_accepts_exact_limit_and_rejects_each_oversized_stream() {
    assert_eq!(MAX_BUSCTL_OUTPUT_BYTES, 65_536);
    let output = |stdout: Vec<u8>, stderr: Vec<u8>, stdout_truncated, stderr_truncated| {
        crate::system_tools::BoundedOutput {
            status: std::process::ExitStatus::from_raw(0),
            stdout,
            stderr,
            stdout_truncated,
            stderr_truncated,
        }
    };

    let exact = vec![b'x'; MAX_BUSCTL_OUTPUT_BYTES];
    assert_eq!(
        validate_busctl_output(output(exact.clone(), Vec::new(), false, false))
            .expect("exact output limit must remain valid")
            .len(),
        MAX_BUSCTL_OUTPUT_BYTES
    );
    assert!(
        validate_busctl_output(output(Vec::new(), exact, false, false)).is_ok(),
        "exact stderr limit must remain valid"
    );
    assert!(validate_busctl_output(output(Vec::new(), Vec::new(), true, false)).is_err());
    assert!(validate_busctl_output(output(Vec::new(), Vec::new(), false, true)).is_err());
}

#[test]
fn strict_owner_probe_kills_a_hung_busctl_at_the_shared_deadline() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("strict-owner-timeout");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake tool directory");
    write_executable(&fake_bin.join("busctl"), "#!/bin/sh\nsleep 30\n");
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let started = std::time::Instant::now();
    let deadline = started + std::time::Duration::from_millis(25);

    let error = notification_owner_for_mutation_until(deadline)
        .expect_err("hung owner query must fail at the shared deadline");

    assert!(
        error.chain().any(|cause| {
            cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == ErrorKind::TimedOut)
        }),
        "timeout context must retain the operating-system timeout kind: {error:#}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "hung busctl and helper processes must be killed promptly"
    );
    fs::remove_dir_all(root).expect("remove strict owner timeout fixture");
}

#[test]
fn parse_busctl_json_returns_none_for_invalid_json() {
    let owner = parse_busctl_json("not json");

    assert!(owner.is_none());
}

#[test]
fn read_cmdline_program_reports_current_test_process_name() {
    let program = read_cmdline_program(std::process::id()).expect("current process argv0");

    // argv0 should always provide a non-empty executable basename for the current process
    assert!(!program.trim().is_empty());
    assert!(!program.contains('/'));
}

#[test]
fn read_cmdline_program_returns_none_for_missing_process() {
    let program = read_cmdline_program(u32::MAX);

    // Missing /proc entries should be a clean absence, not an error-shaped owner
    assert!(program.is_none());
}

#[test]
fn read_comm_reports_current_test_process_name() {
    let comm = read_comm(std::process::id()).expect("current process comm");

    // comm is the fallback name used when busctl does not provide a reliable command
    assert!(!comm.trim().is_empty());
}

#[test]
fn read_comm_returns_none_for_missing_process() {
    let comm = read_comm(u32::MAX);

    // Missing processes must not produce placeholder names that could match a daemon
    assert!(comm.is_none());
}

#[test]
fn read_comm_prefers_a_live_proc_identity_without_invoking_ps() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("read-comm-proc-first");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("create fake tool directory");
    write_executable(
        &fake_bin.join("ps"),
        "#!/bin/sh\nprintf 'wrong-fallback\\n'\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);
    let expected = fs::read_to_string(format!("/proc/{}/comm", std::process::id()))
        .expect("read current process comm")
        .trim()
        .to_string();

    assert_eq!(
        read_comm(std::process::id()).as_deref(),
        Some(expected.as_str())
    );
    fs::remove_dir_all(root).expect("remove proc comm fixture");
}

#[test]
fn read_comm_uses_successful_ps_output_when_proc_identity_is_missing() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("read-comm-ps-fallback");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("create fake tool directory");
    write_executable(
        &fake_bin.join("ps"),
        "#!/bin/sh\nprintf 'fallback-owner\\n'\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    assert_eq!(read_comm(u32::MAX).as_deref(), Some("fallback-owner"));
    fs::remove_dir_all(root).expect("remove ps comm fixture");
}

#[test]
fn missing_systemctl_does_not_emit_per_daemon_status_errors() {
    // Non-systemd installs can still use D-Bus and process detection without systemctl
    let err = Error::from(ErrorKind::NotFound);
    assert!(systemctl_spawn_error(&err).is_none());
}

#[test]
fn unexpected_systemctl_spawn_errors_remain_visible() {
    let err = Error::from(ErrorKind::PermissionDenied);

    assert!(systemctl_spawn_error(&err).is_some());
}

#[test]
fn detect_uses_bus_owner_systemd_status_and_pgrep_results() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("detect-fake-commands");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    write_executable(
        &fake_bin.join("busctl"),
        "#!/bin/sh\n\
         if [ \"$2\" = '--json=short' ]; then\n\
         printf '{\"Status\":{\"Comm\":\"dunst\"}}\\n'\n\
         exit 0\n\
         fi\n\
         exit 1\n",
    );
    write_executable(
        &fake_bin.join("systemctl"),
        "#!/bin/sh\n\
         if [ \"$4\" = 'dunst.service' ]; then exit 0; fi\n\
         exit 3\n",
    );
    write_executable(
        &fake_bin.join("pgrep"),
        "#!/bin/sh\n\
         if [ \"$4\" = 'unixnotis-daemon' ]; then\n\
         printf '4321\\nnot-a-pid\\n'\n\
         exit 0\n\
         fi\n\
         exit 1\n",
    );
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    let detection = crate::detect::detect();

    // Fake commands exercise the full detection flow without touching host daemon state
    assert_eq!(detection.owner.as_ref().and_then(|owner| owner.pid), None);
    assert_eq!(
        detection
            .owner
            .as_ref()
            .and_then(|owner| owner.comm.as_deref()),
        Some("dunst")
    );
    let dunst = detection
        .daemons
        .iter()
        .find(|daemon| daemon.name == "dunst")
        .expect("dunst entry should exist");
    assert!(dunst.systemd_active);
    assert!(dunst.systemd_error.is_none());
    assert!(dunst.is_owner);

    let unixnotis = detection
        .daemons
        .iter()
        .find(|daemon| daemon.name == "unixnotis-daemon")
        .expect("unixnotis entry should exist");
    assert_eq!(unixnotis.running_pids, [4321]);
    assert!(!unixnotis.systemd_active);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn detect_falls_back_to_text_busctl_status_when_json_status_fails() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("detect-text-busctl-fallback");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    write_executable(
        &fake_bin.join("busctl"),
        "#!/bin/sh\n\
         if [ \"$2\" = '--json=short' ]; then exit 1; fi\n\
         printf 'Status of org.freedesktop.Notifications:\\n   Comm=mako\\n'\n",
    );
    write_executable(&fake_bin.join("systemctl"), "#!/bin/sh\nexit 3\n");
    write_executable(&fake_bin.join("pgrep"), "#!/bin/sh\nexit 1\n");
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    let detection = crate::detect::detect();

    // Older busctl versions may lack JSON output, so text fallback must still identify owners
    assert_eq!(
        detection
            .owner
            .as_ref()
            .and_then(|owner| owner.comm.as_deref()),
        Some("mako")
    );
    assert!(detection
        .daemons
        .iter()
        .any(|daemon| daemon.name == "mako" && daemon.is_owner));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn detect_resolves_unique_owner_when_well_known_status_has_no_process_fields() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = test_root("detect-unique-owner-fallback");
    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).expect("fake bin dir");
    write_executable(
        &fake_bin.join("busctl"),
        "#!/bin/sh\n\
         if [ \"$2\" = '--json=short' ] && [ \"$4\" = ':1.77' ]; then\n\
         printf '{\"Status\":{\"Comm\":\"fnott\"}}\\n'\n\
         exit 0\n\
         fi\n\
         if [ \"$2\" = '--json=short' ]; then printf '{}\\n'; exit 0; fi\n\
         if [ \"$2\" = 'status' ]; then printf 'Name=org.freedesktop.Notifications\\n'; exit 0; fi\n\
         if [ \"$2\" = 'call' ]; then printf 's \":1.77\"\\n'; exit 0; fi\n\
         exit 1\n",
    );
    write_executable(&fake_bin.join("systemctl"), "#!/bin/sh\nexit 3\n");
    write_executable(&fake_bin.join("pgrep"), "#!/bin/sh\nexit 1\n");
    let _fake_tools = crate::system_tools::routing::use_fake_tool_bin(&fake_bin);

    let detection = crate::detect::detect();

    assert_eq!(
        detection
            .owner
            .as_ref()
            .and_then(|owner| owner.comm.as_deref()),
        Some("fnott")
    );
    assert!(detection
        .daemons
        .iter()
        .any(|daemon| daemon.name == "fnott" && daemon.is_owner));

    let _ = fs::remove_dir_all(root);
}

fn test_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
