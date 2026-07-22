//! Directory creation, marker, and empty-removal tests

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

use super::{
    create_directory_all, ensure_marked_directory, remove_empty_directory, validate_child_name,
};
use crate::filesystem::descriptor::{classify_directory_creation, CreateDirectoryOutcome};
use crate::test_support::unique_temp_path;

#[test]
fn directory_creation_builds_missing_components_with_requested_mode() {
    let root = unique_temp_path("create-directory-tree");
    let target = root.join("parent").join("child");

    assert_eq!(
        create_directory_all(&target, 0o750).expect("create directory tree"),
        CreateDirectoryOutcome::TargetCreated
    );
    assert_eq!(
        create_directory_all(&target, 0o700).expect("existing directory stays unchanged"),
        CreateDirectoryOutcome::TargetAlreadyExisted
    );

    for directory in [&root, &root.join("parent"), &target] {
        assert_eq!(
            fs::metadata(directory)
                .expect("directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o750
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ownership_marker_name_accepts_one_normal_component_only() {
    validate_child_name(".owner".as_ref()).expect("plain marker name");

    for invalid in ["", ".", "..", "nested/.owner", "/.owner"] {
        validate_child_name(invalid.as_ref()).expect_err("invalid marker name must fail");
    }
}

#[test]
fn directory_creation_rejects_a_linked_parent() {
    let root = unique_temp_path("create-directory-linked-parent");
    let outside = root.join("outside");
    let linked = root.join("linked");
    fs::create_dir_all(&outside).expect("create outside");
    symlink(&outside, &linked).expect("create parent link");

    create_directory_all(&linked.join("child"), 0o755).expect_err("linked parent should fail");

    assert!(!outside.join("child").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn directory_creation_result_distinguishes_creation_collision_and_failure() {
    assert!(classify_directory_creation(Ok(())).expect("successful mkdir should be new"));
    assert!(
        !classify_directory_creation(Err(std::io::ErrorKind::AlreadyExists.into()))
            .expect("mkdir collision should be retried as existing")
    );

    let error = classify_directory_creation(Err(std::io::ErrorKind::PermissionDenied.into()))
        .expect_err("unrelated mkdir failure should propagate");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
}

#[test]
fn marked_directory_refuses_to_adopt_an_unmarked_existing_target() {
    let root = unique_temp_path("marked-directory-adoption");
    let target = root.join("service");
    fs::create_dir_all(&target).expect("create foreign directory");
    fs::write(target.join("foreign"), "keep").expect("write foreign child");

    ensure_marked_directory(&target, 0o755, ".owner".as_ref(), b"owned\n", 0o644)
        .expect_err("unmarked directory should not be adopted");

    assert!(!target.join(".owner").exists());
    assert_eq!(
        fs::read_to_string(target.join("foreign")).expect("read foreign child"),
        "keep"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn marked_directory_creation_and_reopen_share_one_ownership_contract() {
    let root = unique_temp_path("marked-directory-create");
    let target = root.join("service");

    assert_eq!(
        ensure_marked_directory(&target, 0o750, ".owner".as_ref(), b"owned\n", 0o640)
            .expect("create marked directory"),
        CreateDirectoryOutcome::TargetCreated
    );
    assert_eq!(
        ensure_marked_directory(&target, 0o700, ".owner".as_ref(), b"owned\n", 0o600)
            .expect("validate marked directory"),
        CreateDirectoryOutcome::TargetAlreadyExisted
    );
    assert_eq!(
        fs::read_to_string(target.join(".owner")).expect("read marker"),
        "owned\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn empty_directory_removal_is_idempotent() {
    let root = unique_temp_path("remove-empty-directory");
    let target = root.join("empty");
    fs::create_dir_all(&target).expect("create empty directory");

    assert!(remove_empty_directory(&target).expect("remove empty directory"));
    assert!(!remove_empty_directory(&target).expect("missing directory stays removed"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn empty_directory_removal_rejects_nonempty_and_link_targets() {
    let root = unique_temp_path("remove-empty-directory-shapes");
    let target = root.join("directory");
    let linked = root.join("linked");
    fs::create_dir_all(&target).expect("create target directory");
    fs::write(target.join("file"), "data").expect("write child");
    symlink(&target, &linked).expect("create directory link");

    remove_empty_directory(&target).expect_err("nonempty directory should fail");
    remove_empty_directory(&linked).expect_err("directory link should fail");

    assert!(target.join("file").exists());
    assert!(fs::symlink_metadata(linked)
        .expect("link remains")
        .file_type()
        .is_symlink());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn empty_directory_removal_rejects_linked_ancestors_without_touching_target() {
    let root = unique_temp_path("remove-empty-linked-parent");
    let outside = root.join("outside");
    let linked = root.join("linked");
    fs::create_dir_all(outside.join("empty")).expect("create outside directory");
    symlink(&outside, &linked).expect("create parent link");

    remove_empty_directory(&linked.join("empty")).expect_err("linked parent should fail");

    assert!(outside.join("empty").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn empty_directory_removal_does_not_create_missing_parents() {
    let root = unique_temp_path("remove-empty-missing-parent");
    let missing_parent = root.join("missing");
    let target = missing_parent.join("directory");

    assert!(!remove_empty_directory(&target).expect("empty directory is missing"));

    assert!(!missing_parent.exists());
    let _ = fs::remove_dir_all(root);
}
