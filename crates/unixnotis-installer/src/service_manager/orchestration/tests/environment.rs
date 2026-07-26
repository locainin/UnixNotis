use std::path::PathBuf;

use crate::service_manager::ServiceManager;

#[test]
fn only_systemd_uses_dbus_environment_helper() {
    let systemd = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd"));
    let dinit = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit"));
    let runit = ServiceManager::runit_user(PathBuf::from("/tmp/runit"));
    let s6 = ServiceManager::s6_user(PathBuf::from("/tmp/s6"), PathBuf::from("/tmp/live"));

    assert!(systemd.uses_dbus_environment_helper());
    assert!(!dinit.uses_dbus_environment_helper());
    assert!(!runit.uses_dbus_environment_helper());
    assert!(!s6.uses_dbus_environment_helper());
}

#[test]
fn artifact_backends_do_not_emit_environment_commands() {
    let runit = ServiceManager::runit_user(PathBuf::from("/tmp/runit"));
    let s6 = ServiceManager::s6_user(PathBuf::from("/tmp/s6"), PathBuf::from("/tmp/live"));
    let values = [("WAYLAND_DISPLAY", "wayland-1".to_string())];

    assert!(runit.environment_sync_commands(&values, true).is_empty());
    assert!(s6.environment_sync_commands(&values, true).is_empty());
}

#[test]
fn backend_environment_policy_keeps_transient_shell_state_out_of_systemd() {
    let systemd = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd"));
    let dinit = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit"));

    assert!(!systemd
        .import_variable_names()
        .contains(&"DBUS_SESSION_BUS_ADDRESS"));
    assert!(!systemd.import_variable_names().contains(&"PATH"));
    assert!(dinit
        .import_variable_names()
        .contains(&"DBUS_SESSION_BUS_ADDRESS"));
    assert!(!dinit.import_variable_names().contains(&"PATH"));
}
