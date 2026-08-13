//! Launcher file-read boundary cases

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::time::{Duration, UNIX_EPOCH};

use rustix::io::{fcntl_getfd, FdFlags};

use super::super::read::{
    launcher_contents_are_supported, launcher_size_is_supported, open_launcher_descriptor,
    read_launcher, snapshot_is_unchanged, MAX_LAUNCHER_BYTES, MAX_LAUNCHER_LINES,
};
use crate::daemon::notifications::identity::executable::executable_evidence_for_path;
use crate::test_support::TempRoot;

#[test]
fn user_writable_launcher_is_not_inspected() {
    let root = TempRoot::new("user-writable-launcher");
    let path = root.join("launcher");
    fs::write(&path, "#!/bin/sh\nexec /usr/bin/true \"$@\"\n").expect("write launcher fixture");
    // Keep the fixture user-writable even when tests run as root in CI
    fs::set_permissions(&path, fs::Permissions::from_mode(0o775))
        .expect("make launcher fixture executable");
    let identity = executable_evidence_for_path(&path)
        .expect("read launcher fixture identity")
        .identity;

    assert!(read_launcher(&path, identity).is_none());
}

#[test]
fn launcher_symlink_is_not_followed() {
    let root = TempRoot::new("launcher-symlink");
    let target = std::path::Path::new("/usr/bin/true");
    let identity = executable_evidence_for_path(target)
        .expect("read target identity")
        .identity;
    let link = root.join("launcher");
    symlink(target, &link).expect("create launcher symlink fixture");

    assert!(read_launcher(&link, identity).is_none());
    assert!(open_launcher_descriptor(&link).is_none());
}

#[test]
fn oversized_launcher_is_rejected() {
    let root = TempRoot::new("oversized-launcher");
    let path = root.join("launcher");
    let contents = format!(
        "#!/bin/sh\n# {}\nexec /usr/bin/true\n",
        "x".repeat(65 * 1024)
    );
    fs::write(&path, contents).expect("write oversized launcher fixture");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("make oversized launcher executable");
    let identity = executable_evidence_for_path(&path)
        .expect("read oversized launcher identity")
        .identity;

    assert!(read_launcher(&path, identity).is_none());
    assert!(launcher_size_is_supported(MAX_LAUNCHER_BYTES));
    assert!(!launcher_size_is_supported(
        MAX_LAUNCHER_BYTES.saturating_add(1)
    ));
    let exact_bytes =
        vec![b'x'; usize::try_from(MAX_LAUNCHER_BYTES).expect("byte limit fits usize")];
    let oversized_bytes = vec![
        b'x';
        usize::try_from(MAX_LAUNCHER_BYTES.saturating_add(1))
            .expect("oversized byte limit fits usize")
    ];
    assert!(launcher_contents_are_supported(&exact_bytes));
    assert!(!launcher_contents_are_supported(&oversized_bytes));
    assert!(launcher_contents_are_supported(
        &"\n"
            .repeat(MAX_LAUNCHER_LINES.saturating_sub(1))
            .into_bytes()
    ));
    assert!(!launcher_contents_are_supported(
        &"\n".repeat(MAX_LAUNCHER_LINES).into_bytes()
    ));
}

#[test]
fn changed_launcher_identity_is_rejected() {
    let current = executable_evidence_for_path(std::path::Path::new("/usr/bin/true"))
        .expect("current system executable");
    let stale = executable_evidence_for_path(std::path::Path::new("/usr/bin/false"))
        .expect("different system executable");

    assert!(read_launcher(&current.canonical_path, stale.identity).is_none());
}

#[test]
fn launcher_descriptor_is_close_on_exec() {
    let descriptor = open_launcher_descriptor(std::path::Path::new("/usr/bin/true"))
        .expect("open protected launcher candidate");
    let flags = fcntl_getfd(&descriptor).expect("read launcher descriptor flags");

    assert!(flags.contains(FdFlags::CLOEXEC));
}

#[test]
fn launcher_snapshot_requires_identity_size_and_time_to_remain_equal() {
    let current = executable_evidence_for_path(std::path::Path::new("/usr/bin/true"))
        .expect("current executable identity")
        .identity;
    let other = executable_evidence_for_path(std::path::Path::new("/usr/bin/false"))
        .expect("other executable identity")
        .identity;
    let first_time = UNIX_EPOCH + Duration::from_secs(10);
    let second_time = UNIX_EPOCH + Duration::from_secs(11);

    assert!(snapshot_is_unchanged(
        current, current, 20, 20, first_time, first_time
    ));
    assert!(!snapshot_is_unchanged(
        current, other, 20, 20, first_time, first_time
    ));
    assert!(!snapshot_is_unchanged(
        current, current, 20, 21, first_time, first_time
    ));
    assert!(!snapshot_is_unchanged(
        current,
        current,
        20,
        20,
        first_time,
        second_time
    ));
}
