use std::path::Path;

use super::policy::{FileFingerprint, FileFingerprintSignature};

pub(super) fn write_executable(path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create executable parent");
    }
    std::fs::write(path, "#!/bin/sh\nexit 0\n").expect("write executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("set executable mode");
    }
}

pub(super) fn test_signature(len: u64) -> FileFingerprintSignature {
    let signed_len = i64::try_from(len).expect("test length should fit i64");
    FileFingerprintSignature {
        len,
        #[cfg(unix)]
        dev: len + 1,
        #[cfg(unix)]
        ino: len + 2,
        #[cfg(unix)]
        mode: 0o755,
        #[cfg(unix)]
        uid: rustix::process::geteuid().as_raw(),
        #[cfg(unix)]
        gid: 1000,
        #[cfg(unix)]
        mtime: signed_len + 3,
        #[cfg(unix)]
        mtime_nsec: signed_len + 4,
        #[cfg(unix)]
        ctime: signed_len + 5,
        #[cfg(unix)]
        ctime_nsec: signed_len + 6,
    }
}

pub(super) fn test_fingerprint(len: u64) -> FileFingerprint {
    FileFingerprint {
        signature: test_signature(len),
    }
}
