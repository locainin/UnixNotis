use std::os::unix::fs::symlink;

use super::{owned_expected_object, InstallerLock};

#[test]
fn lock_ownership_requires_both_the_expected_shape_and_effective_user() {
    assert!(owned_expected_object(true, 1_000, 1_000));
    assert!(!owned_expected_object(false, 1_000, 1_000));
    assert!(!owned_expected_object(true, 1_001, 1_000));
    assert!(!owned_expected_object(false, 1_001, 1_000));
}

#[test]
fn second_installer_cannot_acquire_the_same_action_lock() {
    let root = test_root("contended");
    let lock_path = root.join("installer.lock");
    let first = InstallerLock::acquire_at(&lock_path).expect("first action lock");

    let error = InstallerLock::acquire_at(&lock_path).expect_err("second action must be rejected");

    assert!(
        error
            .to_string()
            .contains("another UnixNotis installer action is already running"),
        "unexpected contention error: {error:#}"
    );
    drop(first);
    InstallerLock::acquire_at(&lock_path).expect("released action lock");
    std::fs::remove_dir_all(root).expect("remove lock fixture");
}

#[test]
fn installer_lock_rejects_a_symlink_target() {
    let root = test_root("symlink");
    let target = root.join("target");
    std::fs::write(&target, b"not a lock").expect("write symlink target");
    let lock_path = root.join("installer.lock");
    symlink(&target, &lock_path).expect("create lock symlink");

    InstallerLock::acquire_at(&lock_path).expect_err("lock symlink must be rejected");

    std::fs::remove_dir_all(root).expect("remove lock fixture");
}

fn test_root(label: &str) -> std::path::PathBuf {
    let root = crate::test_support::fs::unique_temp_path(&format!("installer-lock-{label}"));
    let _cleanup = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create lock fixture");
    root
}
