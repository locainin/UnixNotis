use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use super::{
    create_symlink_if_missing, read_symlink, replace_symlink_atomic, CreateSymlinkOutcome,
};
use crate::test_support::unique_temp_path;

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
