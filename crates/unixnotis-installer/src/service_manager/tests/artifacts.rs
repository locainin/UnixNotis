use std::path::PathBuf;

use crate::service_manager::{ServiceArtifactKind, ServiceManager, UNIXNOTIS_DAEMON_SERVICE};

#[test]
fn systemd_backend_reports_primary_artifact_path() {
    let root = PathBuf::from("/tmp/systemd/user");
    let manager = ServiceManager::systemd_user(root.clone());

    assert_eq!(manager.artifact_root(), root);
    assert_eq!(
        manager.primary_artifact_path(),
        PathBuf::from("/tmp/systemd/user").join(UNIXNOTIS_DAEMON_SERVICE)
    );
}

#[test]
fn systemd_backend_uses_file_artifact_not_external_renderer() {
    let manager = ServiceManager::systemd_user(PathBuf::from("/tmp/systemd/user"));
    let artifacts = manager.artifacts(std::path::Path::new("/tmp/bin"));

    assert_eq!(artifacts[0].kind, ServiceArtifactKind::File);
    assert!(artifacts[0].contents.is_some());
    assert_eq!(artifacts[0].mode, None);
}
