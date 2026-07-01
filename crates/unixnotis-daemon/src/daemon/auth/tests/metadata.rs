use super::authorization::control_owner_uid_is_allowed;
use super::metadata::{
    trusted_control_file_metadata_is_safe, trusted_control_owner_uid_is_allowed,
};
use super::support::write_executable;
use crate::test_support::TempRoot;

#[cfg(unix)]
#[test]
fn trusted_control_file_metadata_rejects_group_or_world_writable_files() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempRoot::new("auth-metadata");
    let trusted = root.join("noticenterctl");
    write_executable(&trusted);

    let metadata = std::fs::metadata(&trusted).expect("metadata");
    assert!(trusted_control_file_metadata_is_safe(&metadata));

    let mut permissions = metadata.permissions();
    permissions.set_mode(0o775);
    std::fs::set_permissions(&trusted, permissions).expect("set group writable");
    let metadata = std::fs::metadata(&trusted).expect("metadata");
    assert!(!trusted_control_file_metadata_is_safe(&metadata));
}

#[test]
fn control_uid_helpers_accept_only_expected_or_trusted_owner() {
    let expected_uid = rustix::process::geteuid().as_raw();
    let other_uid = expected_uid.saturating_add(1);

    assert!(control_owner_uid_is_allowed(expected_uid, expected_uid));
    assert!(!control_owner_uid_is_allowed(other_uid, expected_uid));

    assert!(trusted_control_owner_uid_is_allowed(
        expected_uid,
        expected_uid
    ));
    assert!(trusted_control_owner_uid_is_allowed(0, expected_uid));
    assert!(!trusted_control_owner_uid_is_allowed(
        other_uid,
        expected_uid
    ));
}
