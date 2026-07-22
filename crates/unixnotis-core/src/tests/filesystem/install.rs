use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

use super::copy_file_atomic;
use crate::test_support::unique_temp_path;

#[test]
fn atomic_copy_replaces_regular_file_and_preserves_source_mode() {
    let root = unique_temp_path("copy-file-replace");
    let source = root.join("release").join("unixnotis-daemon");
    let destination = root.join("bin").join("unixnotis-daemon");
    fs::create_dir_all(source.parent().expect("source parent")).expect("create source parent");
    fs::create_dir_all(destination.parent().expect("destination parent"))
        .expect("create destination parent");
    fs::write(&source, "new binary").expect("write source");
    fs::set_permissions(&source, fs::Permissions::from_mode(0o751)).expect("set source mode");
    fs::write(&destination, "old binary").expect("write destination");

    copy_file_atomic(&source, &destination).expect("copy regular file");

    assert_eq!(
        fs::read_to_string(&destination).expect("read destination"),
        "new binary"
    );
    assert_eq!(
        fs::metadata(&destination)
            .expect("destination metadata")
            .permissions()
            .mode()
            & 0o777,
        0o751
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn atomic_copy_rejects_source_symlink_without_publishing_destination() {
    let root = unique_temp_path("copy-file-source-symlink");
    let source_target = root.join("source-target");
    let source_link = root.join("source-link");
    let destination = root.join("bin").join("unixnotis-daemon");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&source_target, "source").expect("write source target");
    symlink(&source_target, &source_link).expect("create source link");

    copy_file_atomic(&source_link, &destination).expect_err("source link should fail");

    assert!(!destination.exists());
    assert_eq!(
        fs::read_to_string(source_target).expect("read source target"),
        "source"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn atomic_copy_rejects_destination_symlink_without_changing_its_target() {
    let root = unique_temp_path("copy-file-destination-symlink");
    let source = root.join("source");
    let protected = root.join("protected");
    let destination = root.join("destination");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&source, "source").expect("write source");
    fs::write(&protected, "protected").expect("write protected");
    symlink(&protected, &destination).expect("create destination link");

    copy_file_atomic(&source, &destination).expect_err("destination link should fail");

    assert_eq!(
        fs::read_to_string(protected).expect("read protected"),
        "protected"
    );
    assert!(fs::symlink_metadata(destination)
        .expect("destination link remains")
        .file_type()
        .is_symlink());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn atomic_copy_rejects_symlinked_destination_parent() {
    let root = unique_temp_path("copy-file-parent-symlink");
    let source = root.join("source");
    let outside = root.join("outside");
    let linked_parent = root.join("linked-bin");
    fs::create_dir_all(&outside).expect("create outside directory");
    fs::write(&source, "source").expect("write source");
    symlink(&outside, &linked_parent).expect("create parent link");
    let destination = linked_parent.join("unixnotis-daemon");

    copy_file_atomic(&source, &destination).expect_err("linked parent should fail");

    assert!(!outside.join("unixnotis-daemon").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn atomic_copy_rejects_directory_source_without_creating_destination() {
    let root = unique_temp_path("copy-file-directory-source");
    let source = root.join("source-directory");
    let destination = root.join("bin").join("unixnotis-daemon");
    fs::create_dir_all(&source).expect("create source directory");

    copy_file_atomic(&source, &destination).expect_err("directory source should fail");

    assert!(!destination.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn atomic_copy_does_not_create_a_missing_source_parent() {
    let root = unique_temp_path("copy-file-missing-source-parent");
    let missing_parent = root.join("missing-source");
    let source = missing_parent.join("unixnotis-daemon");
    let destination = root.join("bin").join("unixnotis-daemon");

    copy_file_atomic(&source, &destination).expect_err("missing source should fail");

    assert!(!missing_parent.exists());
    assert!(!destination.exists());
    let _ = fs::remove_dir_all(root);
}
