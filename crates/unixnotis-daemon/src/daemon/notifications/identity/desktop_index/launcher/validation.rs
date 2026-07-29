//! Protected runtime-target validation

use std::path::Path;

use super::super::super::executable::FileIdentity;
use super::read::open_launcher_descriptor;

pub(super) fn protected_runtime_target(path: &Path) -> Option<FileIdentity> {
    // Runtime targets are opened directly so the literal path cannot terminate in a symlink
    let descriptor = open_launcher_descriptor(path)?;
    let metadata = std::fs::File::from(descriptor).metadata().ok()?;
    let identity = FileIdentity::from_metadata(&metadata);
    (identity.is_system_managed() && identity.is_executable_regular()).then_some(identity)
}
