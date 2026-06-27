use std::fs;
use std::os::unix::fs::{symlink, FileTypeExt, PermissionsExt};
use std::os::unix::net::UnixListener;

use crate::detect::Detection;
use crate::model::ActionMode;
use crate::service_manager::{ServiceArtifact, ServiceArtifactKind};

use super::super::super::service::write_service_artifact;
use super::super::support::{test_context, test_paths, test_root};

// Write-path tests cover the low-level artifact writer before backend-specific lists use it
// The cases here focus on filesystem shape, permissions, and symlink refusal

#[test]
fn write_service_artifact_creates_directory_artifact() {
    let root = test_root("install-service-directory-artifact");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let artifact = ServiceArtifact {
        // Plain directories represent parent containers, not recursive ownership
        path: root.join("service-dir"),
        kind: ServiceArtifactKind::Directory,
        contents: None,
        mode: None,
    };

    let changed = write_service_artifact(&ctx, &artifact).expect("directory should be created");

    assert!(changed);
    assert!(artifact.path.is_dir());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_service_artifact_sets_executable_file_mode() {
    let root = test_root("install-service-executable-artifact");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let artifact = ServiceArtifact {
        // Executable files cover runit and s6 run scripts that must ignore process umask
        path: root.join("run"),
        kind: ServiceArtifactKind::ExecutableFile,
        contents: Some("#!/bin/sh\nexec unixnotis-daemon\n".to_string()),
        mode: Some(0o755),
    };

    let changed = write_service_artifact(&ctx, &artifact).expect("script should be written");

    // The writer returns changed for both new files and content replacement
    assert!(changed);
    assert_eq!(
        fs::read_to_string(&artifact.path).expect("read script"),
        "#!/bin/sh\nexec unixnotis-daemon\n"
    );
    assert_eq!(
        fs::metadata(&artifact.path)
            .expect("script metadata")
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_service_artifact_reports_executable_mode_changes() {
    let root = test_root("install-service-executable-mode-change");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let artifact = ServiceArtifact {
        path: root.join("run"),
        kind: ServiceArtifactKind::ExecutableFile,
        contents: Some("#!/bin/sh\nexec unixnotis-daemon\n".to_string()),
        mode: Some(0o755),
    };
    fs::create_dir_all(&root).expect("make service root");
    fs::write(
        &artifact.path,
        artifact.contents.as_ref().expect("script contents"),
    )
    .expect("seed script");
    fs::set_permissions(&artifact.path, fs::Permissions::from_mode(0o644))
        .expect("seed non-executable mode");

    let changed = write_service_artifact(&ctx, &artifact).expect("mode drift should be fixed");

    // Matching bytes but wrong mode still require a reload/start-visible artifact change
    assert!(changed);
    assert_eq!(
        fs::metadata(&artifact.path)
            .expect("script metadata")
            .permissions()
            .mode()
            & 0o777,
        0o755
    );

    let changed_again =
        write_service_artifact(&ctx, &artifact).expect("matching script should stay quiet");

    // Reinstall should not look dirty once both contents and mode already match
    assert!(!changed_again);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_managed_directory_artifact_creates_ownership_marker() {
    let root = test_root("install-service-managed-directory");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let artifact = ServiceArtifact {
        // Managed directories are recursive uninstall roots and require an ownership marker
        path: root.join("managed-service"),
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };

    let changed = write_service_artifact(&ctx, &artifact).expect("managed dir should be created");

    // Managed directory writes seed the marker that later authorizes recursive cleanup
    assert!(changed);
    assert_eq!(
        fs::read_to_string(artifact.path.join(".unixnotis-managed")).expect("read marker"),
        "unixnotis\n"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_managed_directory_rejects_unmarked_existing_directory() {
    let root = test_root("install-service-unmarked-directory");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let service_dir = root.join("preexisting-service");
    fs::create_dir_all(&service_dir).expect("make preexisting service dir");
    // A foreign file makes the directory look like a pre-existing user or manager service
    fs::write(service_dir.join("foreign"), "do not delete").expect("foreign file");
    let artifact = ServiceArtifact {
        path: service_dir.clone(),
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };

    let err = write_service_artifact(&ctx, &artifact).expect_err("unmarked dir is unsafe");

    // The foreign file should survive because the installer never adopted this directory
    assert!(err.to_string().contains("refusing to manage unmarked"));
    assert!(service_dir.join("foreign").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_service_artifact_rejects_symlink_parent_component() {
    let root = test_root("install-service-symlink-parent");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let target = root.join("target");
    let symlink_parent = root.join("linked-parent");
    fs::create_dir_all(&target).expect("make target dir");
    // Parent symlinks are more dangerous than final-path symlinks because writes can be redirected
    symlink(&target, &symlink_parent).expect("create parent link");
    let artifact = ServiceArtifact {
        path: symlink_parent.join("service-file"),
        kind: ServiceArtifactKind::File,
        contents: Some("contents".to_string()),
        mode: None,
    };

    let err = write_service_artifact(&ctx, &artifact).expect_err("symlink parent is unsafe");

    // The target directory proves the writer did not follow the linked parent
    assert!(format!("{err:#}").contains("refusing symlink parent"));
    assert!(!target.join("service-file").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_service_artifact_rejects_non_matching_symlink_target() {
    let root = test_root("install-service-symlink-target-reject");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    fs::create_dir_all(&root).expect("make root");
    let existing_target = root.join("existing-target");
    let expected_target = root.join("expected-target");
    let link_path = root.join("service-link");
    fs::write(&existing_target, "existing").expect("write existing target");
    fs::write(&expected_target, "expected").expect("write expected target");
    // A symlink with a different target could belong to another manager or a user hand edit
    symlink(&existing_target, &link_path).expect("create existing service link");
    let artifact = ServiceArtifact {
        path: link_path.clone(),
        kind: ServiceArtifactKind::Symlink {
            target: expected_target,
        },
        contents: None,
        mode: None,
    };

    let err = write_service_artifact(&ctx, &artifact).expect_err("foreign symlink is not replaced");

    // The existing link target stays intact, matching uninstall's conservative ownership check
    assert!(err.to_string().contains("cannot replace service symlink"));
    assert_eq!(
        fs::read_link(&link_path).expect("service link should remain"),
        existing_target
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn install_replaces_regular_owned_artifact_but_rejects_unsafe_existing_path() {
    let root = test_root("install-service-owned-replace");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);

    let owned_path = root.join("owned-service-file");
    fs::create_dir_all(&root).expect("make root");
    // Regular files are installer-owned shapes, so replacing old bytes is expected
    fs::write(&owned_path, "old contents").expect("write old owned file");
    let owned_artifact = ServiceArtifact {
        path: owned_path.clone(),
        kind: ServiceArtifactKind::File,
        contents: Some("new contents".to_string()),
        mode: None,
    };

    let changed =
        write_service_artifact(&ctx, &owned_artifact).expect("owned file should be replaced");

    assert!(changed);
    assert_eq!(
        fs::read_to_string(&owned_path).expect("read replaced file"),
        "new contents"
    );

    let foreign_target = root.join("foreign-target");
    let unsafe_file_path = root.join("unsafe-service-file");
    fs::write(&foreign_target, "new contents").expect("write foreign target");
    // Replacing a symlink file would follow attacker-controlled path state
    symlink(&foreign_target, &unsafe_file_path).expect("create unsafe file link");
    let unsafe_file_artifact = ServiceArtifact {
        path: unsafe_file_path.clone(),
        kind: ServiceArtifactKind::File,
        contents: Some("new contents".to_string()),
        mode: None,
    };

    let err = write_service_artifact(&ctx, &unsafe_file_artifact)
        .expect_err("symlink file artifact is unsafe");

    // The symlink remains intact so the foreign target is not modified through it
    assert!(err.to_string().contains("cannot replace symlink"));
    assert_eq!(
        fs::read_link(&unsafe_file_path).expect("unsafe link should remain"),
        foreign_target
    );

    let path = root.join("service-link");
    // Symlink artifacts are allowed to replace symlinks only, not regular user files
    fs::write(&path, "not a symlink").expect("write regular file");
    let artifact = ServiceArtifact {
        path,
        kind: ServiceArtifactKind::Symlink {
            target: root.join("target"),
        },
        contents: None,
        mode: None,
    };

    let err = write_service_artifact(&ctx, &artifact).expect_err("regular file is not replaced");

    assert!(err.to_string().contains("cannot replace non-symlink"));
    assert_eq!(
        fs::read_to_string(&artifact.path).expect("regular file should remain"),
        "not a symlink"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_service_artifact_rejects_socket_artifact_path() {
    let root = test_root("install-service-special-file-reject");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    fs::create_dir_all(&root).expect("make root");
    let socket_path = root.join("service.socket");
    // A Unix socket is a simple special file that must never be read as service text
    let _listener = UnixListener::bind(&socket_path).expect("create socket artifact path");
    let artifact = ServiceArtifact {
        path: socket_path.clone(),
        kind: ServiceArtifactKind::File,
        contents: Some("new contents".to_string()),
        mode: None,
    };

    let err = write_service_artifact(&ctx, &artifact).expect_err("socket path is unsafe");

    // The socket remains untouched and the writer fails before read_to_string can block on it
    assert!(err
        .to_string()
        .contains("cannot replace non-regular service artifact"));
    assert!(fs::symlink_metadata(&socket_path)
        .expect("socket should remain")
        .file_type()
        .is_socket());
    let _ = fs::remove_dir_all(&root);
}
