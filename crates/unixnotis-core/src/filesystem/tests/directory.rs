//! Descriptor-relative directory operation tests

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

use rustix::fs::{mkfifoat, Mode, CWD};

use super::{
    classify_directory_creation, create_directory_all, remove_directory_tree,
    remove_empty_directory,
};
use crate::test_support::unique_temp_path;

#[test]
fn directory_creation_builds_missing_components_with_requested_mode() {
    let root = unique_temp_path("create-directory-tree");
    let target = root.join("parent").join("child");

    assert!(create_directory_all(&target, 0o750).expect("create directory tree"));
    assert!(!create_directory_all(&target, 0o700).expect("existing directory stays unchanged"));

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
fn recursive_directory_removal_deletes_regular_nested_tree() {
    let root = unique_temp_path("remove-directory-tree");
    let target = root.join("managed");
    fs::create_dir_all(target.join("nested")).expect("create nested directory");
    fs::write(target.join("root-file"), "root").expect("write root file");
    fs::write(target.join("nested").join("child-file"), "child").expect("write child file");

    assert!(remove_directory_tree(&target).expect("remove managed tree"));
    assert!(!remove_directory_tree(&target).expect("missing tree stays removed"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn recursive_directory_removal_rejects_a_child_symlink() {
    let root = unique_temp_path("remove-directory-child-link");
    let target = root.join("managed");
    let protected = root.join("protected");
    fs::create_dir_all(&target).expect("create managed directory");
    fs::write(&protected, "protected").expect("write protected file");
    symlink(&protected, target.join("linked-child")).expect("create child link");

    remove_directory_tree(&target).expect_err("child link should fail");

    assert_eq!(
        fs::read_to_string(protected).expect("read protected file"),
        "protected"
    );
    assert!(target.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn recursive_directory_removal_rejects_a_special_child() {
    let root = unique_temp_path("remove-directory-special-child");
    let target = root.join("managed");
    let fifo = target.join("fifo");
    fs::create_dir_all(&target).expect("create managed directory");
    mkfifoat(CWD, &fifo, Mode::from_raw_mode(0o600)).expect("create fifo child");

    remove_directory_tree(&target).expect_err("special child should fail");

    assert!(fs::symlink_metadata(fifo).is_ok());
    assert!(target.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn directory_removal_rejects_linked_ancestors_without_touching_target() {
    let root = unique_temp_path("remove-directory-linked-parent");
    let outside = root.join("outside");
    let linked = root.join("linked");
    fs::create_dir_all(outside.join("empty")).expect("create outside directory");
    symlink(&outside, &linked).expect("create parent link");

    remove_empty_directory(&linked.join("empty")).expect_err("linked parent should fail");
    remove_directory_tree(&linked).expect_err("linked root should fail");

    assert!(outside.join("empty").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn directory_removal_does_not_create_missing_parents() {
    let root = unique_temp_path("remove-directory-missing-parent");
    let missing_parent = root.join("missing");
    let target = missing_parent.join("directory");

    assert!(!remove_empty_directory(&target).expect("empty directory is missing"));
    assert!(!remove_directory_tree(&target).expect("directory tree is missing"));

    assert!(!missing_parent.exists());
    let _ = fs::remove_dir_all(root);
}
