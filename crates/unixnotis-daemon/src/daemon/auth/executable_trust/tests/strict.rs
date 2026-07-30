use super::super::fingerprint::fingerprint_cache;
use super::super::paths::is_trusted_control_executable_path;
use super::super::snapshots::trusted_snapshot_cache;
use crate::daemon::auth::authorization::control_executable_is_allowed;
use crate::daemon::auth::support::write_executable;
use crate::test_support::{env_lock, TempRoot};

#[test]
fn strict_trust_uses_current_executable_directory_and_rejects_foreign_path() {
    let _guard = env_lock();
    let current_exe = std::env::current_exe().expect("current test executable");
    let trusted_dir = current_exe
        .parent()
        .expect("current executable should have a parent")
        .to_path_buf();
    let trusted = trusted_dir.join("noticenterctl");
    let root = TempRoot::new("auth-strict-foreign");
    let foreign = root.join("noticenterctl");
    write_executable(&trusted);
    write_executable(&foreign);
    trusted_snapshot_cache()
        .lock()
        .expect("snapshot cache lock")
        .clear();
    fingerprint_cache()
        .lock()
        .expect("fingerprint cache lock")
        .clear();

    assert!(is_trusted_control_executable_path(&trusted, false));
    assert!(!is_trusted_control_executable_path(&foreign, false));
    assert!(control_executable_is_allowed::<std::os::fd::OwnedFd>(
        Some(&trusted),
        None,
        &["noticenterctl"],
        false
    ));
    assert!(!control_executable_is_allowed::<std::os::fd::OwnedFd>(
        Some(&trusted),
        None,
        &["unixnotis-center"],
        false
    ));
    assert!(!control_executable_is_allowed::<std::os::fd::OwnedFd>(
        Some(&foreign),
        None,
        &["noticenterctl"],
        false
    ));

    let _ = std::fs::remove_file(trusted);
}
