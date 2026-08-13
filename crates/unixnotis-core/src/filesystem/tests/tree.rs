//! Preflighted directory-tree removal tests

use std::fs;
use std::os::unix::fs::symlink;

use rustix::fs::{mkfifoat, Mode, CWD};

use super::{
    preflight_directory_contents, remove_directory_tree, remove_marked_directory_tree,
    revalidate_directory_identity,
};
use crate::filesystem::descriptor::open_target_directory;
use crate::test_support::unique_temp_path;

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
fn marked_tree_preflight_preserves_regular_siblings_when_a_child_is_unsafe() {
    let root = unique_temp_path("marked-tree-preflight");
    let target = root.join("managed");
    fs::create_dir_all(&target).expect("create managed directory");
    fs::write(target.join(".owner"), "owned\n").expect("write ownership marker");
    fs::write(target.join("regular"), "keep until full preflight").expect("write regular child");
    symlink("regular", target.join("unsafe-link")).expect("create unsafe link");

    remove_marked_directory_tree(&target, ".owner".as_ref(), b"owned\n")
        .expect_err("unsafe child should reject the whole tree");

    assert_eq!(
        fs::read_to_string(target.join("regular")).expect("regular sibling remains"),
        "keep until full preflight"
    );
    assert!(target.join(".owner").exists());
    assert!(fs::symlink_metadata(target.join("unsafe-link"))
        .expect("unsafe link remains")
        .file_type()
        .is_symlink());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn marked_tree_preflight_directly_rejects_unsafe_descendants() {
    let root = unique_temp_path("marked-tree-direct-preflight");
    let target = root.join("managed");
    fs::create_dir_all(target.join("nested")).expect("create nested directory");
    fs::write(target.join("nested").join("regular"), "keep").expect("write regular child");
    symlink("regular", target.join("nested").join("unsafe-link")).expect("create unsafe link");
    let (_parent_fd, _name, directory_fd) = open_target_directory(&target)
        .expect("open target")
        .expect("target exists");

    preflight_directory_contents(&directory_fd).expect_err("unsafe descendant must fail preflight");

    assert_eq!(
        fs::read_to_string(target.join("nested").join("regular")).expect("regular child remains"),
        "keep"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn directory_identity_revalidation_rejects_a_same_device_replacement() {
    let root = unique_temp_path("directory-identity-replacement");
    let target = root.join("managed");
    let moved = root.join("original");
    fs::create_dir_all(&target).expect("create original directory");
    let (parent_fd, file_name, directory_fd) = open_target_directory(&target)
        .expect("open target")
        .expect("target exists");

    revalidate_directory_identity(&parent_fd, &file_name, &directory_fd)
        .expect("unchanged identity should pass");
    fs::rename(&target, &moved).expect("move retained directory");
    fs::create_dir(&target).expect("create same-device replacement");

    revalidate_directory_identity(&parent_fd, &file_name, &directory_fd)
        .expect_err("replacement identity must fail");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn marked_tree_removal_validates_marker_and_deletes_a_preflighted_tree() {
    let root = unique_temp_path("marked-tree-remove");
    let target = root.join("managed");
    fs::create_dir_all(target.join("nested")).expect("create nested tree");
    fs::write(target.join(".owner"), "owned\n").expect("write ownership marker");
    fs::write(target.join("nested").join("file"), "owned").expect("write nested file");

    assert!(
        remove_marked_directory_tree(&target, ".owner".as_ref(), b"owned\n")
            .expect("remove marked tree")
    );
    assert!(!target.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn tree_removal_rejects_linked_ancestors_without_touching_target() {
    let root = unique_temp_path("remove-tree-linked-parent");
    let outside = root.join("outside");
    let linked = root.join("linked");
    fs::create_dir_all(outside.join("empty")).expect("create outside directory");
    symlink(&outside, &linked).expect("create parent link");

    remove_directory_tree(&linked).expect_err("linked root should fail");

    assert!(outside.join("empty").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn tree_removal_does_not_create_missing_parents() {
    let root = unique_temp_path("remove-tree-missing-parent");
    let missing_parent = root.join("missing");
    let target = missing_parent.join("directory");

    assert!(!remove_directory_tree(&target).expect("directory tree is missing"));

    assert!(!missing_parent.exists());
    let _ = fs::remove_dir_all(root);
}
