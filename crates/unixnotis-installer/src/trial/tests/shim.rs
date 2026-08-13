use std::fs;

use super::shim::{
    ensure_trial_control_access, remove_trial_control_shim, select_trial_shim_dir,
    trial_control_command_is_compatible,
};
use super::test_support::temp_dir;

#[test]
#[cfg(unix)]
fn ensure_trial_control_access_creates_and_owns_a_private_shim() {
    let _lock = crate::test_support::env::test_env_lock();
    let root = temp_dir("ensure-control-access");
    let home = root.join("home");
    let local_bin = home.join(".local").join("bin");
    let target = root.join("target").join("debug").join("noticenterctl");
    fs::create_dir_all(target.parent().expect("target parent")).expect("target parent");
    fs::write(&target, "#!/bin/sh\n").expect("trial control target");

    let _home = crate::test_support::env::EnvGuard::set("HOME", &home);
    let _path = crate::test_support::env::EnvGuard::set("PATH", &local_bin);

    let shim = ensure_trial_control_access(&target)
        .expect("trial control access should be checked")
        .expect("a visible local-bin path should receive a shim");

    assert_eq!(shim.path, local_bin.join("noticenterctl"));
    assert!(super::paths::path_exists_no_follow(&shim.path));

    // Drop owns cleanup, so a later trial cannot inherit this run's shim
    drop(shim);
    assert!(!super::paths::path_exists_no_follow(
        &local_bin.join("noticenterctl")
    ));

    let _ = fs::remove_dir_all(root);
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
fn trial_shim_dir_rejects_a_symlinked_local_bin() {
    let root = temp_dir("linked-local-bin");
    let outside = root.join("outside");
    let local_bin = root.join("local").join("bin");
    fs::create_dir_all(&outside).expect("outside directory");
    fs::create_dir_all(local_bin.parent().expect("local parent")).expect("local parent");
    std::os::unix::fs::symlink(&outside, &local_bin).expect("local bin link");

    let selected = select_trial_shim_dir(&local_bin, std::slice::from_ref(&local_bin), None);

    assert!(selected.is_none());
    assert!(!outside.join("noticenterctl").exists());
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
fn trial_control_command_rejects_arbitrary_local_bin_binary() {
    // A writable PATH directory cannot become a trial trust root by pathname
    let root = temp_dir("reject-local-bin-forgery");
    let debug = root.join("target").join("debug").join("noticenterctl");
    let forged = root.join(".local").join("bin").join("noticenterctl");
    fs::create_dir_all(debug.parent().expect("debug parent")).expect("debug parent");
    fs::create_dir_all(forged.parent().expect("local-bin parent")).expect("local-bin parent");
    fs::write(&debug, "#!/bin/sh\n").expect("debug ctl");
    fs::write(&forged, "#!/bin/sh\n").expect("forged ctl");

    assert!(!trial_control_command_is_compatible(&forged, &debug));

    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn trial_control_command_accepts_local_bin_symlink_to_trial_binary() {
    // PATH convenience remains compatible only when it resolves to the trial binary
    let root = temp_dir("accept-trial-local-bin-link");
    let debug = root.join("target").join("debug").join("noticenterctl");
    let shim = root.join(".local").join("bin").join("noticenterctl");
    fs::create_dir_all(debug.parent().expect("debug parent")).expect("debug parent");
    fs::create_dir_all(shim.parent().expect("local-bin parent")).expect("local-bin parent");
    fs::write(&debug, "#!/bin/sh\n").expect("debug ctl");
    std::os::unix::fs::symlink(&debug, &shim).expect("trial shim");

    assert!(trial_control_command_is_compatible(&shim, &debug));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn trial_control_command_rejects_renamed_component_binaries() {
    // Renderer and daemon names must not make unrelated local-bin files trusted
    let root = temp_dir("reject-renamed-components");
    let debug = root.join("target").join("debug").join("noticenterctl");
    fs::create_dir_all(debug.parent().expect("debug parent")).expect("debug parent");
    fs::write(&debug, "#!/bin/sh\n").expect("debug ctl");

    for executable in [
        "noticenterctl",
        "unixnotis-center",
        "unixnotis-popups",
        "unixnotis-daemon",
    ] {
        let forged = root.join(".local").join("bin").join(executable);
        fs::create_dir_all(forged.parent().expect("local-bin parent")).expect("local-bin parent");
        fs::write(&forged, "#!/bin/sh\n").expect("forged component");

        assert!(!trial_control_command_is_compatible(&forged, &debug));
    }

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

#[test]
#[cfg(unix)]
fn remove_trial_control_shim_treats_missing_path_as_already_clean() {
    let root = temp_dir("remove-missing-shim");
    let target = root.join("target").join("noticenterctl");
    let shim = root.join("missing").join("noticenterctl");

    let removed = remove_trial_control_shim(&shim, &target).expect("missing shim should be clean");

    assert!(!removed);
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn remove_trial_control_shim_reports_non_directory_parent() {
    let root = temp_dir("inspect-invalid-shim-parent");
    let parent = root.join("not-a-directory");
    let shim = parent.join("noticenterctl");
    let target = root.join("target").join("noticenterctl");
    fs::write(&parent, "blocking file\n").expect("blocking parent file");

    let error = remove_trial_control_shim(&shim, &target)
        .expect_err("invalid shim parent should report inspection failure");

    assert!(error
        .to_string()
        .contains("failed to inspect trial noticenterctl shim"));
    let _ = fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn remove_trial_control_shim_rejects_a_symlinked_parent() {
    let root = temp_dir("remove-linked-shim-parent");
    let target = root.join("target").join("noticenterctl");
    let outside = root.join("outside");
    let outside_shim = outside.join("noticenterctl");
    let linked_parent = root.join("linked-bin");
    fs::create_dir_all(target.parent().expect("target parent")).expect("target parent");
    fs::create_dir_all(&outside).expect("outside directory");
    fs::write(&target, "#!/bin/sh\n").expect("target");
    std::os::unix::fs::symlink(&target, &outside_shim).expect("outside trial shim");
    std::os::unix::fs::symlink(&outside, &linked_parent).expect("linked shim parent");
    let shim = linked_parent.join("noticenterctl");

    remove_trial_control_shim(&shim, &target).expect_err("linked parent should fail");

    assert_eq!(
        fs::read_link(&outside_shim).expect("outside shim remains"),
        target
    );
    let _ = fs::remove_dir_all(root);
}
