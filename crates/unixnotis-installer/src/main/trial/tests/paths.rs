use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::paths::{find_command_on_path_with_index, path_entries_match, path_exists_no_follow};
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
fn command_lookup_skips_non_executable_path_shadow() {
    let root = temp_dir("non-executable-path-shadow");
    let shadow_dir = root.join("shadow");
    let real_dir = root.join("real");
    fs::create_dir_all(&shadow_dir).expect("shadow dir");
    fs::create_dir_all(&real_dir).expect("real dir");
    let shadow = shadow_dir.join("noticenterctl");
    let real = real_dir.join("noticenterctl");
    fs::write(&shadow, "#!/bin/sh\nexit 1\n").expect("shadow command");
    fs::write(&real, "#!/bin/sh\nexit 0\n").expect("real command");
    fs::set_permissions(&shadow, fs::Permissions::from_mode(0o644)).expect("shadow chmod");
    fs::set_permissions(&real, fs::Permissions::from_mode(0o755)).expect("real chmod");

    let found =
        find_command_on_path_with_index("noticenterctl", &[shadow_dir.clone(), real_dir.clone()])
            .expect("executable command should be found");

    // A non-executable file with the right name should not block the usable shim target
    assert_eq!(found.0, 1);
    assert_eq!(found.1, real);

    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn command_lookup_accepts_first_executable_file() {
    let root = temp_dir("first-executable-command");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    let command = bin.join("noticenterctl");
    fs::write(&command, "#!/bin/sh\nexit 0\n").expect("command");
    fs::set_permissions(&command, fs::Permissions::from_mode(0o755)).expect("chmod command");

    let found = find_command_on_path_with_index("noticenterctl", std::slice::from_ref(&bin))
        .expect("executable command should be found");

    // Trial shim selection should mirror the first executable command a shell can run
    assert_eq!(found.0, 0);
    assert_eq!(found.1, command);

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
