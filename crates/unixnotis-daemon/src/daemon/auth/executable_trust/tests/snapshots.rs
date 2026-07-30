#[cfg(not(target_os = "linux"))]
mod strict_snapshot_tests {
    use super::super::paths::{canonicalize_best_effort, trusted_snapshot_matches_observed};
    use super::super::snapshots::build_trusted_control_snapshots;
    use crate::daemon::auth::policy::{TrustedExecutableSnapshot, TRUSTED_CONTROL_EXECUTABLES};
    use crate::daemon::auth::support::write_executable;
    use crate::test_support::TempRoot;
    use std::collections::HashMap;
    use std::path::Path;

fn is_trusted_control_executable_path_in_dir(
    path: &Path,
    _trusted_dir: &Path,
    snapshots: &HashMap<String, TrustedExecutableSnapshot>,
) -> bool {
    let observed = canonicalize_best_effort(path);
    let Some(observed_name) = observed.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !TRUSTED_CONTROL_EXECUTABLES.contains(&observed_name) {
        return false;
    }

    snapshots
        .get(observed_name)
        .is_some_and(|snapshot| trusted_snapshot_matches_observed(snapshot, &observed))
}

#[test]
fn strict_snapshot_rejects_unknown_or_untrusted_paths() {
    let trusted_dir = TempRoot::new("auth-rejects-unknown");
    let outsider = trusted_dir.join("python3");
    write_executable(&outsider);
    let snapshots = build_trusted_control_snapshots(trusted_dir.path());

    // Random paths and unapproved binary names must not satisfy strict trust
    assert!(!is_trusted_control_executable_path_in_dir(
        std::path::Path::new("/tmp/noticenterctl"),
        trusted_dir.path(),
        &snapshots,
    ));
    assert!(!is_trusted_control_executable_path_in_dir(
        &outsider,
        trusted_dir.path(),
        &snapshots,
    ));
}

#[test]
fn strict_snapshot_rejects_trusted_name_alias_suffixes() {
    let trusted_dir = TempRoot::new("auth-rejects-alias");
    let alias = trusted_dir.join("noticenterctl.exe");
    write_executable(&alias);
    let snapshots = build_trusted_control_snapshots(trusted_dir.path());

    // Suffix lookalikes should not pass the exact trusted executable list
    assert!(!is_trusted_control_executable_path_in_dir(
        &alias,
        trusted_dir.path(),
        &snapshots,
    ));
}

#[test]
fn strict_snapshot_accepts_trusted_sibling_binary_only() {
    let trusted_dir = TempRoot::new("auth-accepts-sibling");
    let trusted = trusted_dir.join("noticenterctl");
    write_executable(&trusted);
    let snapshots = build_trusted_control_snapshots(trusted_dir.path());

    assert!(is_trusted_control_executable_path_in_dir(
        &trusted,
        trusted_dir.path(),
        &snapshots,
    ));

    let other_dir = TempRoot::new("auth-other-sibling");
    let forged = other_dir.join("noticenterctl");
    write_executable(&forged);
    assert!(!is_trusted_control_executable_path_in_dir(
        &forged,
        trusted_dir.path(),
        &snapshots,
    ));

    // Same path after replacement must no longer match the pinned startup fingerprint
    write_executable(&trusted);
    std::fs::write(&trusted, "#!/bin/sh\necho forged\n").expect("overwrite trusted sibling");
    assert!(!is_trusted_control_executable_path_in_dir(
        &trusted,
        trusted_dir.path(),
        &snapshots,
    ));
}

#[test]
fn strict_snapshot_pins_all_trusted_siblings_at_once() {
    let trusted_dir = TempRoot::new("auth-pins-all-siblings");
    let ctl = trusted_dir.join("noticenterctl");
    let center = trusted_dir.join("unixnotis-center");
    write_executable(&ctl);
    write_executable(&center);

    let snapshots = build_trusted_control_snapshots(trusted_dir.path());
    assert!(is_trusted_control_executable_path_in_dir(
        &ctl,
        trusted_dir.path(),
        &snapshots,
    ));

    // A sibling that has not called yet is still pinned by the initial snapshot
    std::fs::write(&center, "#!/bin/sh\necho replaced\n").expect("replace center");
    assert!(!is_trusted_control_executable_path_in_dir(
        &center,
        trusted_dir.path(),
        &snapshots,
    ));
}

#[cfg(unix)]
#[test]
fn strict_snapshot_rejects_group_writable_trusted_binary() {
    use std::os::unix::fs::PermissionsExt;

    let trusted_dir = TempRoot::new("auth-rejects-group-writable");
    let trusted = trusted_dir.join("noticenterctl");
    write_executable(&trusted);
    let mut permissions = std::fs::metadata(&trusted).expect("metadata").permissions();
    permissions.set_mode(0o775);
    std::fs::set_permissions(&trusted, permissions).expect("set permissions");

    let snapshots = build_trusted_control_snapshots(trusted_dir.path());

    assert!(!is_trusted_control_executable_path_in_dir(
        &trusted,
        trusted_dir.path(),
        &snapshots,
    ));
}
}