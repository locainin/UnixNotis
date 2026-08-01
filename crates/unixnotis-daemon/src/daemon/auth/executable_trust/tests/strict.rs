#[cfg(not(target_os = "linux"))]
mod strict_path_tests {
    use super::super::fingerprint::fingerprint_cache;
    use super::super::paths::is_trusted_control_executable_path;
    use super::super::snapshots::trusted_snapshot_cache;
    use crate::daemon::auth::authorization::control_executable_is_allowed;
    use crate::daemon::auth::support::write_executable;
    use crate::test_support::{env_lock, TempRoot};
    use std::fs::File;
    use std::os::fd::OwnedFd;

    fn open_test_executable(path: &std::path::Path) -> OwnedFd {
        File::open(path).expect("open test executable").into()
    }

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
        let trusted_fd = open_test_executable(&trusted);
        let foreign_fd = open_test_executable(&foreign);
        assert!(control_executable_is_allowed::<OwnedFd>(
            Some(&trusted),
            Some(&trusted_fd),
            &["noticenterctl"],
            false
        ));
        assert!(!control_executable_is_allowed::<OwnedFd>(
            Some(&trusted),
            Some(&trusted_fd),
            &["unixnotis-center"],
            false
        ));
        // Foreign path must be checked with its own fd to verify it's a different executable
        assert!(!control_executable_is_allowed::<OwnedFd>(
            Some(&foreign),
            Some(&foreign_fd),
            &["noticenterctl"],
            false
        ));

        let _ = std::fs::remove_file(trusted);
    }
}
