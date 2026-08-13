use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::PathBuf;

use crate::service_manager::backends::systemd::SERVICE_NAME as UNIXNOTIS_DAEMON_SERVICE;
use crate::service_manager::contract::MANAGED_DIRECTORY_MARKER;
use crate::service_manager::{
    ServiceArtifact, ServiceArtifactKind, ServiceArtifactState, ServiceManager,
};

#[test]
fn systemd_backend_reports_primary_artifact_path() {
    let root = std::env::temp_dir().join("systemd").join("user");
    let manager = ServiceManager::systemd_user(root.clone());

    assert_eq!(manager.artifact_root(), root);
    assert_eq!(
        manager.primary_artifact_path(),
        root.join(UNIXNOTIS_DAEMON_SERVICE)
    );
}

#[test]
fn systemd_backend_uses_file_artifact_not_external_renderer() {
    let manager = ServiceManager::systemd_user(std::env::temp_dir().join("systemd").join("user"));
    let binary_root = std::env::temp_dir().join("bin");
    let artifacts = manager.artifacts(&binary_root);

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
    assert_eq!(
        artifact.inspect().expect("missing marker state"),
        ServiceArtifactState::UnexpectedObject
    );

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

#[test]
fn artifact_conflict_detects_file_symlink_as_unsafe_presence() {
    let root = test_root("artifact-conflict-file-symlink");
    fs::create_dir_all(&root).expect("root");
    let target = root.join("foreign-target");
    let path = root.join("service-file");
    fs::write(&target, "foreign").expect("target");
    symlink(&target, &path).expect("file symlink");
    let artifact = ServiceArtifact {
        path,
        kind: ServiceArtifactKind::File,
        contents: Some("owned".to_string()),
        mode: None,
    };

    assert!(!artifact.is_present_safely());
    assert!(artifact.exists_at_path_but_not_safely());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn artifact_conflict_detects_wrong_symlink_target() {
    let root = test_root("artifact-conflict-wrong-symlink");
    fs::create_dir_all(&root).expect("root");
    let actual = root.join("actual-target");
    let expected = root.join("expected-target");
    let path = root.join("service-link");
    fs::write(&actual, "actual").expect("actual");
    symlink(&actual, &path).expect("service link");
    let artifact = ServiceArtifact {
        path,
        kind: ServiceArtifactKind::Symlink { target: expected },
        contents: None,
        mode: None,
    };

    assert!(!artifact.is_present_safely());
    assert!(artifact.exists_at_path_but_not_safely());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_artifact_presence_rejects_directory_and_accepts_regular_file() {
    let root = test_root("artifact-file-presence-shape");
    fs::create_dir_all(&root).expect("root");
    let path = root.join("service-file");
    let artifact = ServiceArtifact {
        path: path.clone(),
        kind: ServiceArtifactKind::File,
        contents: Some("owned".to_string()),
        mode: None,
    };

    fs::create_dir_all(&path).expect("directory at file path");

    // Directories at file artifact paths are conflicts, not valid installed files
    assert!(!artifact.is_present_safely());
    assert!(artifact.exists_at_path_but_not_safely());

    fs::remove_dir(&path).expect("remove directory");
    fs::write(&path, "foreign contents").expect("regular file");

    // Regular files are the safe shape; content comparison belongs to shared artifacts
    assert!(artifact.is_present_safely());
    assert!(!artifact.exists_at_path_but_not_safely());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn shared_file_presence_requires_exact_contents() {
    let root = test_root("artifact-shared-presence");
    fs::create_dir_all(&root).expect("root");
    let path = root.join("type");
    let artifact = ServiceArtifact {
        path: path.clone(),
        kind: ServiceArtifactKind::SharedFile {
            created_marker: None,
        },
        contents: Some("bundle\n".to_string()),
        mode: Some(0o644),
    };

    fs::write(&path, "longrun\n").expect("foreign shared file");

    // Shared setup files count only when bytes still match the backend contract
    assert!(!artifact.is_present_safely());
    assert!(artifact.exists_at_path_but_not_safely());

    fs::write(&path, "bundle\n").expect("matching shared file");

    assert!(artifact.is_present_safely());
    assert!(!artifact.exists_at_path_but_not_safely());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn managed_directory_marker_rejects_wrong_contents() {
    let root = test_root("managed-marker-wrong-contents");
    let service_dir = root.join("service");
    fs::create_dir_all(&service_dir).expect("service dir");
    fs::write(
        service_dir.join(MANAGED_DIRECTORY_MARKER),
        "not unixnotis\n",
    )
    .expect("marker");
    let artifact = ServiceArtifact {
        path: service_dir,
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };

    // The marker has to be exact so a random file name collision is not ownership proof
    assert!(!artifact.is_present_safely());
    assert!(artifact.exists_at_path_but_not_safely());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn artifact_inspection_propagates_non_missing_path_errors() {
    let root = test_root("artifact-inspection-error");
    fs::create_dir_all(&root).expect("create artifact inspection root");
    let regular_parent = root.join("regular-parent");
    fs::write(&regular_parent, "not a directory").expect("create invalid parent");
    let artifact = ServiceArtifact {
        path: regular_parent.join("service"),
        kind: ServiceArtifactKind::File,
        contents: Some("owned".to_string()),
        mode: None,
    };

    assert!(
        artifact.inspect().is_err(),
        "path lookup failures must not become missing artifacts"
    );
    fs::remove_dir_all(root).expect("remove artifact inspection fixture");
}

#[test]
fn managed_marker_inspection_propagates_permission_errors() {
    // Root bypasses directory mode bits, so this boundary cannot be observed in root CI
    if rustix::process::getuid().as_raw() == 0 {
        return;
    }
    let root = test_root("managed-marker-inspection-error");
    let service_dir = root.join("service");
    fs::create_dir_all(&service_dir).expect("create managed service directory");
    fs::set_permissions(&service_dir, fs::Permissions::from_mode(0o000))
        .expect("remove service directory search permission");
    let artifact = ServiceArtifact {
        path: service_dir.clone(),
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };

    assert!(
        artifact.inspect().is_err(),
        "marker lookup failures must not become an absent marker"
    );
    fs::set_permissions(&service_dir, fs::Permissions::from_mode(0o700))
        .expect("restore service directory permission");
    fs::remove_dir_all(root).expect("remove marker inspection fixture");
}

#[test]
fn plain_directory_presence_rejects_regular_file() {
    let root = test_root("artifact-directory-presence");
    fs::create_dir_all(&root).expect("root");
    let path = root.join("env");
    fs::write(&path, "not a directory").expect("file at directory path");
    let artifact = ServiceArtifact {
        path,
        kind: ServiceArtifactKind::Directory,
        contents: None,
        mode: None,
    };

    // Plain directory artifacts are shared containers and must not adopt files
    assert!(!artifact.is_present_safely());
    assert!(artifact.exists_at_path_but_not_safely());

    let _ = fs::remove_dir_all(root);
}

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("unixnotis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}
