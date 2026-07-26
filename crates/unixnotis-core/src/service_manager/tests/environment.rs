use super::super::{validate_session_bus_address, variables_for_backend, ServiceManagerKind};

#[test]
fn systemd_environment_excludes_shell_bus_and_path_values() {
    let variables = variables_for_backend(ServiceManagerKind::Systemd);

    assert!(variables.contains(&"WAYLAND_DISPLAY"));
    assert!(variables.contains(&"XDG_RUNTIME_DIR"));
    assert!(!variables.contains(&"DBUS_SESSION_BUS_ADDRESS"));
    assert!(!variables.contains(&"PATH"));
}

#[test]
fn direct_managers_accept_only_an_explicit_stable_bus_variable() {
    for kind in [
        ServiceManagerKind::Dinit,
        ServiceManagerKind::Runit,
        ServiceManagerKind::S6,
    ] {
        let variables = variables_for_backend(kind);
        assert!(variables.contains(&"DBUS_SESSION_BUS_ADDRESS"));
        assert!(!variables.contains(&"PATH"));
    }
}

#[test]
fn persisted_session_bus_address_must_match_the_standard_user_bus() {
    assert!(validate_session_bus_address("unix:path=/run/user/1000/bus", 1000).is_ok());

    let error = validate_session_bus_address("unix:path=/tmp/transient-bus", 1000)
        .expect_err("transient bus must be rejected");
    assert_eq!(
        error.to_string(),
        "refusing to persist nonstandard session bus address: unix:path=/tmp/transient-bus"
    );
}
