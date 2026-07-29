//! Runtime-target file validation cases

use std::os::unix::fs::symlink;
use std::path::Path;
use std::{fs, os::unix::fs::PermissionsExt};

use super::super::validation::protected_runtime_target;
use crate::test_support::TempRoot;

#[test]
fn protected_runtime_target_accepts_installed_regular_executable() {
    let identity = protected_runtime_target(Path::new("/usr/bin/true"))
        .expect("installed protected executable");

    assert!(identity.is_system_managed());
    assert!(identity.is_executable_regular());
}

#[test]
fn runtime_target_symlink_is_not_followed() {
    let root = TempRoot::new("runtime-target-symlink");
    let path = root.join("runtime");
    symlink("/usr/bin/true", &path).expect("create runtime target symlink fixture");

    assert!(protected_runtime_target(&path).is_none());
}

#[test]
fn changed_runtime_target_identity_is_detected() {
    let current =
        protected_runtime_target(Path::new("/usr/bin/true")).expect("current runtime target");
    let stale =
        protected_runtime_target(Path::new("/usr/bin/false")).expect("different runtime target");

    assert!(!current.same_file(stale));
}

#[test]
fn user_owned_runtime_target_is_rejected() {
    let root = TempRoot::new("user-runtime-target");
    let path = root.join("runtime");
    fs::write(&path, "fixture").expect("write runtime target fixture");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("make runtime target executable");

    assert!(protected_runtime_target(&path).is_none());
}
