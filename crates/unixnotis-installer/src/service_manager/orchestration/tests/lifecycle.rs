use std::path::PathBuf;

use crate::service_manager::ServiceManager;

#[test]
fn non_systemd_enablement_uses_owned_artifacts() {
    let systemd = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd"));
    let dinit = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit"));

    assert_eq!(systemd.enabled_by_artifacts(), None);
    assert_eq!(dinit.enabled_by_artifacts(), Some(false));
}

#[test]
fn only_systemd_needs_temporary_start_state_cleanup() {
    let systemd = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd"));
    assert!(
        systemd.prepare_start_command().is_some(),
        "systemd should clear a runtime mask before an explicit start"
    );

    for manager in [
        ServiceManager::dinit_user(PathBuf::from("/tmp/dinit")),
        ServiceManager::runit_user(PathBuf::from("/tmp/runit")),
        ServiceManager::s6_user(PathBuf::from("/tmp/s6"), PathBuf::from("/tmp/live")),
    ] {
        assert!(
            manager.prepare_start_command().is_none(),
            "non-systemd managers must not receive systemd mask cleanup"
        );
    }
}
