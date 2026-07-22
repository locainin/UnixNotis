//! Symbolic-link operation tests

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use super::{
    classify_symlink_creation, create_symlink_if_missing, existing_link_outcome, open_parent,
    read_symlink, replace_symlink_atomic, reserve_temp_symlink, validate_symlink_or_missing,
    CreateSymlinkOutcome, SymlinkCreateAttempt,
};
use crate::test_support::unique_temp_path;
use std::ffi::OsString;

#[test]
fn create_symlink_is_idempotent_for_an_exact_target() {
    let root = unique_temp_path("create-symlink");
    let link = root.join("service").join("enabled");

    assert_eq!(
        create_symlink_if_missing(&link, Path::new("../run")).expect("create symbolic link"),
        CreateSymlinkOutcome::Created
    );
    assert_eq!(
        create_symlink_if_missing(&link, Path::new("../run")).expect("keep matching symbolic link"),
        CreateSymlinkOutcome::Unchanged
    );
    assert_eq!(
        read_symlink(&link).expect("read link"),
        Some("../run".into())
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn create_symlink_preserves_a_different_target() {
    let root = unique_temp_path("create-symlink-mismatch");
    let link = root.join("enabled");
    fs::create_dir_all(&root).expect("create root");
    symlink("actual", &link).expect("create existing link");

    let outcome = create_symlink_if_missing(&link, Path::new("expected"))
        .expect("inspect existing symbolic link");

    assert_eq!(
        outcome,
        CreateSymlinkOutcome::TargetMismatch("actual".into())
    );
    assert_eq!(
        read_symlink(&link).expect("read link"),
        Some("actual".into())
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn symlink_creation_result_distinguishes_creation_collision_and_failure() {
    assert_eq!(
        classify_symlink_creation(Ok(())).expect("successful symlink creation"),
        SymlinkCreateAttempt::Created
    );
    assert_eq!(
        classify_symlink_creation(Err(std::io::ErrorKind::AlreadyExists.into()))
            .expect("symlink collision"),
        SymlinkCreateAttempt::Collision
    );

    let error = classify_symlink_creation(Err(std::io::ErrorKind::PermissionDenied.into()))
        .expect_err("unrelated symlink failure should propagate");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
}

#[test]
fn existing_link_classification_distinguishes_exact_and_different_targets() {
    assert_eq!(
        existing_link_outcome("service".into(), Path::new("service")),
        CreateSymlinkOutcome::Unchanged
    );
    assert_eq!(
        existing_link_outcome("other".into(), Path::new("service")),
        CreateSymlinkOutcome::TargetMismatch("other".into())
    );
}

#[test]
fn create_symlink_rejects_an_existing_regular_file() {
    let root = unique_temp_path("create-symlink-regular");
    let link = root.join("enabled");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&link, "regular").expect("write regular file");

    create_symlink_if_missing(&link, Path::new("service"))
        .expect_err("regular destination should fail");

    assert_eq!(
        fs::read_to_string(link).expect("read regular file"),
        "regular"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn create_symlink_rejects_a_linked_parent() {
    let root = unique_temp_path("create-symlink-linked-parent");
    let outside = root.join("outside");
    let linked = root.join("linked");
    fs::create_dir_all(&outside).expect("create outside");
    symlink(&outside, &linked).expect("create parent link");

    create_symlink_if_missing(&linked.join("enabled"), Path::new("service"))
        .expect_err("linked parent should fail");

    assert!(!outside.join("enabled").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn atomic_symlink_replacement_handles_missing_and_existing_links() {
    let root = unique_temp_path("replace-symlink");
    let link = root.join("compiled");

    assert!(replace_symlink_atomic(&link, Path::new("compiled-one")).expect("create compiled link"));
    assert!(
        replace_symlink_atomic(&link, Path::new("compiled-two")).expect("replace compiled link")
    );
    assert!(!replace_symlink_atomic(&link, Path::new("compiled-two"))
        .expect("matching compiled link stays unchanged"));

    assert_eq!(
        read_symlink(&link).expect("read compiled link"),
        Some("compiled-two".into())
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn atomic_symlink_replacement_rejects_a_regular_destination() {
    let root = unique_temp_path("replace-symlink-regular");
    let link = root.join("compiled");
    fs::create_dir_all(&root).expect("create root");
    fs::write(&link, "regular").expect("write regular destination");

    replace_symlink_atomic(&link, Path::new("compiled-next"))
        .expect_err("regular destination should fail");

    assert_eq!(
        fs::read_to_string(link).expect("read regular destination"),
        "regular"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn temporary_symlink_reservation_skips_a_collision_and_uses_the_next_name() {
    let root = unique_temp_path("symlink-temp-collision");
    fs::create_dir_all(&root).expect("create root");
    symlink("protected", root.join("first")).expect("plant first candidate");
    let (parent_fd, _) = open_parent(&root.join("link")).expect("open parent");

    let reserved = reserve_temp_symlink(
        &parent_fd,
        [OsString::from("first"), OsString::from("second")],
        Path::new("service"),
    )
    .expect("reserve second candidate");

    assert_eq!(reserved, OsString::from("second"));
    assert_eq!(
        fs::read_link(root.join("first")).expect("first link"),
        Path::new("protected")
    );
    assert_eq!(
        fs::read_link(root.join("second")).expect("second link"),
        Path::new("service")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn temporary_symlink_reservation_propagates_non_collision_errors() {
    let root = unique_temp_path("symlink-temp-error");
    fs::create_dir_all(&root).expect("create root");
    let (parent_fd, _) = open_parent(&root.join("link")).expect("open parent");

    let error = reserve_temp_symlink(
        &parent_fd,
        [OsString::from("x".repeat(300)), OsString::from("unused")],
        Path::new("service"),
    )
    .expect_err("overlong candidate should fail");

    assert_ne!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(!root.join("unused").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn symlink_revalidation_accepts_links_and_missing_entries_but_rejects_files() {
    let root = unique_temp_path("symlink-revalidation");
    fs::create_dir_all(&root).expect("create root");
    let (parent_fd, _) = open_parent(&root.join("target")).expect("open parent");

    validate_symlink_or_missing(&parent_fd, std::ffi::OsStr::new("missing"))
        .expect("missing entry is safe");
    symlink("service", root.join("link")).expect("create link");
    validate_symlink_or_missing(&parent_fd, std::ffi::OsStr::new("link"))
        .expect("link entry is safe");
    fs::write(root.join("regular"), "data").expect("write regular file");
    validate_symlink_or_missing(&parent_fd, std::ffi::OsStr::new("regular"))
        .expect_err("regular entry should fail");

    let _ = fs::remove_dir_all(root);
}
