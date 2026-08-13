//! Service artifact symlink creation and safe removal

use std::io::ErrorKind;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::filesystem::{
    create_symlink_if_missing, remove_symlink_if_target, CreateSymlinkOutcome, RemoveSymlinkOutcome,
};

use crate::paths::format_with_home;

pub(in crate::actions::install) fn write_service_symlink(
    path: &Path,
    target: &Path,
) -> Result<bool> {
    // Relative targets are compared exactly as stored by the service backend
    match create_symlink_if_missing(path, target) {
        Ok(CreateSymlinkOutcome::Created) => Ok(true),
        Ok(CreateSymlinkOutcome::Unchanged) => Ok(false),
        Ok(CreateSymlinkOutcome::TargetMismatch(existing)) => Err(anyhow!(
            "cannot replace service symlink {} because it points to {} instead of {}",
            format_with_home(path),
            format_with_home(&existing),
            format_with_home(target)
        )),
        Err(error) if error.kind() == ErrorKind::InvalidInput => Err(anyhow!(
            "cannot replace non-symlink service artifact at {}",
            format_with_home(path)
        )),
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to inspect or create symlink {}",
                format_with_home(path)
            )
        }),
    }
}

pub(in crate::actions::install) fn remove_service_symlink(
    path: &Path,
    expected_target: &Path,
) -> Result<()> {
    // Core compares the stored target and unlinks relative to the same stable parent descriptor
    match remove_symlink_if_target(path, expected_target) {
        Ok(RemoveSymlinkOutcome::Missing | RemoveSymlinkOutcome::Removed) => Ok(()),
        Ok(RemoveSymlinkOutcome::TargetMismatch(actual_target)) => Err(anyhow!(
            "refusing to remove symlink {} because it points to {} instead of {}",
            format_with_home(path),
            format_with_home(&actual_target),
            format_with_home(expected_target)
        )),
        Err(error) if error.kind() == ErrorKind::InvalidInput => Err(anyhow!(
            "refusing to remove non-symlink service artifact at {}",
            format_with_home(path)
        )),
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to inspect or remove symlink {}",
                format_with_home(path)
            )
        }),
    }
}
