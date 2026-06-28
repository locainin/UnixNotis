use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::trial::{
    path_entries_match, path_exists_no_follow, remove_trial_control_shim, select_trial_shim_dir,
    trial_launch_script,
};

fn temp_dir(label: &str) -> PathBuf {
    // Unique paths keep parallel test runs from sharing trial shim state
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "unixnotis-installer-main-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

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
#[cfg(unix)]
fn path_exists_no_follow_detects_dangling_trial_shim_symlink() {
    let root = temp_dir("dangling-shim");
    let shim = root.join("noticenterctl");
    std::os::unix::fs::symlink(root.join("missing-target"), &shim).expect("dangling shim");

    // exists() would return false here, but trial mode must still refuse to overwrite it
    assert!(path_exists_no_follow(&shim));

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
    assert!(!path_exists_no_follow(&shim));
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
fn trial_launch_script_guards_cleanup_with_expected_symlink_target() {
    let script = trial_launch_script(
        "'/tmp/unixnotis-daemon'",
        "'/home/user/.local/bin/noticenterctl'",
        "'/tmp/target/debug/noticenterctl'",
    );

    // Signal-time cleanup must not be a blind rm of whatever is at the shim path
    assert!(script.contains("[ -L '/home/user/.local/bin/noticenterctl' ]"));
    assert!(script.contains("readlink -- '/home/user/.local/bin/noticenterctl'"));
    assert!(script.contains("= '/tmp/target/debug/noticenterctl'"));
    assert!(script.contains("rm -f -- '/home/user/.local/bin/noticenterctl'"));
}
