//! Atomic file operation tests

use super::{
    file_mode, reserve_temp, write_file_atomic, write_file_atomic_preserving_mode,
    write_file_if_missing,
};
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::fd::OwnedFd;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::net::UnixStream;

use rustix::fs::{mkfifoat, Mode, CWD};

use crate::filesystem::descriptor::{open_parent, sync_directory};
use crate::test_support::unique_temp_path;

#[test]
fn atomic_write_rejects_target_symlink_without_changing_outside_file() {
    let root = unique_temp_path("atomic-target-symlink");
    fs::create_dir_all(&root).expect("create test root");
    let outside = root.join("outside");
    let target = root.join("state.json");
    fs::write(&outside, "keep").expect("write outside file");
    symlink(&outside, &target).expect("create target symlink");

    let error = write_file_atomic(&target, b"replace", 0o600)
        .expect_err("target symlink should be rejected");

    assert_ne!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read_to_string(outside).expect("read outside file"),
        "keep"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn atomic_write_rejects_symlinked_ancestor_without_creating_outside_file() {
    let root = unique_temp_path("atomic-ancestor-symlink");
    let outside = root.join("outside");
    let linked = root.join("linked");
    fs::create_dir_all(&outside).expect("create outside directory");
    symlink(&outside, &linked).expect("create ancestor symlink");
    let target = linked.join("nested").join("state.json");

    let error = write_file_atomic(&target, b"replace", 0o600)
        .expect_err("symlinked ancestor should be rejected");

    assert_ne!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(!outside.join("nested").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn temp_reservation_skips_planted_symlink_and_uses_next_candidate() {
    let root = unique_temp_path("atomic-temp-symlink");
    fs::create_dir_all(&root).expect("create test root");
    let outside = root.join("outside");
    fs::write(&outside, "keep").expect("write outside file");
    symlink(&outside, root.join("first")).expect("plant temp symlink");
    let (parent_fd, _) = open_parent(&root.join("target")).expect("open parent");

    let (name, mut file) = reserve_temp(
        &parent_fd,
        [OsString::from("first"), OsString::from("second")],
        0o600,
    )
    .expect("reserve second candidate");
    file.write_all(b"new").expect("write reserved file");
    drop(file);

    assert_eq!(name, OsString::from("second"));
    assert_eq!(
        fs::read_to_string(outside).expect("read outside file"),
        "keep"
    );
    assert_eq!(
        fs::read_to_string(root.join("second")).expect("read temp"),
        "new"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn create_if_missing_preserves_existing_file_and_mode() {
    let root = unique_temp_path("atomic-if-missing");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("theme.css");

    assert!(write_file_if_missing(&target, b"first", 0o640).expect("create file"));
    assert!(!write_file_if_missing(&target, b"second", 0o600).expect("preserve file"));

    assert_eq!(fs::read_to_string(&target).expect("read file"), "first");
    let mode = fs::metadata(&target)
        .expect("file metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o640);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn create_if_missing_rejects_every_unsafe_existing_target() {
    let root = unique_temp_path("atomic-if-missing-unsafe");
    fs::create_dir_all(&root).expect("create test root");
    let outside = root.join("outside.css");
    fs::write(&outside, "outside").expect("write outside file");
    let direct_link = root.join("direct-link.css");
    let file_link = root.join("file-link.css");
    let directory = root.join("directory.css");
    let fifo = root.join("fifo.css");
    symlink("missing.css", &direct_link).expect("create dangling link");
    symlink(&outside, &file_link).expect("create file link");
    fs::create_dir(&directory).expect("create directory target");
    mkfifoat(CWD, &fifo, Mode::from_raw_mode(0o600)).expect("create fifo target");

    for target in [&direct_link, &file_link, &directory, &fifo] {
        let error = write_file_if_missing(target, b"replacement", 0o644)
            .expect_err("unsafe existing target should fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
    assert_eq!(
        fs::read_to_string(outside).expect("read outside"),
        "outside"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn create_if_missing_propagates_non_collision_open_error() {
    let root = unique_temp_path("atomic-if-missing-error");
    fs::create_dir_all(&root).expect("create test root");
    let long_name = "x".repeat(300);

    let error = write_file_if_missing(&root.join(long_name), b"data", 0o600)
        .expect_err("overlong target name should fail");

    assert_ne!(error.kind(), std::io::ErrorKind::AlreadyExists);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn create_if_missing_preserves_non_directory_parent_errors() {
    let root = unique_temp_path("atomic-if-missing-parent-file");
    fs::create_dir_all(&root).expect("create test root");
    let parent_file = root.join("parent-file");
    fs::write(&parent_file, "not a directory").expect("write parent file");

    let error = write_file_if_missing(&parent_file.join("state"), b"data", 0o600)
        .expect_err("regular-file parent must reject creation");

    assert_eq!(error.kind(), std::io::ErrorKind::NotADirectory);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn temp_reservation_propagates_non_collision_error_without_using_later_candidate() {
    let root = unique_temp_path("atomic-temp-error");
    fs::create_dir_all(&root).expect("create test root");
    let (parent_fd, _) = open_parent(&root.join("target")).expect("open parent");
    let long_name = OsString::from("x".repeat(300));

    let error = reserve_temp(&parent_fd, [long_name, OsString::from("unused")], 0o600)
        .expect_err("overlong temp name should fail");

    assert_ne!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(!root.join("unused").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn directory_sync_propagates_invalid_descriptor_type() {
    let (stream, _peer) = UnixStream::pair().expect("create socket pair");
    let fd: OwnedFd = stream.into();

    let error = sync_directory(&fd).expect_err("socket cannot be synchronized as a directory");

    assert_ne!(error.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn file_mode_masks_special_and_non_permission_bits() {
    assert_eq!(file_mode(0o17640), Mode::from_raw_mode(0o640));
}

#[test]
fn atomic_write_replaces_regular_file_and_applies_requested_mode() {
    let root = unique_temp_path("atomic-replace");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("state.json");
    fs::write(&target, "old").expect("write old file");

    write_file_atomic(&target, b"new", 0o600).expect("replace file");

    assert_eq!(fs::read_to_string(&target).expect("read file"), "new");
    let mode = fs::metadata(&target)
        .expect("file metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn preserving_atomic_write_keeps_existing_mode_and_replaces_contents() {
    let root = unique_temp_path("atomic-preserve-mode");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("config.toml");
    fs::write(&target, "old").expect("write old file");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("set old mode");

    write_file_atomic_preserving_mode(&target, b"new", 0o644).expect("replace file");

    assert_eq!(fs::read_to_string(&target).expect("read file"), "new");
    let mode = fs::metadata(&target)
        .expect("file metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn preserving_atomic_write_uses_default_mode_for_a_missing_file() {
    let root = unique_temp_path("atomic-preserve-default");
    fs::create_dir_all(&root).expect("create test root");
    let target = root.join("config.toml");

    write_file_atomic_preserving_mode(&target, b"new", 0o640).expect("create file");

    let mode = fs::metadata(&target)
        .expect("file metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o640);
    let _ = fs::remove_dir_all(root);
}
