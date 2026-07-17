use std::os::unix::net::UnixListener;

use super::super::environment::*;
use crate::doctor::report::DoctorSeverity;
use crate::test_support::{test_env_lock, EnvGuard};

#[test]
fn healthy_session_environment_reports_transport_without_exposing_address() {
    let _lock = test_env_lock();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-doctor-session-environment-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create runtime directory");
    let socket_path = root.join("bus");
    let _listener = UnixListener::bind(&socket_path).expect("bind runtime bus socket");
    let _runtime = EnvGuard::set("XDG_RUNTIME_DIR", &root);
    let _address = EnvGuard::set(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={}", socket_path.display()),
    );
    let _wayland = EnvGuard::set("WAYLAND_DISPLAY", "wayland-1");

    let check = inspect_session_environment(true);

    assert_eq!(check.severity, DoctorSeverity::Pass);
    assert_eq!(check.data["dbus_transport"], "unix");
    assert!(!check
        .details
        .as_deref()
        .is_some_and(|details| details.contains(&socket_path.display().to_string())));
    std::fs::remove_dir_all(root).expect("remove runtime directory");
}

#[test]
fn missing_environment_becomes_actionable_when_bus_connection_failed() {
    let _lock = test_env_lock();
    let _runtime = EnvGuard::remove("XDG_RUNTIME_DIR");
    let _address = EnvGuard::remove("DBUS_SESSION_BUS_ADDRESS");
    let _wayland = EnvGuard::remove("WAYLAND_DISPLAY");
    let _session = EnvGuard::set("XDG_SESSION_TYPE", "tty");

    let check = inspect_session_environment(false);

    assert_eq!(check.severity, DoctorSeverity::Warning);
    assert!(check
        .hint
        .as_deref()
        .is_some_and(|hint| hint.contains("XDG_RUNTIME_DIR is not set")));
}

#[test]
fn malformed_bus_transport_is_classified_without_echoing_untrusted_text() {
    let _lock = test_env_lock();
    let malicious_prefix = format!("{}\u{1b}[31m{}", "x".repeat(8_192), "injected");
    let address = format!("{malicious_prefix}:value");
    let _address = EnvGuard::set("DBUS_SESSION_BUS_ADDRESS", address);

    let check = inspect_session_environment(true);

    assert_eq!(check.data["dbus_transport"], "other");
    let details = check.details.expect("session environment details");
    assert!(details.contains("other transport"));
    assert!(!details.contains("injected"));
    assert!(!details.contains('\u{1b}'));
    assert!(details.len() < 512);
}

#[test]
fn recognized_bus_transports_use_a_fixed_classification() {
    assert_eq!(classify_bus_transport("unix"), "unix");
    assert_eq!(classify_bus_transport("tcp"), "tcp");
    assert_eq!(classify_bus_transport("nonce-tcp"), "nonce-tcp");
    assert_eq!(classify_bus_transport("autolaunch"), "autolaunch");
    assert_eq!(classify_bus_transport("UNIX"), "other");
}
