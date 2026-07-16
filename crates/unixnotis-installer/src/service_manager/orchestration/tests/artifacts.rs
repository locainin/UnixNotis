use std::path::PathBuf;

use crate::service_manager::ServiceManager;

#[test]
fn primary_artifact_paths_stay_under_backend_roots() {
    for manager in [
        ServiceManager::systemd_user(PathBuf::from("/tmp/systemd")),
        ServiceManager::dinit_user(PathBuf::from("/tmp/dinit")),
        ServiceManager::runit_user(PathBuf::from("/tmp/runit")),
        ServiceManager::s6_user(PathBuf::from("/tmp/s6"), PathBuf::from("/tmp/live")),
    ] {
        assert!(
            manager
                .primary_artifact_path()
                .starts_with(manager.artifact_root()),
            "primary artifact must remain under its manager root"
        );
    }
}

#[test]
fn runit_removes_the_gate_already_written_in_install_artifacts() {
    let systemd = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd"));
    let runit = ServiceManager::runit_user(PathBuf::from("/tmp/runit"));

    assert!(systemd.pre_start_artifacts_to_write().is_empty());
    assert!(runit.pre_start_artifacts_to_write().is_empty());
    assert_eq!(runit.pre_start_artifacts_to_remove().len(), 1);
}
