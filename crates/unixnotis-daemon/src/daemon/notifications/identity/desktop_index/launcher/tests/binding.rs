//! Launcher-binding revalidation failure cases

use super::super::launcher_binding_is_current;
use crate::daemon::notifications::identity::desktop_index::model::PackageLauncherBinding;
use crate::daemon::notifications::identity::executable::FileIdentity;

#[test]
fn unprotected_launcher_binding_is_never_current() {
    let binding = PackageLauncherBinding {
        launcher_path: "/tmp/unixnotis-missing-launcher".into(),
        launcher_identity: identity(80),
        launcher_digest: [0; 32],
        target_path: "/tmp/unixnotis-missing-runtime".into(),
        target_identity: identity(81),
    };

    assert!(!launcher_binding_is_current(&binding));
}

fn identity(inode: u64) -> FileIdentity {
    FileIdentity {
        device: 1,
        inode,
        uid: 0,
        mode: 0o100_755,
    }
}
