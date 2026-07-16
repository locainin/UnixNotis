use std::path::PathBuf;

use crate::service_manager::ServiceManager;

#[test]
fn direct_command_backends_expose_native_availability_probes() {
    for manager in [
        ServiceManager::systemd_user(PathBuf::from("/tmp/systemd")),
        ServiceManager::dinit_user(PathBuf::from("/tmp/dinit")),
        ServiceManager::runit_user(PathBuf::from("/tmp/runit")),
    ] {
        assert!(
            manager.availability_command().is_some(),
            "supported manager must expose an availability command"
        );
    }

    let s6 = ServiceManager::s6_user(PathBuf::from("/tmp/s6"), PathBuf::from("/tmp/live"));
    assert!(
        s6.availability_command().is_none(),
        "s6 validates its command set through readiness checks"
    );
}

#[test]
fn non_systemd_enablement_uses_owned_artifacts() {
    let systemd = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd"));
    let dinit = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit"));

    assert_eq!(systemd.enabled_by_artifacts(), None);
    assert_eq!(dinit.enabled_by_artifacts(), Some(false));
}
