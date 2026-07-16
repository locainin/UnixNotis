use std::path::PathBuf;

use crate::service_manager::ServiceManager;

#[test]
fn manager_constructors_preserve_backend_roots() {
    let systemd = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd"));
    let dinit = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit"));
    let runit = ServiceManager::runit_user(PathBuf::from("/tmp/runit"));
    let s6 = ServiceManager::s6_user(PathBuf::from("/tmp/s6"), PathBuf::from("/tmp/live"));

    assert_eq!(systemd.artifact_root(), PathBuf::from("/tmp/systemd"));
    assert_eq!(dinit.artifact_root(), PathBuf::from("/tmp/dinit"));
    assert_eq!(runit.artifact_root(), PathBuf::from("/tmp/runit"));
    assert_eq!(s6.artifact_root(), PathBuf::from("/tmp/s6"));
}

#[test]
fn backend_identity_requires_both_kind_and_root() {
    let first = ServiceManager::runit_user(PathBuf::from("/tmp/services"));
    let same = ServiceManager::runit_user(PathBuf::from("/tmp/services"));
    let other_kind = ServiceManager::dinit_user(PathBuf::from("/tmp/services"));
    let other_root = ServiceManager::runit_user(PathBuf::from("/tmp/other"));

    assert!(first.manages_same_backend_root(&same));
    assert!(!first.manages_same_backend_root(&other_kind));
    assert!(!first.manages_same_backend_root(&other_root));
}
