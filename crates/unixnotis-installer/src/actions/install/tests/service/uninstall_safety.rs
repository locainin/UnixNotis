use std::fs;
use std::os::unix::fs::{symlink, FileTypeExt};
use std::os::unix::net::UnixListener;

use crate::detect::Detection;
use crate::model::ActionMode;
use crate::service_manager::{ServiceArtifact, ServiceArtifactKind};

use super::super::super::service::{remove_service_artifact, write_service_artifact};
use super::super::support::{test_context, test_paths, test_root};

// Uninstall safety tests model hostile or stale filesystem shapes at service artifact paths
// Each case proves cleanup removes only the artifact shape that the backend declared

#[test]
fn uninstall_does_not_follow_service_symlink() {
    let root = test_root("install-service-remove-matching-symlink");
    fs::create_dir_all(&root).expect("make root");
    let target = root.join("target");
    let link = root.join("service-link");
    fs::write(&target, "target").expect("write target");
    // Matching symlink artifacts remove the link itself and leave the target untouched
    symlink(&target, &link).expect("create link");
    let artifact = ServiceArtifact {
        path: link,
        kind: ServiceArtifactKind::Symlink {
            target: target.clone(),
        },
        contents: None,
        mode: None,
    };

    remove_service_artifact(&artifact).expect("link should be removed");

    // Removing a symlink artifact should never remove the linked target
    assert!(fs::symlink_metadata(&artifact.path).is_err());
    assert!(target.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn uninstall_does_not_remove_non_matching_symlink() {
    let root = test_root("install-service-keep-foreign-symlink");
    fs::create_dir_all(&root).expect("make root");
    let actual_target = root.join("actual-target");
    let expected_target = root.join("expected-target");
    let link = root.join("service-link");
    fs::write(&actual_target, "actual").expect("write actual target");
    fs::write(&expected_target, "expected").expect("write expected target");
    // A changed target means the enablement link no longer proves installer ownership
    symlink(&actual_target, &link).expect("create link");
    let artifact = ServiceArtifact {
        path: link.clone(),
        kind: ServiceArtifactKind::Symlink {
            target: expected_target.clone(),
        },
        contents: None,
        mode: None,
    };

    let err = remove_service_artifact(&artifact).expect_err("foreign link should not be removed");

    // The expected target check prevents deleting links now owned by another manager or user edit
    assert!(err.to_string().contains("refusing to remove symlink"));
    assert_eq!(
        fs::read_link(&link).expect("link should remain"),
        actual_target
    );
    assert!(expected_target.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn uninstall_rejects_symlink_file_artifact() {
    let root = test_root("install-service-keep-file-symlink");
    fs::create_dir_all(&root).expect("make root");
    let target = root.join("target");
    let link = root.join("service-file");
    fs::write(&target, "target").expect("write target");
    // File artifacts and link artifacts have different removal contracts
    symlink(&target, &link).expect("create file link");
    let artifact = ServiceArtifact {
        path: link.clone(),
        kind: ServiceArtifactKind::File,
        contents: Some(String::new()),
        mode: None,
    };

    let err = remove_service_artifact(&artifact).expect_err("file link should not be removed");

    // File artifacts reject final-path symlinks instead of unlinking them silently
    assert!(err.to_string().contains("refusing to remove symlink"));
    assert_eq!(fs::read_link(&link).expect("link should remain"), target);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn uninstall_rejects_unmarked_managed_directory() {
    let root = test_root("install-service-unmarked-remove");
    let service_dir = root.join("service-dir");
    fs::create_dir_all(&service_dir).expect("make service dir");
    // Recursive removal is blocked unless the marker proves UnixNotis owns the tree
    fs::write(service_dir.join("foreign"), "do not remove").expect("foreign file");
    let artifact = ServiceArtifact {
        path: service_dir.clone(),
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };

    let err = remove_service_artifact(&artifact).expect_err("unmarked dir should not be removed");

    // Existing unmarked directories are treated as foreign even if their names match ours
    assert!(err.to_string().contains("unmarked service directory"));
    assert!(service_dir.join("foreign").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn uninstall_rejects_managed_directory_symlink_even_when_target_is_marked() {
    let root = test_root("install-service-managed-symlink-remove");
    let target_dir = root.join("target-service-dir");
    let linked_dir = root.join("linked-service-dir");
    fs::create_dir_all(&target_dir).expect("make target service dir");
    fs::write(target_dir.join(".unixnotis-managed"), "unixnotis\n").expect("write marker");
    fs::write(target_dir.join("foreign"), "do not remove").expect("write target file");
    // The root path itself must be a real directory, even if the target looks marked
    symlink(&target_dir, &linked_dir).expect("link service dir");
    let artifact = ServiceArtifact {
        path: linked_dir,
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };

    let err = remove_service_artifact(&artifact).expect_err("managed symlink should not remove");

    // The marked target still remains because root symlink paths are refused before traversal
    assert!(err.to_string().contains("unsafe service directory"));
    assert!(target_dir.join("foreign").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn uninstall_rejects_symlink_inside_managed_directory() {
    let root = test_root("install-service-managed-directory-child-link");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let service_dir = root.join("managed-service");
    let target = root.join("outside-target");
    let child_link = service_dir.join("linked-child");
    let artifact = ServiceArtifact {
        path: service_dir.clone(),
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };
    write_service_artifact(&ctx, &artifact).expect("managed directory should be written");
    fs::write(&target, "outside").expect("write outside target");
    // Child symlinks are refused so recursive cleanup never follows service-tree links
    symlink(&target, &child_link).expect("create child link");

    let err = remove_service_artifact(&artifact).expect_err("child link should be rejected");

    // The full error chain carries the child-link refusal below the outer removal context
    assert!(format!("{err:#}").contains("refusing symlink inside managed service directory"));
    assert_eq!(
        fs::read_link(&child_link).expect("child link should remain untouched"),
        target
    );
    assert!(service_dir.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn uninstall_rejects_socket_inside_managed_directory() {
    let root = test_root("sock");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    // Unix socket paths are small, so this fixture keeps filesystem names short
    let service_dir = root.join("m");
    let socket_path = service_dir.join("s");
    let artifact = ServiceArtifact {
        path: service_dir.clone(),
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };
    write_service_artifact(&ctx, &artifact).expect("managed directory should be written");
    // Special files inside an owned tree are treated as tampering or unexpected runtime state
    let _listener = UnixListener::bind(&socket_path).expect("create socket child");

    let err = remove_service_artifact(&artifact).expect_err("socket child should be rejected");

    // The recursive remover fails closed and does not delete the containing service directory
    assert!(format!("{err:#}").contains("refusing special file inside managed service directory"));
    assert!(fs::symlink_metadata(&socket_path)
        .expect("socket child should remain")
        .file_type()
        .is_socket());
    assert!(service_dir.exists());
    let _ = fs::remove_dir_all(&root);
}
