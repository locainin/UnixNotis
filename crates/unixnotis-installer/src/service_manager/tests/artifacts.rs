use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

use crate::service_manager::{
    ServiceArtifact, ServiceArtifactKind, ServiceManager, MANAGED_DIRECTORY_MARKER,
    UNIXNOTIS_DAEMON_SERVICE,
};

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

#[test]
fn managed_directory_presence_requires_marker_file() {
    let root = test_root("managed-presence-marker");
    let service_dir = root.join("service");
    fs::create_dir_all(&service_dir).expect("service dir");
    let artifact = ServiceArtifact {
        path: service_dir.clone(),
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };

    assert!(!artifact.is_present_safely());

    fs::write(service_dir.join(MANAGED_DIRECTORY_MARKER), "unixnotis\n").expect("marker");

    assert!(artifact.is_present_safely());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn managed_directory_presence_rejects_marker_symlink() {
    let root = test_root("managed-presence-marker-symlink");
    let service_dir = root.join("service");
    fs::create_dir_all(&service_dir).expect("service dir");
    fs::write(root.join("foreign-marker"), "unixnotis\n").expect("foreign marker");
    symlink(
        root.join("foreign-marker"),
        service_dir.join(MANAGED_DIRECTORY_MARKER),
    )
    .expect("marker symlink");
    let artifact = ServiceArtifact {
        path: service_dir,
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };

    assert!(!artifact.is_present_safely());

    let _ = fs::remove_dir_all(root);
}

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
