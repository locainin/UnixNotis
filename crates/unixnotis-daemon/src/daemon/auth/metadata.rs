//! File metadata policy for trusted control binaries

use std::os::unix::fs::MetadataExt;

use rustix::process::geteuid;

pub(in crate::daemon) fn trusted_control_file_metadata_is_safe(
    metadata: &std::fs::Metadata,
) -> bool {
    // Group/world writable binaries can be replaced by accounts outside the trust boundary
    let mode = metadata.mode();
    if mode & 0o022 != 0 {
        return false;
    }

    // User installs should be owned by the desktop user, while distro packages may be root
    let uid = metadata.uid();
    let expected_uid = geteuid().as_raw();
    trusted_control_owner_uid_is_allowed(uid, expected_uid)
}

pub(in crate::daemon) fn trusted_control_owner_uid_is_allowed(uid: u32, expected_uid: u32) -> bool {
    uid == expected_uid || uid == 0
}
