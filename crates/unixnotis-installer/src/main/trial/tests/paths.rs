use std::fs;
use std::path::Path;

use super::paths::{path_entries_match, path_exists_no_follow};
use super::test_support::temp_dir;

#[test]
fn path_entries_match_accepts_canonical_equivalents() {
    // Symlinked PATH entries should behave like their real directory
    let root = temp_dir("canonical");
    let target = root.join("target");
    let alias = root.join("alias");
    fs::create_dir_all(&target).expect("target");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &alias).expect("symlink");

    #[cfg(unix)]
    assert!(path_entries_match(Path::new(&alias), Path::new(&target)));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn path_entries_match_rejects_different_existing_directories() {
    let root = temp_dir("different-paths");
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(&left).expect("left");
    fs::create_dir_all(&right).expect("right");

    // Different real directories must not be treated as the same PATH entry
    assert!(!path_entries_match(&left, &right));

    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn path_exists_no_follow_detects_dangling_trial_shim_symlink() {
    let root = temp_dir("dangling-shim");
    let shim = root.join("noticenterctl");
    std::os::unix::fs::symlink(root.join("missing-target"), &shim).expect("dangling shim");

    // exists() would return false here, but trial mode must still refuse to overwrite it
    assert!(path_exists_no_follow(&shim));

    let _ = fs::remove_dir_all(root);
}
