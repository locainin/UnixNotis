use std::fs;

use super::super::parse::{css_validator_binary_from, is_executable_regular_file};
use super::helpers::TempDirGuard;

#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt as _};

#[cfg(unix)]
#[test]
fn validator_lookup_accepts_only_executable_regular_files() {
    let root = TempDirGuard::new("validator-lookup");
    let executable = root.path().join("unixnotis-css-validate");
    fs::write(&executable, "#!/bin/sh\n").expect("write validator fixture");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
        .expect("mark validator executable");

    let noticenterctl = root.path().join("noticenterctl");
    assert_eq!(
        css_validator_binary_from(&noticenterctl).expect("find validator"),
        executable
    );

    fs::set_permissions(&executable, fs::Permissions::from_mode(0o600))
        .expect("remove executable permission");
    assert!(css_validator_binary_from(&noticenterctl).is_err());
}

#[cfg(unix)]
#[test]
fn validator_lookup_rejects_symlink_candidates() {
    let root = TempDirGuard::new("validator-symlink");
    let target = root.path().join("real-validator");
    fs::write(&target, "#!/bin/sh\n").expect("write validator target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700))
        .expect("mark validator target executable");

    let candidate = root.path().join("unixnotis-css-validate");
    symlink(&target, &candidate).expect("link validator candidate");

    assert!(!is_executable_regular_file(&candidate));
    assert!(css_validator_binary_from(&root.path().join("noticenterctl")).is_err());
}

#[cfg(unix)]
#[test]
fn validator_lookup_checks_test_binary_parent() {
    let root = TempDirGuard::new("validator-test-parent");
    let deps = root.path().join("deps");
    fs::create_dir(&deps).expect("create deps directory");
    let executable = root.path().join("unixnotis-css-validate");
    fs::write(&executable, "#!/bin/sh\n").expect("write validator fixture");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
        .expect("mark validator executable");

    assert_eq!(
        css_validator_binary_from(&deps.join("noticenterctl-test")).expect("find validator"),
        executable
    );
}
