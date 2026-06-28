use std::fs;

use super::shim::{
    remove_trial_control_shim, select_trial_shim_dir, trial_control_command_is_compatible,
};
use super::test_support::temp_dir;

#[test]
fn trial_shim_dir_uses_local_bin_when_it_wins_path_resolution() {
    // This models the clean case where ~/.local/bin is the first command location
    let root = temp_dir("wins-path");
    let local_bin = root.join("local").join("bin");
    let fallback = root.join("fallback");
    fs::create_dir_all(&local_bin).expect("local bin");
    fs::create_dir_all(&fallback).expect("fallback");

    let path_entries = vec![local_bin.clone(), fallback];
    // No existing command means the new shim would become the command shell finds
    let selected = select_trial_shim_dir(&local_bin, &path_entries, None);

    assert_eq!(selected.as_deref(), Some(local_bin.as_path()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn trial_shim_dir_creates_missing_local_bin_when_it_is_visible_on_path() {
    let root = temp_dir("create-local-bin");
    let local_bin = root.join("local").join("bin");

    let selected = select_trial_shim_dir(&local_bin, std::slice::from_ref(&local_bin), None);

    // The directory is created only after proving it can win PATH lookup
    assert_eq!(selected.as_deref(), Some(local_bin.as_path()));
    assert!(local_bin.is_dir());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn trial_shim_dir_rejects_shadowed_local_bin() {
    // This protects against creating a trusted shim that shell lookup never reaches
    let root = temp_dir("shadowed");
    let shadow_dir = root.join("shadow");
    let local_bin = root.join("local").join("bin");
    fs::create_dir_all(&shadow_dir).expect("shadow dir");
    fs::create_dir_all(&local_bin).expect("local bin");

    let existing = shadow_dir.join("noticenterctl");
    fs::write(&existing, "#!/bin/sh\n").expect("existing");
    // The shadow directory appears first, so its command wins PATH resolution
    let path_entries = vec![shadow_dir, local_bin.clone()];

    let selected = select_trial_shim_dir(&local_bin, &path_entries, Some(&(0, existing)));

    assert!(selected.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn trial_shim_dir_rejects_local_bin_when_not_on_path() {
    // A shim outside PATH would be invisible to normal noticenterctl calls
    let root = temp_dir("not-on-path");
    let local_bin = root.join("local").join("bin");
    let other = root.join("other");
    fs::create_dir_all(&local_bin).expect("local bin");
    fs::create_dir_all(&other).expect("other");

    let selected = select_trial_shim_dir(&local_bin, &[other], None);

    // Trial mode should fall back to the direct debug binary in this case
    assert!(selected.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn trial_control_command_accepts_debug_and_release_siblings() {
    let root = temp_dir("compatible-target-tree");
    let debug = root.join("target").join("debug").join("noticenterctl");
    let release = root.join("target").join("release").join("noticenterctl");
    fs::create_dir_all(debug.parent().expect("debug parent")).expect("debug parent");
    fs::create_dir_all(release.parent().expect("release parent")).expect("release parent");
    fs::write(&debug, "#!/bin/sh\n").expect("debug ctl");
    fs::write(&release, "#!/bin/sh\n").expect("release ctl");

    // Debug and release siblings are both trusted by trial daemon auth
    assert!(trial_control_command_is_compatible(&release, &debug));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn trial_control_command_rejects_unrelated_path() {
    let root = temp_dir("reject-unrelated");
    let debug = root.join("target").join("debug").join("noticenterctl");
    let unrelated = root.join("bin").join("noticenterctl");
    fs::create_dir_all(debug.parent().expect("debug parent")).expect("debug parent");
    fs::create_dir_all(unrelated.parent().expect("unrelated parent")).expect("unrelated parent");
    fs::write(&debug, "#!/bin/sh\n").expect("debug ctl");
    fs::write(&unrelated, "#!/bin/sh\n").expect("unrelated ctl");

    // Random commands should not be treated as trial-compatible control binaries
    assert!(!trial_control_command_is_compatible(&unrelated, &debug));

    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn remove_trial_control_shim_removes_only_matching_symlink() {
    let root = temp_dir("remove-matching-shim");
    let target = root.join("target").join("debug").join("noticenterctl");
    let shim = root.join("local").join("bin").join("noticenterctl");
    fs::create_dir_all(target.parent().expect("target parent")).expect("target parent");
    fs::create_dir_all(shim.parent().expect("shim parent")).expect("shim parent");
    fs::write(&target, "#!/bin/sh\n").expect("target");
    std::os::unix::fs::symlink(&target, &shim).expect("trial shim");

    let removed = remove_trial_control_shim(&shim, &target).expect("cleanup should succeed");

    assert!(removed);
    assert!(!super::paths::path_exists_no_follow(&shim));
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn remove_trial_control_shim_preserves_replaced_regular_file() {
    let root = temp_dir("preserve-replaced-shim");
    let target = root.join("target").join("debug").join("noticenterctl");
    let shim = root.join("local").join("bin").join("noticenterctl");
    fs::create_dir_all(target.parent().expect("target parent")).expect("target parent");
    fs::create_dir_all(shim.parent().expect("shim parent")).expect("shim parent");
    fs::write(&target, "#!/bin/sh\n").expect("target");
    fs::write(&shim, "user-owned command\n").expect("replaced command");

    let removed = remove_trial_control_shim(&shim, &target).expect("cleanup should not fail");

    assert!(!removed);
    assert_eq!(
        fs::read_to_string(&shim).expect("user file remains"),
        "user-owned command\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn remove_trial_control_shim_preserves_symlink_to_wrong_target() {
    let root = temp_dir("preserve-wrong-shim");
    let target = root.join("target").join("debug").join("noticenterctl");
    let other = root.join("other").join("noticenterctl");
    let shim = root.join("local").join("bin").join("noticenterctl");
    fs::create_dir_all(target.parent().expect("target parent")).expect("target parent");
    fs::create_dir_all(other.parent().expect("other parent")).expect("other parent");
    fs::create_dir_all(shim.parent().expect("shim parent")).expect("shim parent");
    fs::write(&target, "#!/bin/sh\n").expect("target");
    fs::write(&other, "#!/bin/sh\n").expect("other target");
    std::os::unix::fs::symlink(&other, &shim).expect("wrong shim");

    let removed = remove_trial_control_shim(&shim, &target).expect("cleanup should not fail");

    // Cleanup must not remove a user-replaced symlink just because the filename matches
    assert!(!removed);
    assert!(super::paths::path_exists_no_follow(&shim));
    let _ = fs::remove_dir_all(root);
}
